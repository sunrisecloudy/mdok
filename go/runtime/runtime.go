// Package runtime executes a parsed document: static validation (lint) and
// sequential step execution with checks and captures (test).
//
// Failure taxonomy mirrors the Rust CLI:
//   - plan-time failures (shell/curl parse, JMESPath compile, template
//     variable references, policy on literal URLs) mark the document
//     failed/error with diagnostics and no executed steps (exit class 2/3);
//   - check/capture failures mark the owning step failed silently (no
//     document diagnostics; exit class 1);
//   - transfer failures mark the step failed silently (exit class 3).
package runtime

import (
	"context"
	"fmt"
	"net/url"
	"regexp"
	"strings"
	"time"

	"mdok/core"
	"mdok/curlplan"
	"mdok/httpx"
	"mdok/jmespath"
	"mdok/shell"
	"mdok/template"
)

// validateCurl runs the full static validation for one curl item with the
// Rust diagnostic cascades:
//   - denied host on a valid plan -> planner E304 plus policy E302, error;
//   - unknown option (E300) -> option diagnostic, plus policy E302 when the
//     raw URL operand's host is denied (the planner never ran);
//   - template render error -> template diagnostic alone when the URL
//     operand is a literal, plus invalid-URL E304 and policy E302 when the
//     operand itself is an unexpanded template (error status);
//   - shell errors stand alone (failed).
func validateCurl(curl *core.CurlItem, path string, cfg *core.ExecConfig, known map[string]any) *planFinding {
	argv, err := shell.ParseCurlSource(curl.Source)
	if err != nil {
		diag, ok := shell.Diagnostic(err)
		if !ok {
			diag = core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E200",
				Title: "Shell error", Message: err.Error()}
		}
		diag.File, diag.Step = path, curl.Name
		diags := []core.Diagnostic{diag}
		status := "failed"
		// The shell tokenizer validates template structure itself; a
		// template-class error still cascades to the policy pair when the
		// raw source carries a literal denied host.
		if diag.Code == "MDOK-E400" || diag.Code == "MDOK-E402" || diag.Code == "MDOK-E404" {
			if host, literal := literalHost(curl.Source); literal && !cfg.ContainsHost(host) {
				diags = append(diags,
					core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E304",
						Title:   "Curl transfer error",
						Message: fmt.Sprintf("host `%s` is not allowed", host),
						File:    path, Step: curl.Name},
					policyDenied(host))
				status = "error"
			}
		}
		return &planFinding{diags: diags, status: status}
	}
	if classDiags, option := curlplan.ClassifyOptions(argv); len(classDiags) > 0 {
		for i := range classDiags {
			classDiags[i].File, classDiags[i].Step = path, curl.Name
		}
		// --next and --parallel carry a host-policy companion when their
		// URL-bearing argument targets a denied host.
		if option == "--next" || option == "-Z" || option == "--parallel" {
			probe := ""
			for _, arg := range argv[1:] {
				if strings.HasPrefix(arg, "http://") || strings.HasPrefix(arg, "https://") {
					probe = arg
					break
				}
			}
			if probe != "" {
				if parsed, perr := url.Parse(probe); perr == nil && !cfg.ContainsHost(parsed.Hostname()) {
					classDiags = append(classDiags, policyDenied(parsed.Hostname()))
				}
			}
		}
		return &planFinding{diags: classDiags, status: "error"}
	}
	plan, diag := curlplan.Parse(argv)
	if diag != nil {
		diag.File, diag.Step = path, curl.Name
		diags := []core.Diagnostic{*diag}
		status := "failed"
		if diag.Code == "MDOK-E300" {
			if operand := urlOperand(argv); operand != "" && !strings.Contains(operand, "{{") {
				if parsed, perr := url.Parse(operand); perr == nil && !cfg.ContainsHost(parsed.Hostname()) {
					diags = append(diags, policyDenied(parsed.Hostname()))
					status = "error"
				}
			}
		}
		if diag.Code == "MDOK-E304" && strings.Contains(diag.Message, "exactly one URL") {
			// Multiple URL operands each contribute a planner/policy pair
			// for denied hosts.
			status = "error"
			denied := 0
			for _, arg := range argv[1:] {
				if strings.HasPrefix(arg, "http://") || strings.HasPrefix(arg, "https://") {
					if parsed, perr := url.Parse(arg); perr == nil && !cfg.ContainsHost(parsed.Hostname()) {
						denied++
					}
				}
			}
			if denied > 1 {
				diags = append(diags, policyDenied(""))
				for i := 1; i < denied; i++ {
					diags = append(diags,
						core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E304",
							Title: "Curl transfer error", Message: "exactly one URL is required",
							File: path, Step: curl.Name})
					diags = append(diags, policyDenied(""))
				}
			}
		}
		return &planFinding{diags: diags, status: status}
	}
	if known != nil {
		for _, arg := range argv {
			if _, diag := template.Expand(arg, known); diag != nil {
				diag.File, diag.Step = path, curl.Name
				diags := []core.Diagnostic{*diag}
				status := "failed"
				// Rust cascades to the planner/policy pair only when the URL
				// operand is a literal denied host; a templated URL cannot be
				// policy-checked at plan time and stands alone.
				if operand := urlOperand(argv); operand != "" && !strings.Contains(operand, "{{") {
					if parsed, perr := url.Parse(operand); perr == nil && !cfg.ContainsHost(parsed.Hostname()) {
						diags = append(diags,
							core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E304",
								Title:   "Curl transfer error",
								Message: fmt.Sprintf("host `%s` is not allowed", parsed.Hostname()),
								File:    path, Step: curl.Name},
							policyDenied(parsed.Hostname()))
						status = "error"
					}
				}
				return &planFinding{diags: diags, status: status}
			}
		}
	}
	if !strings.Contains(plan.URL, "{{") {
		return policyFinding(plan, path, curl.Name, cfg)
	}
	host, _ := literalPlanHost(plan.URL)
	if host != "" && !cfg.ContainsHost(host) {
		// Literal scheme://host prefix with a templated path still
		// policy-checks the host.
		return &planFinding{diags: []core.Diagnostic{
			{Severity: core.SeverityError, Code: "MDOK-E304", Title: "Curl transfer error",
				Message: fmt.Sprintf("host `%s` is not allowed", host), File: path, Step: curl.Name},
			policyDenied(host),
		}, status: "error"}
	}
	return nil
}

