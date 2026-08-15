package shell

import (
	"errors"
	"reflect"
	"strings"
	"testing"

	"mdok/template"
)

func mustParse(t *testing.T, source string) []string {
	t.Helper()
	argv, err := ParseCurlSource(source)
	if err != nil {
		t.Fatalf("ParseCurlSource(%q) failed: %v", source, err)
	}
	return argv
}

func wantCode(t *testing.T, source, code string) {
	t.Helper()
	_, err := ParseCurlSource(source)
	if err == nil {
		t.Fatalf("ParseCurlSource(%q) expected %s error, got success", source, code)
	}
	var shellErr *Error
	if !errors.As(err, &shellErr) {
		t.Fatalf("ParseCurlSource(%q) expected *shell.Error, got %T", source, err)
	}
	if shellErr.Code != code {
		t.Fatalf("ParseCurlSource(%q) expected %s, got %s (%s)", source, code, shellErr.Code, shellErr.Message)
	}
}

// Ports parses_quoted_words_and_templates_without_retokenizing_values:
// templates stay as literal word content; expansion is a separate step that
// must not re-tokenize the rendered values.
func TestParsesQuotedWordsAndTemplatesWithoutRetokenizingValues(t *testing.T) {
	argv := mustParse(t, `curl --header "X-Name: {{name|header}}" "{{base}}/me"`)
	want := []string{"curl", "--header", "X-Name: {{name|header}}", "{{base}}/me"}
	if !reflect.DeepEqual(argv, want) {
		t.Fatalf("raw argv: got %q, want %q", argv, want)
	}
	vars := map[string]any{
		"name": `W "Admin"`,
		"base": "https://example.test",
	}
	expanded := make([]string, len(argv))
	for i, arg := range argv {
		value, diag := template.Expand(arg, vars)
		if diag != nil {
			t.Fatalf("expand %q: %s", arg, diag.Message)
		}
		expanded[i] = value
	}
	wantExpanded := []string{"curl", "--header", `X-Name: W "Admin"`, "https://example.test/me"}
	if !reflect.DeepEqual(expanded, wantExpanded) {
		t.Fatalf("expanded argv: got %q, want %q", expanded, wantExpanded)
	}
}

// Ports rejects_shell_operators_expansions_and_multiple_commands.
func TestRejectsShellOperatorsExpansionsAndMultipleCommands(t *testing.T) {
	for _, source := range []string{
		"curl x | jq .",
		"curl $(touch /tmp/x)",
		"curl x; echo bad",
		"curl x > out",
	} {
		wantCode(t, source, "MDOK-E201")
	}
	// A leading assignment inside a word is rejected; an assignment in the
	// first word (CURL=x) merely fails the literal-curl check (E202), which
	// still fails the parse exactly like the Rust test asserts.
	wantCode(t, "=x curl y", "MDOK-E201")
	if _, err := ParseCurlSource("CURL=x curl y"); err == nil {
		t.Fatal("CURL=x curl y must be rejected")
	}
}

// Ports supports_backslash_continuations.
func TestSupportsBackslashContinuations(t *testing.T) {
	argv := mustParse(t, "curl --url https://example.test/\\\nusers")
	if argv[2] != "https://example.test/users" {
		t.Fatalf("got %q", argv[2])
	}
}

// The e2e workflows join long curl commands with backslash-newline
// continuations inside a fence body that ends with a newline; only middle
// lines carry the continuation backslash.
func TestMultiLineFenceBodyWithContinuations(t *testing.T) {
	source := "curl --request POST \"{{base_url}}/auth/login\" \\\n  --header \"Content-Type: application/json\" \\\n  --data-raw '{\"email\":{{email|json}}}'\n"
	argv := mustParse(t, source)
	want := []string{
		"curl", "--request", "POST", "{{base_url}}/auth/login",
		"--header", "Content-Type: application/json",
		"--data-raw", `{"email":{{email|json}}}`,
	}
	if !reflect.DeepEqual(argv, want) {
		t.Fatalf("got %q, want %q", argv, want)
	}
}

// Ports bounds_argv_count_before_evaluation.
func TestBoundsArgvCountBeforeEvaluation(t *testing.T) {
	// Exactly MaxArgvArguments words is allowed.
	exact := "curl" + strings.Repeat(" x", MaxArgvArguments-1)
	if _, err := ParseCurlSource(exact); err != nil {
		t.Fatalf("64 arguments should be allowed: %v", err)
	}
	// One more word is over the limit.
	over := "curl" + strings.Repeat(" x", MaxArgvArguments)
	wantCode(t, over, "MDOK-E405")
}

