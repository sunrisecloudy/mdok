// Command mdok is the Go port of the mdok CLI: lint and test Markdown API
// workflows with JSON reports.
package main

import (
	"context"
	"fmt"
	"os"
	"strings"

	"mdok/config"
	"mdok/core"
	"mdok/markdown"
	"mdok/report"
	"mdok/runtime"
	"mdok/shell"
)

const curlCompatVersion = "8.21"

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
	if mode != "lint" && mode != "test" {
		fmt.Fprintf(os.Stderr, "unsupported command: %s (this port implements lint and test)\n", mode)
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
	statuses := []string{}
	for _, path := range paths {
		source, err := os.ReadFile(path)
		if err != nil {
			rep.AddDocument(core.DocumentResult{
				Path: path, Status: "failed",
				Diagnostics: []core.Diagnostic{{
					Severity: core.SeverityError, Code: "MDOK-E001",
					Title: "Input error", Message: err.Error(), File: path,
				}},
			})
			statuses = append(statuses, "failed")
			continue
		}
		doc, err := markdown.Parse(path, source)
		if err != nil {
			diag := diagnosticFrom(err)
			diag.File = path
			rep.AddDocument(core.DocumentResult{
				Path: path, Status: "failed", Diagnostics: []core.Diagnostic{diag},
			})
			statuses = append(statuses, "failed")
			continue
		}
		var result *core.DocumentResult
		if mode == "lint" {
			result = runner.Lint(doc)
		} else {
			result = runner.Test(context.Background(), doc, stringVars(opts.vars))
		}
		rep.AddDocument(*result)
		statuses = append(statuses, result.Status)
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
	return report.ExitCode(statuses, mode)
}

func stringVars(raw map[string]string) map[string]any {
	vars := map[string]any{}
	for key, value := range raw {
		vars[key] = value
	}
	return vars
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
