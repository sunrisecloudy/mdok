// Package runtime executes a parsed document: static validation (lint) and
// sequential step execution with checks and captures (test).
package runtime

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"time"

	"mdok/core"
	"mdok/curlplan"
	"mdok/httpx"
	"mdok/jmespath"
	"mdok/shell"
	"mdok/template"
)

// Runner executes documents against an effective configuration.
type Runner struct {
	Config *core.ExecConfig
}

// Lint statically validates a document: shell tokenization, curl option
// parsing, and policy checks on concrete (non-templated) URLs.
func (r *Runner) Lint(doc *core.Document) *core.DocumentResult {
	started := time.Now()
	result := &core.DocumentResult{Path: doc.Path, Status: "passed"}
	for _, item := range doc.Items {
		curl, ok := item.(*core.CurlItem)
		if !ok {
			continue
		}
		argv, diags := tokenize(curl, result)
		if len(diags) > 0 {
			continue
		}
		plan, diag := curlplan.Parse(argv)
		if diag != nil {
			diag.File, diag.Step = doc.Path, curl.Name
			result.Diagnostics = append(result.Diagnostics, *diag)
			continue
		}
		if !strings.Contains(plan.URL, "{{") {
			if diag := curlplan.CheckPolicy(plan, r.Config); diag != nil {
				diag.File, diag.Step = doc.Path, curl.Name
				result.Diagnostics = append(result.Diagnostics, *diag)
			}
		}
	}
	if hasErrors(result.Diagnostics) {
		result.Status = "failed"
	}
	result.DurationMS = time.Since(started).Milliseconds()
	return result
}

// Test executes every item in order: curl steps run against the configured
// policy, checks evaluate JMESPath assertions, captures merge new variables.
func (r *Runner) Test(ctx context.Context, doc *core.Document, cliVars map[string]any) *core.DocumentResult {
	started := time.Now()
	result := &core.DocumentResult{Path: doc.Path, Status: "passed"}
	vars := mergeVars(doc.Vars, cliVars)
	transfers := map[string]*transferBundle{}
	for _, item := range doc.Items {
		switch typed := item.(type) {
		case *core.CurlItem:
			stepStart := time.Now()
			step := core.StepResult{Name: typed.Name, Status: "passed"}
			argv, diags := tokenize(typed, result)
			if len(diags) == 0 {
				expanded := make([]string, len(argv))
				for i, arg := range argv {
					value, diag := template.Expand(arg, vars)
					if diag != nil {
						diag.File, diag.Step = doc.Path, typed.Name
						result.Diagnostics = append(result.Diagnostics, *diag)
						step.Status = "failed"
						break
					}
					expanded[i] = value
				}
				if step.Status == "passed" {
					plan, diag := curlplan.Parse(expanded)
					if diag != nil {
						diag.File, diag.Step = doc.Path, typed.Name
						result.Diagnostics = append(result.Diagnostics, *diag)
						step.Status = "failed"
					} else if diag := curlplan.CheckPolicy(plan, r.Config); diag != nil {
						diag.File, diag.Step = doc.Path, typed.Name
						result.Diagnostics = append(result.Diagnostics, *diag)
						step.Status = "failed"
					} else {
						runCtx, cancel := context.WithTimeout(ctx, r.Config.TotalTimeout)
						transfer, err := httpx.Execute(runCtx, plan, r.Config)
						cancel()
						if err != nil {
							result.Diagnostics = append(result.Diagnostics, core.Diagnostic{
								Severity: core.SeverityError, Code: "MDOK-E600",
								Title: "Transfer failed", Message: err.Error(),
								File: doc.Path, Step: typed.Name,
							})
							step.Status = "failed"
						} else {
							step.Method, step.URL = plan.Method, plan.URL
							step.StatusCode = transfer.Status
							step.Attempt = transfer.Attempt
							step.RedirectCount = transfer.RedirectCount
							transfers[typed.Name] = &transferBundle{transfer: transfer}
						}
					}
				}
			} else {
				step.Status = "failed"
			}
			step.DurationMS = time.Since(stepStart).Milliseconds()
			result.Steps = append(result.Steps, step)

		case *core.CheckItem:
			bundle, ok := transfers[typed.Step]
			if !ok {
				result.Diagnostics = append(result.Diagnostics, core.Diagnostic{
					Severity: core.SeverityError, Code: "MDOK-E501",
					Title: "Check without a step", Message: fmt.Sprintf("no executed step %q", typed.Step),
					File: doc.Path, Step: typed.Step,
				})
				continue
			}
			for _, line := range typed.Lines {
				root := buildRoot(bundle, vars)
				ok, err := jmespath.Check(line, root)
				if err != nil {
					diag, handled := jmespathDiagnostic(err)
					if !handled {
						diag = core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E500",
							Title: "Check evaluation failed", Message: err.Error()}
					}
					diag.File, diag.Step = doc.Path, typed.Step
					result.Diagnostics = append(result.Diagnostics, diag)
					continue
				}
				if !ok {
					result.Diagnostics = append(result.Diagnostics, core.Diagnostic{
						Severity: core.SeverityError, Code: "MDOK-E500",
						Title: "Check failed", Message: fmt.Sprintf("assertion failed: %s", line),
						File: doc.Path, Step: typed.Step,
					})
				}
			}

		case *core.CaptureItem:
			bundle, ok := transfers[typed.Step]
			if !ok {
				result.Diagnostics = append(result.Diagnostics, core.Diagnostic{
					Severity: core.SeverityError, Code: "MDOK-E501",
					Title: "Capture without a step", Message: fmt.Sprintf("no executed step %q", typed.Step),
					File: doc.Path, Step: typed.Step,
				})
				continue
			}
			root := buildRoot(bundle, vars)
			captured, err := jmespath.Capture(typed.Expr, root)
			if err != nil {
				diag, handled := jmespathDiagnostic(err)
				if !handled {
					diag = core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E503",
						Title: "Capture failed", Message: err.Error()}
				}
				diag.File, diag.Step = doc.Path, typed.Step
				result.Diagnostics = append(result.Diagnostics, diag)
				continue
			}
			for key, value := range captured {
				vars[key] = value
			}
		}
	}
	if hasErrors(result.Diagnostics) {
		result.Status = "failed"
	}
	result.DurationMS = time.Since(started).Milliseconds()
	return result
}

