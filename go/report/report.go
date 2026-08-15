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

// ExitCode derives the process exit code from document results, mirroring
// the Rust taxonomy: 0 pass; 1 check/capture failure (silent step failure);
// 2 plan/static error (error diagnostics present); 3 transfer or policy
// error (status "error"). The mode no longer changes the mapping.
func ExitCode(results []core.DocumentResult) int {
	exit := 0
	for _, result := range results {
		class := result.ExitClass
		switch {
		case result.Status == "error":
			class = 3
		case class == 0 && len(result.Diagnostics) > 0:
			class = 2
		case class == 0 && result.Status == "failed":
			class = 1
		}
		if class > exit {
			exit = class
		}
	}
	return exit
}