// literalPlanHost extracts the host when the URL has a literal
// scheme://host prefix (the path may still contain templates).
func literalPlanHost(raw string) (string, bool) {
	if !strings.Contains(raw, "://") {
		return "", false
	}
	rest := raw[strings.Index(raw, "://")+3:]
	slash := strings.Index(rest, "/")
	authority := rest
	if slash >= 0 {
		authority = rest[:slash]
	}
	if strings.Contains(authority, "{{") {
		return "", false
	}
	return authority, false
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// literalHost extracts the host of the first literal http(s) URL in raw
// curl source, reporting whether one exists.
func literalHost(source string) (string, bool) {
	match := literalURL.FindString(source)
	if match == "" {
		return "", false
	}
	if parsed, err := url.Parse(match); err == nil {
		return parsed.Hostname(), true
	}
	return "", false
}

var literalURL = regexp.MustCompile(`https?://[A-Za-z0-9._~:-]+`)

// urlOperand picks the non-option http(s) operand from a curl argv.
func urlOperand(argv []string) string {
	template := ""
	for i := 1; i < len(argv); i++ {
		arg := argv[i]
		if strings.HasPrefix(arg, "-") {
			continue
		}
		if strings.HasPrefix(arg, "http://") || strings.HasPrefix(arg, "https://") {
			return arg
		}
		if strings.Contains(arg, "{{") && template == "" {
			template = arg
		}
	}
	return template
}

func policyDenied(host string) core.Diagnostic {
	return core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E302",
		Title:   "Policy error",
		Message: fmt.Sprintf("host `%s` is not in the allowed host policy", host)}
}

// lintVars builds the lint-time variable view: document variables plus
// placeholders for every referenced name. Rust's lint validates template
// structure (syntax, depth, filters over known values) but not variable
// existence, since CLI variables are unavailable offline.
func lintVars(doc *core.Document) map[string]any {
	known := map[string]any{}
	for key, value := range doc.Vars {
		known[key] = value
	}
	for _, item := range doc.Items {
		switch typed := item.(type) {
		case *core.CurlItem:
			argv, err := shell.ParseCurlSource(typed.Source)
			if err != nil {
				continue
			}
			for _, arg := range argv {
				for _, name := range template.ParseNames(arg) {
					if _, exists := known[name]; !exists {
						known[name] = "lint"
					}
				}
			}
		case *core.CaptureItem:
			for _, key := range captureKeys(typed.Expr) {
				if _, exists := known[key]; !exists {
					known[key] = "lint"
				}
			}
		}
	}
	return known
}