func TestQuoteHandling(t *testing.T) {
	// Empty quotes still produce an empty argument.
	if got := mustParse(t, `curl "" x`); !reflect.DeepEqual(got, []string{"curl", "", "x"}) {
		t.Fatalf("empty double quotes: got %q", got)
	}
	if got := mustParse(t, `curl ''`); !reflect.DeepEqual(got, []string{"curl", ""}) {
		t.Fatalf("empty single quotes: got %q", got)
	}
	// Single quotes keep every character literal, including shell specials.
	if got := mustParse(t, `curl -H 'a$b;c|d'`); got[2] != "a$b;c|d" {
		t.Fatalf("single quote specials: got %q", got[2])
	}
	// Double quotes escape only " \ $ ` and line continuations.
	if got := mustParse(t, `curl -H "a\"b\\c"`); got[2] != `a"b\c` {
		t.Fatalf("double quote escapes: got %q", got[2])
	}
	if got := mustParse(t, `curl -H "a\zb"`); got[2] != `a\zb` {
		t.Fatalf("double quote keeps unknown escapes: got %q", got[2])
	}
	// Continuations work inside double quotes too.
	if got := mustParse(t, "curl -H \"a\\\nb\""); got[2] != "ab" {
		t.Fatalf("double quote continuation: got %q", got[2])
	}
	// Backslash makes unquoted specials literal.
	if got := mustParse(t, `curl a\;b`); got[1] != "a;b" {
		t.Fatalf("escaped semicolon: got %q", got[1])
	}
	// Newlines are fine inside single quotes.
	if got := mustParse(t, "curl 'a\nb'"); got[1] != "a\nb" {
		t.Fatalf("newline in single quotes: got %q", got[1])
	}
}

func TestWordSyntaxErrors(t *testing.T) {
	wantCode(t, "curl 'unterminated", "MDOK-E200")
	wantCode(t, `curl "unterminated`, "MDOK-E200")
	wantCode(t, `curl -H "trailing\`, "MDOK-E200")
	wantCode(t, `curl trailing\`, "MDOK-E200")
	wantCode(t, "curl x\n--y", "MDOK-E201") // unescaped newline
}

func TestNotExactlyOneCurlCommand(t *testing.T) {
	wantCode(t, "", "MDOK-E202")
	wantCode(t, "   ", "MDOK-E202")
	wantCode(t, "wget https://example.test", "MDOK-E202")
	wantCode(t, "{{host}}curl x", "MDOK-E202") // first word is not the literal curl
	wantCode(t, "Curl x", "MDOK-E202")
}

func TestTemplatesValidatedButKeptLiteral(t *testing.T) {
	wantCode(t, "curl {{unclosed", "MDOK-E400")
	wantCode(t, "curl {{x|wat}}", "MDOK-E400")
	wantCode(t, "curl {{}}", "MDOK-E400")
	// A valid template inside single quotes is still recognized as template
	// text (the Rust tokenizer checks {{ before quote state).
	if got := mustParse(t, "curl '{{x}}'"); got[1] != "{{x}}" {
		t.Fatalf("got %q", got[1])
	}
}

func TestComments(t *testing.T) {
	argv := mustParse(t, "curl https://example.test # trailing note")
	want := []string{"curl", "https://example.test"}
	if !reflect.DeepEqual(argv, want) {
		t.Fatalf("got %q, want %q", argv, want)
	}
	wantCode(t, "curl x # note\nmore", "MDOK-E201")
	// A hash inside a word is plain content.
	if got := mustParse(t, "curl a#b"); got[1] != "a#b" {
		t.Fatalf("got %q", got[1])
	}
}

func TestIsCurl(t *testing.T) {
	if !IsCurl([]string{"curl", "x"}) {
		t.Fatal("curl argv should be a curl command")
	}
	if IsCurl(nil) || IsCurl([]string{"wget", "x"}) || IsCurl([]string{}) {
		t.Fatal("non-curl argv must be rejected")
	}
	if argv := mustParse(t, `curl -H "X: y"`); !IsCurl(argv) {
		t.Fatal("parsed argv should be a curl command")
	}
}

func TestDiagnosticHelper(t *testing.T) {
	_, err := ParseCurlSource("wget x")
	diag, ok := Diagnostic(err)
	if !ok || diag.Code != "MDOK-E202" || diag.Severity != "error" {
		t.Fatalf("Diagnostic: got %+v ok=%v", diag, ok)
	}
	if !strings.Contains(err.Error(), "MDOK-E202") {
		t.Fatalf("Error() should carry the code: %v", err)
	}
}