type transferBundle struct {
	transfer *core.Transfer
}

func buildRoot(bundle *transferBundle, vars map[string]any) map[string]any {
	headers := map[string][]string{}
	for _, kv := range bundle.transfer.Headers {
		headers[kv.Key] = append(headers[kv.Key], kv.Value)
	}
	root := jmespath.NewRoot(&jmespath.Transfer{
		Status:          bundle.transfer.Status,
		Headers:         headers,
		RedirectCount:   bundle.transfer.RedirectCount,
		Attempt:         bundle.transfer.Attempt,
		DownloadedBytes: len(bundle.transfer.Body),
	}, bundle.transfer.Body, vars)
	return root.Map()
}

func jmespathDiagnostic(err error) (core.Diagnostic, bool) {
	var jerr *jmespath.Error
	if errors.As(err, &jerr) {
		return jerr.Diagnostic, true
	}
	return core.Diagnostic{}, false
}

func tokenize(curl *core.CurlItem, result *core.DocumentResult) ([]string, []core.Diagnostic) {
	argv, err := shell.ParseCurlSource(curl.Source)
	if err != nil {
		diag, ok := shell.Diagnostic(err)
		if !ok {
			diag = core.Diagnostic{
				Severity: core.SeverityError, Code: "MDOK-E200",
				Title: "Shell error", Message: err.Error(),
			}
		}
		diag.File, diag.Step = result.Path, curl.Name
		result.Diagnostics = append(result.Diagnostics, diag)
		return nil, result.Diagnostics
	}
	return argv, nil
}

func mergeVars(docVars, cliVars map[string]any) map[string]any {
	vars := map[string]any{}
	for key, value := range docVars {
		vars[key] = value
	}
	for key, value := range cliVars {
		vars[key] = value
	}
	return vars
}

func hasErrors(diags []core.Diagnostic) bool {
	for _, diag := range diags {
		if diag.Severity == core.SeverityError {
			return true
		}
	}
	return false
}