// Runner executes documents against an effective configuration.
type Runner struct {
	Config *core.ExecConfig
}

// planFinding is one static-validation finding against a document item.
// status "failed" maps to exit class 2; "error" (policy) to class 3.
type planFinding struct {
	diags  []core.Diagnostic
	status string
}

func (f *planFinding) worst() (string, []core.Diagnostic) {
	return f.status, f.diags
}

// Lint statically validates a document: shell tokenization, curl option
// parsing, JMESPath compilation, and policy checks on concrete URLs.
// Template rendering is not validated (variables are unknown at lint time).
func (r *Runner) Lint(doc *core.Document) *core.DocumentResult {
	started := time.Now()
	result := &core.DocumentResult{Path: doc.Path, Status: "passed"}
	worst := "passed"
	for _, item := range doc.Items {
		switch typed := item.(type) {
		case *core.CurlItem:
			result.Steps = append(result.Steps, core.StepResult{Name: typed.Name, Status: "passed"})
			if finding := validateCurl(typed, doc.Path, r.Config, lintVars(doc)); finding != nil {
				result.Diagnostics = append(result.Diagnostics, finding.diags...)
				result.Steps[len(result.Steps)-1].Status = "failed"
				worst = worstStatus(worst, finding.status)
			}
		case *core.CheckItem:
			for _, line := range typed.Lines {
				if diag := jmespath.Validate(line); diag != nil {
					diag.File, diag.Step = doc.Path, typed.Step
					result.Diagnostics = append(result.Diagnostics, *diag)
					worst = worstStatus(worst, "failed")
				}
			}
		case *core.CaptureItem:
			if diag := jmespath.Validate(typed.Expr); diag != nil {
				diag.File, diag.Step = doc.Path, typed.Step
				result.Diagnostics = append(result.Diagnostics, *diag)
				worst = worstStatus(worst, "failed")
			}
		}
	}
	result.Status = worst
	finalize(result, started, true)
	return result
}

func worstStatus(current, candidate string) string {
	if current == "error" {
		return "error"
	}
	if candidate == "error" {
		return "error"
	}
	if current == "failed" || candidate == "failed" {
		return "failed"
	}
	return current
}

