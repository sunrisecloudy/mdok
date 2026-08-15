// Package core defines the shared contracts between mdok pipeline stages:
// markdown extraction, shell tokenizing, template expansion, curl planning,
// HTTP execution, JMESPath checks, and report assembly.
package core

import "time"

// Severity of a diagnostic.
type Severity string

const (
	SeverityError   Severity = "error"
	SeverityWarning Severity = "warning"
)

// Diagnostic is one MDOK-Exxx finding attached to a document or step.
type Diagnostic struct {
	Severity Severity `json:"severity"`
	Code     string   `json:"code"`
	Title    string   `json:"title"`
	Message  string   `json:"message"`
	File     string   `json:"file"`
	Step     string   `json:"step"`
}

// CurlItem is a ```curl mdok name=STEP``` fence body (raw, unexpanded).
type CurlItem struct {
	Name   string
	Source string
}

// CheckItem is a ```jmespath mdok check=STEP``` fence; each line is a
// boolean JMESPath expression that must evaluate truthy.
type CheckItem struct {
	Step  string
	Lines []string
}

// CaptureItem is a ```jmespath mdok capture=STEP``` fence; the expression
// must evaluate to a JSON object merged into the variables for later steps.
type CaptureItem struct {
	Step string
	Expr string
}

// Document is one parsed Markdown workflow: TOML variables plus the ordered
// stream of executable items.
type Document struct {
	Path  string
	Vars  map[string]any
	Items []any // *CurlItem | *CheckItem | *CaptureItem
}

// KV is an ordered header or cookie pair.
type KV struct {
	Key   string
	Value string
}

// ExecConfig is the effective runtime configuration after merging mdok.toml
// and CLI flags.
type ExecConfig struct {
	AllowedHosts    []string
	AllowedSchemes  []string
	AllowedReadPaths []string
	ConnectTimeout  time.Duration
	TotalTimeout    time.Duration
}

// ContainsHost reports whether host is permitted (exact match; the e2e
// surface only needs exact hosts such as 127.0.0.1 and localhost).
func (c *ExecConfig) ContainsHost(host string) bool {
	for _, allowed := range c.AllowedHosts {
		if allowed == host || allowed == "*" {
			return true
		}
	}
	return false
}

// ContainsScheme reports whether scheme is permitted.
func (c *ExecConfig) ContainsScheme(scheme string) bool {
	for _, allowed := range c.AllowedSchemes {
		if allowed == scheme {
			return true
		}
	}
	return false
}

// Timings captures transfer durations.
type Timings struct {
	TotalMS float64 `json:"total_ms"`
}

// Transfer is the outcome of one executed curl step.
type Transfer struct {
	Status        int     `json:"status"`
	Body          []byte  `json:"body"`
	Headers       []KV    `json:"headers,omitempty"`
	RedirectCount int     `json:"redirect_count,omitempty"`
	Attempt       int     `json:"attempt,omitempty"`
	Timings       Timings `json:"timings"`
}

// StepResult captures one executed curl step for the report.
type StepResult struct {
	Name          string        `json:"name"`
	Status        string        `json:"status"`
	DurationMS    int64         `json:"duration_ms"`
	Method        string        `json:"method,omitempty"`
	URL           string        `json:"url,omitempty"`
	StatusCode    int           `json:"status_code,omitempty"`
	Attempt       int           `json:"attempt,omitempty"`
	RedirectCount int           `json:"redirect_count,omitempty"`
}

// DocumentResult is the outcome of lint or test for one document.
type DocumentResult struct {
	Path        string        `json:"path"`
	Status      string        `json:"status"`
	DurationMS  int64         `json:"duration_ms"`
	Steps       []StepResult  `json:"steps"`
	Diagnostics []Diagnostic  `json:"diagnostics"`

	// ExitClass mirrors the Rust exit taxonomy (not serialized): 0 pass,
	// 1 assertion/step failure, 2 plan/static error, 3 transfer/policy error.
	ExitClass int `json:"-"`
}

// Report is the top-level JSON report emitted for --json modes.
type Report struct {
	SchemaVersion string           `json:"schema_version"`
	CurlVersion   string           `json:"curl_version"`
	Documents     []DocumentResult `json:"documents"`
}
