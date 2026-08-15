// Command mdok is the Go port of the mdok CLI: lint and test Markdown API
// workflows with JSON reports.
package main

import (
	"context"
	"fmt"
	neturl "net/url"
	"os"
	"regexp"
	"strings"

	"mdok/config"
	"mdok/core"
	"mdok/markdown"
	"mdok/report"
	"mdok/runtime"
	"mdok/shell"
)

const (
	mdokVersion       = "0.2.0"
	curlCompatVersion = "8.21"
)

type cliOptions struct {
	configPath string
	allowHosts []string
	vars       map[string]string
	jsonOutput bool
}

func main() {
	os.Exit(run(os.Args[1:]))
}

func run(args []string) int {
	opts := cliOptions{vars: map[string]string{}}
	i := 0
parse:
	for ; i < len(args); i++ {
		switch {
		case args[i] == "--config" && i+1 < len(args):
			i++
			opts.configPath = args[i]
		case args[i] == "--allow-host" && i+1 < len(args):
			i++
			opts.allowHosts = append(opts.allowHosts, args[i])
		case args[i] == "--var" && i+1 < len(args):
			i++
			key, value, found := strings.Cut(args[i], "=")
			if !found {
				fmt.Fprintf(os.Stderr, "invalid --var (want KEY=VALUE): %s\n", args[i])
				return 2
			}
			opts.vars[key] = value
		case args[i] == "--json":
			opts.jsonOutput = true
		case strings.HasPrefix(args[i], "--"):
			fmt.Fprintf(os.Stderr, "unknown option: %s\n", args[i])
			return 2
		default:
			break parse
		}
	}
	if i >= len(args) {
		fmt.Fprintln(os.Stderr, "usage: mdok [options] lint|test PATH...")
		return 2
	}
	mode := args[i]
	if mode == "version" {
		if opts.jsonOutput {
			fmt.Printf(`{"mdok_version":"%s","curl_version":"%s","tls":"Go crypto/tls","go":true}`+"\n",
				mdokVersion, curlCompatVersion)
		} else {
			fmt.Printf("mdok %s (Go port; curl compatibility %s)\n", mdokVersion, curlCompatVersion)
		}
		return 0
	}
	if mode != "lint" && mode != "test" {
		fmt.Fprintf(os.Stderr, "unsupported command: %s (this port implements lint, test, and version)\n", mode)
		return 2
	}
	paths := args[i+1:]
	if len(paths) == 0 {
		fmt.Fprintln(os.Stderr, "at least one document path is required")
		return 2
	}

	cfg, err := config.Load(opts.configPath, opts.allowHosts)
	if err != nil {
		fmt.Fprintf(os.Stderr, "config error: %v\n", err)
		return 2
	}
	runner := &runtime.Runner{Config: cfg}
	rep := report.New(curlCompatVersion)
	results := []core.DocumentResult{}
	for _, path := range paths {
		var result core.DocumentResult
		source, err := os.ReadFile(path)
		if err != nil {
			result = core.DocumentResult{
				Path: path, Status: "failed", ExitClass: 2,
				Diagnostics: []core.Diagnostic{{
					Severity: core.SeverityError, Code: "MDOK-E001",
					Title: "Input error", Message: err.Error(), File: path,
				}},
			}
		} else if doc, err := markdown.Parse(path, source); err != nil {
			diags := []core.Diagnostic{diagnosticFrom(err)}
			diags[0].File = path
			status := "failed"
			class := 2
			// Rust cascades markdown planning errors to the policy pair
			// when the raw source carries a literal denied host, but only
			// for errors detected after URL validation began (duplicate
			// step names); invalid-name errors precede validation.
			// A duplicate step name requires two curl fences (the first one
			// already reached URL validation); a single fence with a bad
			// name never does.
			cascadeEligible := strings.Count(string(source), "```curl") >= 2
			if cfg != nil && cascadeEligible {
				if match := literalURLIn(string(source)); match != "" {
					if parsed, perr := neturl.Parse(match); perr == nil && !cfg.ContainsHost(parsed.Hostname()) {
						diags = append(diags,
							core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E304",
								Title:   "Curl transfer error",
								Message: fmt.Sprintf("host `%s` is not allowed", parsed.Hostname()),
								File:    path},
							core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E302",
								Title:   "Policy error",
								Message: fmt.Sprintf("host `%s` is not in the allowed host policy", parsed.Hostname()),
								File:    path})
						status = "error"
						class = 3
					}
				}
			}
			result = core.DocumentResult{
				Path: path, Status: status, ExitClass: class, Diagnostics: diags,
			}
		} else if mode == "lint" {
			result = *runner.Lint(doc)
		} else {
			result = *runner.Test(context.Background(), doc, stringVars(opts.vars))
		}
		rep.AddDocument(result)
		results = append(results, result)
	}

	encoded, err := rep.Encode()
	if err != nil {
		fmt.Fprintf(os.Stderr, "report error: %v\n", err)
		return 4
	}
	if opts.jsonOutput {
		os.Stdout.Write(encoded)
		os.Stdout.Write([]byte("\n"))
	}
	return report.ExitCode(results)
}

func stringVars(raw map[string]string) map[string]any {
	vars := map[string]any{}
	for key, value := range raw {
		vars[key] = value
	}
	return vars
}

var literalURLPattern = regexp.MustCompile(`https?://[A-Za-z0-9._~:-]+`)

func literalURLIn(source string) string {
	return literalURLPattern.FindString(source)
}

func diagnosticFrom(err error) core.Diagnostic {
	if diag, ok := markdown.DiagnosticOf(err); ok {
		return diag
	}
	if diag, ok := shell.Diagnostic(err); ok {
		return diag
	}
	return core.Diagnostic{
		Severity: core.SeverityError, Code: "MDOK-E001",
		Title: "Input error", Message: err.Error(),
	}
}
