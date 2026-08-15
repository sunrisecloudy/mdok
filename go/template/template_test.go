package template

import (
	"strings"
	"testing"
)

func mustExpand(t *testing.T, input string, vars map[string]any) string {
	t.Helper()
	value, diag := Expand(input, vars)
	if diag != nil {
		t.Fatalf("Expand(%q) failed: %s %s: %s", input, diag.Code, diag.Title, diag.Message)
	}
	return value
}

func wantDiag(t *testing.T, input string, vars map[string]any, code string) {
	t.Helper()
	_, diag := Expand(input, vars)
	if diag == nil {
		t.Fatalf("Expand(%q) expected %s diagnostic, got success", input, code)
	}
	if diag.Code != code {
		t.Fatalf("Expand(%q) expected %s, got %s (%s)", input, code, diag.Code, diag.Message)
	}
}

func sampleVars() map[string]any {
	return map[string]any{
		"user": map[string]any{
			"name": "A/B",
			"tags": []any{"x y"},
		},
	}
}

// Ports mdok-template's parses_nested_paths_and_renders_inside_literal_text.
func TestNestedPathsRenderInsideLiteralText(t *testing.T) {
	got := mustExpand(t, "/{{user.name}}/{{user.tags[0]|url}}", sampleVars())
	if got != "/A/B/x%20y" {
		t.Fatalf("got %q", got)
	}
}

// Ports filters_are_typed_and_header_is_safe.
func TestFiltersAreTypedAndHeaderIsSafe(t *testing.T) {
	if got := mustExpand(t, "{{user.name|base64}}", sampleVars()); got != "QS9C" {
		t.Fatalf("base64: got %q", got)
	}
	vars := map[string]any{"value": "ok\nInjected: yes"}
	wantDiag(t, "{{value|header}}", vars, "MDOK-E403")
}

// Ports rejects_bad_templates.
func TestRejectsBadTemplates(t *testing.T) {
	wantDiag(t, "{{missing", nil, "MDOK-E400")     // unclosed {{
	wantDiag(t, "{{value|wat}}", nil, "MDOK-E400") // unknown filter
	wantDiag(t, "}}leading", nil, "MDOK-E400")     // unmatched }}
	wantDiag(t, "{{}}", nil, "MDOK-E400")          // empty template
	wantDiag(t, "{{a|}}", nil, "MDOK-E400")        // empty filter name
	wantDiag(t, "{{a|b|c}}", nil, "MDOK-E400")     // two filters
	wantDiag(t, "{{1abc}}", nil, "MDOK-E400")      // path must start with identifier
	wantDiag(t, "{{user.tags[x]}}", nil, "MDOK-E400")
	wantDiag(t, "{{user.}}", nil, "MDOK-E400")
}

// Ports bounds_expansion_depth_and_rendered_bytes.
func TestBoundsExpansionDepthAndRenderedBytes(t *testing.T) {
	deep := "{{root" + strings.Repeat(".child", MaxExpansionDepth) + "}}"
	if _, diag := Parse(deep); diag == nil || diag.Code != "MDOK-E404" {
		t.Fatalf("expected MDOK-E404 for deep path, got %+v", diag)
	}
	exact := "{{root" + strings.Repeat(".child", MaxExpansionDepth-1) + "}}"
	if _, diag := Parse(exact); diag != nil {
		t.Fatalf("depth %d should be allowed: %s", MaxExpansionDepth, diag.Message)
	}
	vars := map[string]any{"value": strings.Repeat("x", MaxRenderedBytes+1)}
	wantDiag(t, "{{value}}", vars, "MDOK-E404")
}

func TestMissingVariable(t *testing.T) {
	wantDiag(t, "{{nope}}", sampleVars(), "MDOK-E401")
	wantDiag(t, "{{user.absent}}", sampleVars(), "MDOK-E401")
	wantDiag(t, "{{user.tags[7]|url}}", sampleVars(), "MDOK-E401")
	// Indexing into a non-array and keying into a scalar are missing lookups.
	wantDiag(t, "{{user.name[0]}}", sampleVars(), "MDOK-E401")
	wantDiag(t, "{{user.name.deep}}", sampleVars(), "MDOK-E401")
}

func TestURLEncoding(t *testing.T) {
	vars := map[string]any{"value": "space slash/plus+ไทย"}
	got := mustExpand(t, "{{value|url}}", vars)
	want := "space%20slash%2Fplus%2B%E0%B9%84%E0%B8%97%E0%B8%A2"
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
	// Unreserved characters pass through untouched.
	if got := mustExpand(t, "{{v|url}}", map[string]any{"v": "aZ09-._~"}); got != "aZ09-._~" {
		t.Fatalf("unreserved: got %q", got)
	}
	// Reserved punctuation is escaped.
	if got := mustExpand(t, "{{v|url}}", map[string]any{"v": "!#$&'()*,;=?:@[]^`{|}\"<"}); got != "%21%23%24%26%27%28%29%2A%2C%3B%3D%3F%3A%40%5B%5D%5E%60%7B%7C%7D%22%3C" {
		t.Fatalf("reserved: got %q", got)
	}
	// Percent itself and controls are escaped.
	if got := mustExpand(t, "{{v|url}}", map[string]any{"v": "100%\x01"}); got != "100%25%01" {
		t.Fatalf("percent: got %q", got)
	}
}

