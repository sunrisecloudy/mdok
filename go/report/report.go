// Package report assembles the MDOK JSON report from document results.
//
// It is a thin serializer over the shared core contracts: callers execute
// documents, fill durations, and append core.DocumentResult values; this
// package renders the stable wire format and maps outcomes to process
// exit codes.
package report

import (
	"encoding/json"

	"mdok/core"
)

// SchemaVersion is the report schema version, mirroring the Rust
// mdok-report crate.
const SchemaVersion = "1"

// Report is a builder over core.Report. It embeds the wire struct, so it
// serializes exactly like core.Report while adding append helpers.
type Report struct {
	core.Report
}

// New creates an empty report with schema version "1" and the given curl
// version.
func New(curlVersion string) *Report {
	return &Report{Report: core.Report{
		SchemaVersion: SchemaVersion,
		CurlVersion:   curlVersion,
		Documents:     []core.DocumentResult{},
	}}
}

// AddDocument appends one document outcome. Nil step and diagnostic
// slices are normalized to empty slices so the JSON wire shape keeps
// "steps":[] and "diagnostics":[] rather than nulls.
func (r *Report) AddDocument(result core.DocumentResult) {
	if result.Steps == nil {
		result.Steps = []core.StepResult{}
	}
	if result.Diagnostics == nil {
		result.Diagnostics = []core.Diagnostic{}
	}
	r.Documents = append(r.Documents, result)
}

// Encode serializes the report as one compact line of JSON with keys in
// struct order (schema_version, curl_version, documents; then path,
// status, duration_ms, steps, diagnostics per document), mirroring the
// Rust --json output.
func (r *Report) Encode() ([]byte, error) {
	return json.Marshal(&r.Report)
}

// ExitCode maps document statuses and the run mode to the process exit
// code, mirroring the Rust CLI:
//
//   - a fully passing (or skipped/planned-only) run exits 0;
//   - in test mode a failed or errored document is a failed check run
//     and exits 1;
//   - in lint and plan modes a failed or errored document is an input
//     error and exits 2 (the corpus-lint golden files record exit 2 for
//     documents with error diagnostics).
func ExitCode(statuses []string, mode string) int {
	hasFailure := false
	for _, status := range statuses {
		if status == "failed" || status == "error" {
			hasFailure = true
			break
		}
	}
	if !hasFailure {
		return 0
	}
	if mode == "test" {
		return 1
	}
	return 2
}
