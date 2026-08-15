package markdown

import (
	"errors"
	"reflect"
	"strings"
	"testing"

	"mdok/core"
)

func mustParse(t *testing.T, source string) *core.Document {
	t.Helper()
	document, err := Parse("test.md", []byte(source))
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}
	return document
}

func wantCode(t *testing.T, source []byte, code string) {
	t.Helper()
	_, err := Parse("test.md", source)
	if err == nil {
		t.Fatalf("Parse expected %s error, got success", code)
	}
	var markdownErr *Error
	if !errors.As(err, &markdownErr) {
		t.Fatalf("Parse expected *markdown.Error, got %T", err)
	}
	if markdownErr.Diagnostic.Code != code {
		t.Fatalf("Parse expected %s, got %s (%s)", code, markdownErr.Diagnostic.Code, markdownErr.Diagnostic.Message)
	}
}

// Ports extracts_only_marked_fences_and_tracks_headings.
func TestExtractsOnlyMarkedFences(t *testing.T) {
	source := "# Users\n\n```text\ncurl ignored\n```\n\n```curl mdok name=get_user\ncurl https://example.test\n```\n"
	document := mustParse(t, source)
	if len(document.Items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(document.Items))
	}
	request, ok := document.Items[0].(*core.CurlItem)
	if !ok {
		t.Fatalf("expected *core.CurlItem, got %T", document.Items[0])
	}
	if request.Name != "get_user" {
		t.Fatalf("name: got %q", request.Name)
	}
	if request.Source != "curl https://example.test\n" {
		t.Fatalf("source: got %q", request.Source)
	}
	// Headings are parsed without breaking fence tracking.
	if document.Path != "test.md" {
		t.Fatalf("path: got %q", document.Path)
	}
}

// Ports parses_variables_and_plan_associations.
func TestParsesVariablesAndPlanAssociations(t *testing.T) {
	source := "```toml mdok vars\nbase = \"https://example.test\"\n```\n" +
		"```curl mdok name=one\ncurl {{base}}\n```\n" +
		"```jmespath mdok check=one\nstatus == `200`\n```\n"
	document := mustParse(t, source)
	if document.Vars["base"] != "https://example.test" {
		t.Fatalf("vars: got %#v", document.Vars)
	}
	if len(document.Items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(document.Items))
	}
	request := document.Items[0].(*core.CurlItem)
	if request.Name != "one" || request.Source != "curl {{base}}\n" {
		t.Fatalf("request: %+v", request)
	}
	check := document.Items[1].(*core.CheckItem)
	if check.Step != "one" || !reflect.DeepEqual(check.Lines, []string{"status == `200`"}) {
		t.Fatalf("check: %+v", check)
	}
}

// Ports classifies_exec_and_preserves_checks_and_captures_in_core_plan,
// using a curl step because the Go contract has no exec item type.
func TestPreservesChecksAndCaptures(t *testing.T) {
	source := "# Agent tools\n\n```curl mdok name=validate\nprintf '{\"ok\":true}'\n```\n\n" +
		"```jmespath mdok check=validate\nsuccess == `true`\n```\n\n" +
		"```jmespath mdok capture=validate\n{tool_ok: stdout_json.ok}\n```\n"
	document := mustParse(t, source)
	if len(document.Items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(document.Items))
	}
	request := document.Items[0].(*core.CurlItem)
	if request.Name != "validate" || request.Source != "printf '{\"ok\":true}'\n" {
		t.Fatalf("request: %+v", request)
	}
	check := document.Items[1].(*core.CheckItem)
	if check.Step != "validate" || !reflect.DeepEqual(check.Lines, []string{"success == `true`"}) {
		t.Fatalf("check: %+v", check)
	}
	capture := document.Items[2].(*core.CaptureItem)
	if capture.Step != "validate" || capture.Expr != "{tool_ok: stdout_json.ok}" {
		t.Fatalf("capture: %+v", capture)
	}
}

// Ports heading_context_is_updated_without_affecting_later_blocks: heading
// levels change across fences without disturbing extraction.
func TestHeadingContextDoesNotAffectLaterBlocks(t *testing.T) {
	source := "# API\n\n## Users\n\n```curl mdok name=list_users\ncurl https://example.test/users\n```\n\n" +
		"### Details\n\n```curl mdok name=get_user\ncurl https://example.test/users/1\n```\n\n" +
		"## Teams\n\n```curl mdok name=list_teams\ncurl https://example.test/teams\n```\n"
	document := mustParse(t, source)
	var names []string
	for _, item := range document.Items {
		names = append(names, item.(*core.CurlItem).Name)
	}
	if !reflect.DeepEqual(names, []string{"list_users", "get_user", "list_teams"}) {
		t.Fatalf("names: got %q", names)
	}
}