func TestFilterSemantics(t *testing.T) {
	vars := map[string]any{
		"query_value":  "hello world",
		"payload_name": "Ada",
		"count":        float64(42),
		"ratio":        float64(1.5),
		"big":          int64(7),
		"flag":         true,
		"nothing":      nil,
		"array":        []any{1, 2},
		"object":       map[string]any{"k": "v"},
	}
	cases := []struct {
		input string
		want  string
	}{
		{"{{query_value|string}}", "hello world"}, // string: raw, no quotes
		{"{{query_value}}", "hello world"},        // default filter is string
		{"{{query_value|raw}}", "hello world"},
		{"{{payload_name|json}}", `"Ada"`}, // json: quoted JSON literal
		{"{{count|json}}", "42"},
		{"{{count}}", "42"},
		{"{{ratio|json}}", "1.5"},
		{"{{big}}", "7"},
		{"{{flag}}", "true"},
		{"{{flag|json}}", "true"},
		{"{{nothing}}", "null"},
		{"{{nothing|json}}", "null"},
		{"{{count|url}}", "42"},
	}
	for _, tc := range cases {
		if got := mustExpand(t, tc.input, vars); got != tc.want {
			t.Errorf("%s: got %q, want %q", tc.input, got, tc.want)
		}
	}
	// Composite values are rejected by the scalar filters but fine as JSON.
	wantDiag(t, "{{array|string}}", vars, "MDOK-E402")
	wantDiag(t, "{{object|raw}}", vars, "MDOK-E402")
	wantDiag(t, "{{array|url}}", vars, "MDOK-E402")
	if got := mustExpand(t, "{{array|json}}", vars); got != "[1,2]" {
		t.Fatalf("array json: got %q", got)
	}
	if got := mustExpand(t, "{{object|json}}", vars); got != `{"k":"v"}` {
		t.Fatalf("object json: got %q", got)
	}
}

// The e2e workflow posts a JSON body with a template spliced mid-word.
func TestTemplateMidWordInsideJSONBody(t *testing.T) {
	vars := map[string]any{"payload_name": "Ada"}
	input := `'{"name":{{payload_name|json}},"active":true}'`
	if got := mustExpand(t, input, vars); got != `'{"name":"Ada","active":true}'` {
		t.Fatalf("got %q", got)
	}
}

func TestJSONStringEncoding(t *testing.T) {
	vars := map[string]any{"value": "he\"llo\\<line>\n\t\x01"}
	got := mustExpand(t, "{{value|json}}", vars)
	want := "\"he\\\"llo\\\\<line>\\n\\t\\u0001\""
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
	// Object keys are sorted, no HTML escaping, compact separators.
	vars = map[string]any{"value": map[string]any{"b": 1, "a": []any{true, nil, "x"}}}
	if got := mustExpand(t, "{{value|json}}", vars); got != `{"a":[true,null,"x"],"b":1}` {
		t.Fatalf("got %q", got)
	}
}

func TestBase64ByteArray(t *testing.T) {
	if got := mustExpand(t, "{{bytes|base64}}", map[string]any{"bytes": []any{104, 105}}); got != "aGk=" {
		t.Fatalf("got %q", got)
	}
	// Out-of-range bytes are rejected.
	wantDiag(t, "{{bytes|base64}}", map[string]any{"bytes": []any{300}}, "MDOK-E402")
	wantDiag(t, "{{bytes|base64}}", map[string]any{"bytes": 5}, "MDOK-E402")
}

func TestHeaderFilterAllowsPlainValues(t *testing.T) {
	vars := map[string]any{"token": "Bearer abc_123.-~+/="}
	if got := mustExpand(t, "{{token|header}}", vars); got != "Bearer abc_123.-~+/=" {
		t.Fatalf("got %q", got)
	}
	// Carriage returns are rejected like newlines.
	wantDiag(t, "{{v|header}}", map[string]any{"v": "a\rb"}, "MDOK-E403")
}

func TestMultipleTemplatesAndLiteralOnlyInput(t *testing.T) {
	vars := map[string]any{"a": "1", "b": "2"}
	if got := mustExpand(t, "{{a}}-{{b}}-{{a}}", vars); got != "1-2-1" {
		t.Fatalf("got %q", got)
	}
	if got := mustExpand(t, "plain text", vars); got != "plain text" {
		t.Fatalf("got %q", got)
	}
	if got := mustExpand(t, "", vars); got != "" {
		t.Fatalf("got %q", got)
	}
}

func TestAdjacentClosingBracesStayLiteral(t *testing.T) {
	vars := map[string]any{"a": "x"}
	if got := mustExpand(t, "{{a}}}", vars); got != "x}" {
		t.Fatalf("got %q", got)
	}
}

func TestParseParts(t *testing.T) {
	parts, diag := Parse("pre {{user.name|url}} post")
	if diag != nil {
		t.Fatalf("parse failed: %s", diag.Message)
	}
	if len(parts) != 3 {
		t.Fatalf("expected 3 parts, got %d", len(parts))
	}
	if parts[0].Literal != "pre " || parts[2].Literal != " post" {
		t.Fatalf("literals: %+v", parts)
	}
	expr := parts[1].Expr
	if expr == nil || len(expr.Path) != 2 || expr.Path[0].Key != "user" || expr.Path[1].Key != "name" || expr.Filter != FilterURL {
		t.Fatalf("expression: %+v", expr)
	}
}