// Test pre-flights the full plan, then executes items in order.
func (r *Runner) Test(ctx context.Context, doc *core.Document, cliVars map[string]any) *core.DocumentResult {
	started := time.Now()
	vars := mergeVars(doc.Vars, cliVars)
	if finding := r.preflight(doc, vars); finding != nil {
		result := &core.DocumentResult{Path: doc.Path, Status: finding.status,
			Diagnostics: finding.diags, ExitClass: classFor(finding.status)}
		finalize(result, started, false)
		return result
	}
	result := &core.DocumentResult{Path: doc.Path, Status: "passed"}
	transfers := map[string]*core.Transfer{}
	stepByName := map[string]*core.StepResult{}
	aborted := false
	for _, item := range doc.Items {
		if aborted {
			break
		}
		switch typed := item.(type) {
		case *core.CurlItem:
			stepStart := time.Now()
			step := core.StepResult{Name: typed.Name, Status: "passed"}
			argv, err := shell.ParseCurlSource(typed.Source)
			if err == nil {
				expanded, diag := expandArgv(argv, vars)
				if diag != nil {
					// Execution-time template failure: silent step failure,
					// planning-class exit.
					step.Status = "failed"
					result.Status = "failed"
					if result.ExitClass < 2 {
						result.ExitClass = 2
					}
				} else if plan, pdiag := curlplan.Parse(expanded); pdiag != nil {
					step.Status = "failed"
					result.Status = "failed"
					if result.ExitClass < 2 {
						result.ExitClass = 2
					}
				} else if finding := policyFinding(plan, doc.Path, typed.Name, r.Config); finding != nil {
					result.Diagnostics = append(result.Diagnostics, finding.diags...)
					step.Status = "failed"
					result.Status = "error"
					result.ExitClass = 3
					aborted = true
				} else {
					runCtx, cancel := context.WithTimeout(ctx, r.Config.TotalTimeout)
					transfer, err := httpx.Execute(runCtx, plan, r.Config)
					cancel()
					if err != nil {
						// Transfer failure: silent step failure, exit class 3.
						step.Status = "failed"
						result.Status = "failed"
						if result.ExitClass < 3 {
							result.ExitClass = 3
						}
					} else {
						if strings.EqualFold(plan.Method, "HEAD") && len(transfer.Body) == 0 {
							// Rust synthesizes a method echo for bodiless
							// HEAD responses (mdok-curl evaluation_json).
							transfer.Body = []byte(`{"method":"HEAD"}`)
						}
						step.Method, step.URL = plan.Method, plan.URL
						step.StatusCode = transfer.Status
						step.Attempt = transfer.Attempt
						step.RedirectCount = transfer.RedirectCount
						transfers[typed.Name] = transfer
					}
				}
			} else {
				step.Status = "failed"
			}
			step.DurationMS = time.Since(stepStart).Milliseconds()
			result.Steps = append(result.Steps, step)
			stepByName[typed.Name] = &result.Steps[len(result.Steps)-1]

		case *core.CheckItem:
			transfer, ok := transfers[typed.Step]
			if !ok {
				continue
			}
			root := buildRoot(transfer, vars)
			for _, line := range typed.Lines {
				ok, err := jmespath.Check(line, root)
				if err != nil || !ok {
					markStepFailed(stepByName, typed.Step)
					result.Status = "failed"
					if result.ExitClass < 1 {
						result.ExitClass = 1
					}
				}
			}

		case *core.CaptureItem:
			transfer, ok := transfers[typed.Step]
			if !ok {
				continue
			}
			root := buildRoot(transfer, vars)
			captured, err := jmespath.Capture(typed.Expr, root)
			if err != nil {
				markStepFailed(stepByName, typed.Step)
				result.Status = "failed"
				if result.ExitClass < 1 {
					result.ExitClass = 1
				}
				continue
			}
			for key, value := range captured {
				vars[key] = value
			}
		}
	}
	finalize(result, started, false)
	return result
}

// preflight validates everything Rust validates before executing: shell
// tokenization, curl parsing, template rendering against document/CLI
// variables (capture-derived names get placeholders), JMESPath compilation,
// and policy. The first finding wins.
func (r *Runner) preflight(doc *core.Document, vars map[string]any) *planFinding {
	known := map[string]any{}
	for key, value := range vars {
		known[key] = value
	}
	var collected []core.Diagnostic
	sawError := false
	record := func(diags []core.Diagnostic, itemStatus string) {
		collected = append(collected, diags...)
		if itemStatus == "error" {
			sawError = true
		}
	}
	for _, item := range doc.Items {
		switch typed := item.(type) {
		case *core.CurlItem:
			if finding := validateCurl(typed, doc.Path, r.Config, known); finding != nil {
				record(finding.diags, finding.status)
			}
		case *core.CheckItem:
			for _, line := range typed.Lines {
				if diag := jmespath.Validate(line); diag != nil {
					diag.File, diag.Step = doc.Path, typed.Step
					record([]core.Diagnostic{*diag}, "failed")
				}
			}
		case *core.CaptureItem:
			if diag := jmespath.Validate(typed.Expr); diag != nil {
				diag.File, diag.Step = doc.Path, typed.Step
				record([]core.Diagnostic{*diag}, "failed")
				continue
			}
			for _, key := range captureKeys(typed.Expr) {
				if _, exists := known[key]; !exists {
					known[key] = "captured"
				}
			}
		}
	}
	if len(collected) == 0 {
		return nil
	}
	status := "failed"
	if sawError {
		status = "error"
	}
	return &planFinding{diags: collected, status: status}
}