func TestTildeIndentedAndNestedFences(t *testing.T) {
	source := "~~~curl mdok name=tilde\ncurl https://example.test/a\n~~~\n"
	document := mustParse(t, source)
	if got := document.Items[0].(*core.CurlItem).Source; got != "curl https://example.test/a\n" {
		t.Fatalf("tilde fence body: got %q", got)
	}

	// Indented fences (up to three spaces) dedent their bodies.
	source = "  ```curl mdok name=indented\n  curl https://example.test/b\n  ```\n"
	document = mustParse(t, source)
	if got := document.Items[0].(*core.CurlItem).Source; got != "curl https://example.test/b\n" {
		t.Fatalf("indented fence body: got %q", got)
	}

	// Four leading spaces make an indented code block, not a fence.
	source = "    ```curl mdok name=notafence\n    curl https://example.test/c\n    ```\n"
	document = mustParse(t, source)
	if len(document.Items) != 0 {
		t.Fatalf("expected no items, got %d", len(document.Items))
	}

	// Longer opening fences need equally long closing fences.
	source = "````curl mdok name=long\ninner ``` fence\n````\n"
	document = mustParse(t, source)
	if got := document.Items[0].(*core.CurlItem).Source; got != "inner ``` fence\n" {
		t.Fatalf("nested fence body: got %q", got)
	}
}

// Ports metadata_rejects_duplicates_and_bad_quotes.
func TestMetadataRejectsDuplicatesAndBadQuotes(t *testing.T) {
	wantCode(t, []byte("```curl mdok name=a name=b\ncurl x\n```\n"), "MDOK-E100")
	wantCode(t, []byte("```curl mdok name=\"a\ncurl x\n```\n"), "MDOK-E100")
	wantCode(t, []byte("```toml mdok vars extra\ncurl x\n```\n"), "MDOK-E100")
	wantCode(t, []byte("```toml mdok vars name=x\nx = 1\n```\n"), "MDOK-E100")
	wantCode(t, []byte("```jmespath mdok check=a capture=b\nx\n```\n"), "MDOK-E100")
	wantCode(t, []byte("```jmespath mdok\nx\n```\n"), "MDOK-E100")
	wantCode(t, []byte("```python mdok name=x\nprint(1)\n```\n"), "MDOK-E100")
	// The exec fence is recognized by Rust but outside the Go contract's
	// item types, so it is reported as unsupported here.
	wantCode(t, []byte("```exec mdok name=tool\nprintf ok\n```\n"), "MDOK-E100")
	// mdok marker present but malformed metadata.
	wantCode(t, []byte("```mdok\ncurl x\n```\n"), "MDOK-E100")
	wantCode(t, []byte("```name=x mdok curl\ncurl x\n```\n"), "MDOK-E100")
	// Quoted attribute values parse; a space still makes the step name
	// invalid, which is reported as E101 rather than a metadata error.
	wantCode(t, []byte("```curl mdok name='quoted step'\ncurl x\n```\n"), "MDOK-E101")
	if document := mustParse(t, "```curl mdok name='get-user_1'\ncurl x\n```\n"); len(document.Items) != 1 {
		t.Fatal("quoted attribute value should parse")
	}
}

func TestStepNameRules(t *testing.T) {
	// Invalid step names are rejected (must start with a letter).
	wantCode(t, []byte("```curl mdok name=1bad\ncurl x\n```\n"), "MDOK-E101")
	// Reserved names are rejected.
	wantCode(t, []byte("```curl mdok name=steps\ncurl x\n```\n"), "MDOK-E101")
	// Overly long names are rejected.
	long := strings.Repeat("a", maxStepNameBytes+1)
	wantCode(t, []byte("```curl mdok name="+long+"\ncurl x\n```\n"), "MDOK-E101")
	// Duplicate names are rejected.
	wantCode(t, []byte("```curl mdok name=dup\ncurl x\n```\n```curl mdok name=dup\ncurl y\n```\n"), "MDOK-E101")
	// Same for check/capture targets.
	wantCode(t, []byte("```curl mdok name=ok\ncurl x\n```\n```jmespath mdok check=1bad\nx\n```\n"), "MDOK-E101")
}

func TestUnknownStepReference(t *testing.T) {
	// Checks and captures must follow their request in document order.
	wantCode(t, []byte("```jmespath mdok check=ghost\nx\n```\n"), "MDOK-E102")
	source := "```jmespath mdok check=later\nx\n```\n```curl mdok name=later\ncurl x\n```\n"
	wantCode(t, []byte(source), "MDOK-E102")
	wantCode(t, []byte("```curl mdok name=a\ncurl x\n```\n```jmespath mdok capture=other\n{k: v}\n```\n"), "MDOK-E102")
}

func TestVariablesBlockErrors(t *testing.T) {
	wantCode(t, []byte("```toml mdok vars\nthis is not toml\n```\n"), "MDOK-E110")
	wantCode(t, []byte("```toml mdok vars\na = 1\n```\n```toml mdok vars\na = 2\n```\n"), "MDOK-E110")
	// Empty capture expressions reuse the variables error code in Rust.
	wantCode(t, []byte("```curl mdok name=a\ncurl x\n```\n```jmespath mdok capture=a\n\n```\n"), "MDOK-E110")
}

func TestInvalidUTF8(t *testing.T) {
	wantCode(t, []byte{0x7f, 0xff, 0xfe}, "MDOK-E001")
}

