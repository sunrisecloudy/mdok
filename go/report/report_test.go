package report

import (
	"encoding/json"
	"strings"
	"testing"

	"mdok/core"
)

func TestNewInitializesSchemaAndCurlVersion(t *testing.T) {
	report := New("8.21")
	if report.SchemaVersion != "1" {
		t.Fatalf("schema_version = %q, want 1", report.SchemaVersion)
	}
	if report.CurlVersion != "8.21" {
		t.Fatalf("curl_version = %q, want 8.21", report.CurlVersion)
	}
	if report.Documents == nil || len(report.Documents) != 0 {
		t.Fatalf("documents = %#v, want empty non-nil slice", report.Documents)
	}
}

func TestEncodeCompactRoundTrip(t *testing.T) {
	report := New("8.21")
	report.AddDocument(core.DocumentResult{
		Path:       "tests/e2e/01-health-status.md",
		Status:     "passed",
		DurationMS: 42,
		Steps: []core.StepResult{{
			Name:          "health",
			Status:        "passed",
			DurationMS:    41,
			Method:        "GET",
			URL:           "http://127.0.0.1:39170/health",
			StatusCode:    200,
			Attempt:       1,
			RedirectCount: 0,
		}},
		Diagnostics: []core.Diagnostic{},
	})
	report.AddDocument(core.DocumentResult{
		Path:       "tests/corpus/markdown-valid/broken.md",
		Status:     "failed",
		DurationMS: 7,
		// Nil slices must serialize as [] rather than null.
		Diagnostics: []core.Diagnostic{{
			Severity: core.SeverityError,
			Code:     "MDOK-E401",
			Title:    "Missing variable",
			Message:  "variable `base_url` is not defined",
			File:     "tests/corpus/markdown-valid/broken.md",
			Step:     "step_0",
		}},
	})

	encoded, err := report.Encode()
	if err != nil {
		t.Fatalf("Encode failed: %v", err)
	}
	text := string(encoded)

	// Compact, single line, keys in struct order.
	if strings.ContainsAny(text, "\n\t") {
		t.Fatalf("Encode output is not compact: %q", text)
	}
	if !strings.HasPrefix(text, `{"schema_version":"1","curl_version":"8.21","documents":[`) {
		t.Fatalf("Encode prefix = %q, want schema_version, curl_version, documents in struct order", text)
	}
	if want := `"path":"tests/e2e/01-health-status.md","status":"passed","duration_ms":42,"steps":[{"name":"health","status":"passed","duration_ms":41,"method":"GET","url":"http://127.0.0.1:39170/health","status_code":200,"attempt":1` +
		`}],"diagnostics":[]`; !strings.Contains(text, want) {
		t.Fatalf("Encode document shape mismatch:\n got %s\nwant substring %s", text, want)
	}
	if !strings.Contains(text, `"severity":"error","code":"MDOK-E401","title":"Missing variable","message":"variable `+"`base_url`"+` is not defined"`) {
		t.Fatalf("Encode diagnostic shape mismatch: %s", text)
	}
	if want := `"steps":[],"diagnostics":[{"severity"`; !strings.Contains(text, want) {
		t.Fatalf("nil steps slice must serialize as []: %s", text)
	}

	// Round trip through the shared contract type.
	var decoded core.Report
	if err := json.Unmarshal(encoded, &decoded); err != nil {
		t.Fatalf("round-trip unmarshal failed: %v", err)
	}
	if decoded.SchemaVersion != "1" || decoded.CurlVersion != "8.21" {
		t.Fatalf("round-trip header = %+v", decoded)
	}
	if len(decoded.Documents) != 2 {
		t.Fatalf("round-trip documents = %d, want 2", len(decoded.Documents))
	}
	first := decoded.Documents[0]
	if first.Path != "tests/e2e/01-health-status.md" || first.Status != "passed" || first.DurationMS != 42 || len(first.Steps) != 1 {
		t.Fatalf("round-trip first document = %+v", first)
	}
	step := first.Steps[0]
	if step.Name != "health" || step.StatusCode != 200 || step.Method != "GET" || step.Attempt != 1 {
		t.Fatalf("round-trip step = %+v", step)
	}
	second := decoded.Documents[1]
	if len(second.Steps) != 0 || len(second.Diagnostics) != 1 {
		t.Fatalf("round-trip second document = %+v", second)
	}
	diagnostic := second.Diagnostics[0]
	if diagnostic.Code != "MDOK-E401" || diagnostic.Severity != core.SeverityError || diagnostic.Step != "step_0" {
		t.Fatalf("round-trip diagnostic = %+v", diagnostic)
	}
}

func TestEncodeOmitsZeroStepOptionals(t *testing.T) {
	report := New("8.21")
	report.AddDocument(core.DocumentResult{
		Path:   "workflow.md",
		Status: "error",
		Steps:  []core.StepResult{{Name: "request", Status: "error"}},
	})
	encoded, err := report.Encode()
	if err != nil {
		t.Fatalf("Encode failed: %v", err)
	}
	for _, omitted := range []string{`"method"`, `"url"`, `"status_code"`, `"attempt"`, `"redirect_count"`} {
		if strings.Contains(string(encoded), omitted) {
			t.Fatalf("zero-valued optional %s should be omitted: %s", omitted, encoded)
		}
	}
	if !strings.Contains(string(encoded), `{"name":"request","status":"error","duration_ms":0}`) {
		t.Fatalf("unexpected step shape: %s", encoded)
	}
}

func TestExitCode(t *testing.T) {
	tests := []struct {
		name     string
		statuses []string
		mode     string
		want     int
	}{
		{"all passed test", []string{"passed", "passed"}, "test", 0},
		{"all passed lint", []string{"passed"}, "lint", 0},
		{"empty run lint", nil, "lint", 0},
		{"skipped is not a failure", []string{"passed", "skipped"}, "test", 0},
		{"planned is not a failure", []string{"planned", "planned"}, "plan", 0},
		{"failed document in test mode", []string{"passed", "failed"}, "test", 1},
		{"error document in test mode", []string{"error"}, "test", 1},
		{"failed document in lint mode", []string{"failed"}, "lint", 2},
		{"error document in lint mode", []string{"passed", "error"}, "lint", 2},
		{"failed document in plan mode", []string{"failed"}, "plan", 2},
		// The corpus-lint goldens record exit 2 for a lint run whose only
		// document failed with an error diagnostic.
		{"corpus lint golden shape", []string{"failed"}, "lint", 2},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := ExitCode(test.statuses, test.mode); got != test.want {
				t.Fatalf("ExitCode(%v, %q) = %d, want %d", test.statuses, test.mode, got, test.want)
			}
		})
	}
}