// policyFinding checks an expanded plan and mirrors the Rust pair: the
// planner E304 plus the policy-layer E302 for host denials.
func policyFinding(plan *curlplan.Plan, path, step string, cfg *core.ExecConfig) *planFinding {
	hostDenied := false
	var host string
	if parsed, err := url.Parse(plan.URL); err == nil {
		host = parsed.Hostname()
		hostDenied = !cfg.ContainsHost(host)
		if !cfg.ContainsScheme(parsed.Scheme) {
			// Scheme denials emit the policy code twice.
			return &planFinding{diags: []core.Diagnostic{policyDenied(host), policyDenied(host)},
				status: "error"}
		}
	}
	cacertDenied := false
	if plan.CACert != "" {
		if diag := curlplan.CheckPolicy(plan, &core.ExecConfig{
			AllowedHosts:     []string{host},
			AllowedSchemes:   []string{"http", "https"},
			AllowedReadPaths: cfg.AllowedReadPaths,
		}); diag != nil && diag.Code == "MDOK-E303" {
			cacertDenied = true
		}
	}
	if hostDenied && cacertDenied {
		return &planFinding{diags: []core.Diagnostic{policyDenied(host),
			{Severity: core.SeverityError, Code: "MDOK-E303", Title: "File read denied",
				Message: "file is outside the allowed read roots", File: path, Step: step}},
			status: "error"}
	}
	if hostDenied {
		return &planFinding{diags: []core.Diagnostic{
			{Severity: core.SeverityError, Code: "MDOK-E304", Title: "Curl transfer error",
				Message: fmt.Sprintf("host `%s` is not allowed", host), File: path, Step: step},
			policyDenied(host)}, status: "error"}
	}
	if cacertDenied {
		return &planFinding{diags: []core.Diagnostic{
			{Severity: core.SeverityError, Code: "MDOK-E303", Title: "File read denied",
				Message: "file is outside the allowed read roots", File: path, Step: step}},
			status: "error"}
	}
	diag := curlplan.CheckPolicy(plan, cfg)
	if diag == nil {
		return nil
	}
	diag.File, diag.Step = path, step
	status := "failed"
	if diag.Code == "MDOK-E304" || diag.Code == "MDOK-E302" || diag.Code == "MDOK-E303" {
		status = "error"
	}
	return &planFinding{diags: []core.Diagnostic{*diag}, status: status}
}

// captureKeys extracts the object keys a capture multiselect produces
// ({id: body.id, total: length(body)}) so later templates referencing them
// validate against a placeholder instead of failing as missing variables.
func captureKeys(expr string) []string {
	var keys []string
	for _, segment := range strings.Split(expr, ",") {
		segment = strings.TrimSpace(segment)
		segment = strings.TrimPrefix(strings.TrimSpace(segment), "{")
		name, _, found := strings.Cut(segment, ":")
		if !found {
			continue
		}
		name = strings.TrimSpace(name)
		if name != "" {
			keys = append(keys, name)
		}
	}
	return keys
}

func expandArgv(argv []string, vars map[string]any) ([]string, *core.Diagnostic) {
	expanded := make([]string, len(argv))
	for i, arg := range argv {
		value, diag := template.Expand(arg, vars)
		if diag != nil {
			return nil, diag
		}
		expanded[i] = value
	}
	return expanded, nil
}

func buildRoot(transfer *core.Transfer, vars map[string]any) map[string]any {
	headers := map[string][]string{}
	for _, kv := range transfer.Headers {
		headers[kv.Key] = append(headers[kv.Key], kv.Value)
	}
	root := jmespath.NewRoot(&jmespath.Transfer{
		Status:          transfer.Status,
		Headers:         headers,
		RedirectCount:   transfer.RedirectCount,
		Attempt:         transfer.Attempt,
		DownloadedBytes: len(transfer.Body),
	}, transfer.Body, vars)
	return root.Map()
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

func markStepFailed(steps map[string]*core.StepResult, name string) {
	if step, ok := steps[name]; ok {
		step.Status = "failed"
	}
}

func classFor(status string) int {
	if status == "error" {
		return 3
	}
	return 2
}

func finalize(result *core.DocumentResult, started time.Time, lintMode bool) {
	if result.Status == "passed" && hasErrors(result.Diagnostics) {
		result.Status = "failed"
		if result.ExitClass < 2 {
			result.ExitClass = 2
		}
	}
	// Rust reports step entries only for documents that pass planning:
	// lint failures drop them; test keeps them except for policy "error"
	// documents, which never execute steps.
	if result.Status == "error" || (lintMode && result.Status != "passed") {
		result.Steps = nil
	}
	result.DurationMS = time.Since(started).Milliseconds()
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