// Ports rejects_oversized_source_before_parsing.
func TestRejectsOversizedSource(t *testing.T) {
	source := []byte(strings.Repeat("x", MaxSourceBytes+1))
	_, err := Parse("oversized.md", source)
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "source is") {
		t.Fatalf("message: %v", err)
	}
}

// Ports rejects_fence_and_step_budget_overflows.
func TestRejectsFenceAndStepBudgetOverflows(t *testing.T) {
	fenced := strings.Repeat("```text\nignored\n```\n", MaxFences+1)
	_, err := Parse("fences.md", []byte(fenced))
	if err == nil || !strings.Contains(err.Error(), "fenced code blocks") {
		t.Fatalf("fences: %v", err)
	}

	var steps strings.Builder
	for i := 0; i <= MaxSteps; i++ {
		steps.WriteString("```curl mdok name=step-" + string(rune('a'+i%26)) + string(rune('0'+i/26%10)) + string(rune('0'+i%10)) + "\ncurl https://example.test\n```\n")
	}
	_, err = Parse("steps.md", []byte(steps.String()))
	if err == nil || !strings.Contains(err.Error(), "steps") {
		t.Fatalf("steps: %v", err)
	}
}

// Ports rejects_ast_budget_overflow (each heading approximates two nodes).
func TestRejectsASTBudgetOverflow(t *testing.T) {
	var source strings.Builder
	for i := 0; i < MaxASTNodes/2+1; i++ {
		source.WriteString("# heading\n")
	}
	_, err := Parse("ast.md", []byte(source.String()))
	if err == nil || !strings.Contains(err.Error(), "AST") {
		t.Fatalf("ast: %v", err)
	}
}

func TestCheckAndCaptureLimits(t *testing.T) {
	var body strings.Builder
	for i := 0; i < MaxChecksPerStep+1; i++ {
		body.WriteString("status == `200`\n")
	}
	source := "```curl mdok name=s\ncurl x\n```\n```jmespath mdok check=s\n" + body.String() + "```\n"
	_, err := Parse("checks.md", []byte(source))
	if err == nil || !strings.Contains(err.Error(), "checks") {
		t.Fatalf("checks: %v", err)
	}

	captures := strings.Repeat("```jmespath mdok capture=s\n{k: v}\n```\n", MaxCapturesPerStep+1)
	source = "```curl mdok name=s\ncurl x\n```\n" + captures
	_, err = Parse("captures.md", []byte(source))
	if err == nil || !strings.Contains(err.Error(), "captures") {
		t.Fatalf("captures: %v", err)
	}
}

func TestEmptyDocumentAndNoVarsFences(t *testing.T) {
	document := mustParse(t, "# Just a heading\n\nSome prose.\n")
	if document.Vars == nil || len(document.Vars) != 0 {
		t.Fatalf("vars should be non-nil and empty: %#v", document.Vars)
	}
	if len(document.Items) != 0 {
		t.Fatalf("items: %d", len(document.Items))
	}
}

func TestTOMLValueTypes(t *testing.T) {
	source := "```toml mdok vars\n" +
		"host = \"example.test\"\nport = 8443\nratio = 2.5\ndebug = true\ntags = [\"a\", \"b\"]\n" +
		"[db]\nname = \"main\"\n" +
		"```\n"
	document := mustParse(t, source)
	vars := document.Vars
	if vars["host"] != "example.test" || vars["debug"] != true {
		t.Fatalf("scalars: %#v", vars)
	}
	if port, ok := vars["port"].(int64); !ok || port != 8443 {
		t.Fatalf("port: %#v", vars["port"])
	}
	if ratio, ok := vars["ratio"].(float64); !ok || ratio != 2.5 {
		t.Fatalf("ratio: %#v", vars["ratio"])
	}
	if !reflect.DeepEqual(vars["tags"], []any{"a", "b"}) {
		t.Fatalf("tags: %#v", vars["tags"])
	}
	if db, ok := vars["db"].(map[string]any); !ok || db["name"] != "main" {
		t.Fatalf("db: %#v", vars["db"])
	}
}

func TestCheckLinesAreTrimmedAndNonEmpty(t *testing.T) {
	source := "```curl mdok name=a\ncurl x\n```\n```jmespath mdok check=a\n\n  status == `200`  \n\nbody.ok == `true`\n```\n"
	document := mustParse(t, source)
	check := document.Items[1].(*core.CheckItem)
	if !reflect.DeepEqual(check.Lines, []string{"status == `200`", "body.ok == `true`"}) {
		t.Fatalf("lines: %q", check.Lines)
	}
}

func TestDiagnosticOfHelper(t *testing.T) {
	_, err := Parse("test.md", []byte("```curl mdok name=dup\ncurl x\n```\n```curl mdok name=dup\ncurl y\n```\n"))
	diag, ok := DiagnosticOf(err)
	if !ok || diag.Code != "MDOK-E101" || diag.Title != "Markdown planning error" {
		t.Fatalf("got %+v ok=%v", diag, ok)
	}
	if !strings.Contains(err.Error(), "MDOK-E101") {
		t.Fatalf("Error() should carry the code: %v", err)
	}
}
