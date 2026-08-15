package jmespath

import (
	"errors"
	"strings"
	"testing"
)

// The roots below mirror the mdok-test-server fixtures the e2e corpus
// runs against: /health, /echo, /json/standard, /auth/bearer, /auth/login,
// /cookies/echo, /retry/2, and the /users CRUD endpoints.

func root(status int, body string, variables map[string]any) map[string]any {
	return NewRoot(&Transfer{Status: status}, []byte(body), variables).Map()
}

func healthRoot(status int) map[string]any {
	ok := status < 400
	return root(status, `{"ok":`+jsonBool(ok)+`}`, nil)
}

func jsonBool(value bool) string {
	if value {
		return "true"
	}
	return "false"
}

func checkCase(t *testing.T, expression string, evaluation map[string]any, want bool) {
	t.Helper()
	got, err := Check(expression, evaluation)
	if err != nil {
		t.Fatalf("Check(%q) returned error %v, want clean %v", expression, err, want)
	}
	if got != want {
		t.Fatalf("Check(%q) = %v, want %v", expression, got, want)
	}
}

func failingCase(t *testing.T, expression string, evaluation map[string]any, wantCode string) {
	t.Helper()
	passed, err := Check(expression, evaluation)
	if passed {
		t.Fatalf("Check(%q) passed, want failure %s", expression, wantCode)
	}
	var evaluationError *Error
	if !errors.As(err, &evaluationError) {
		t.Fatalf("Check(%q) error %v is %T, want *jmespath.Error", expression, err, err)
	}
	if evaluationError.Diagnostic.Code != wantCode {
		t.Fatalf("Check(%q) error code %q, want %q (message: %q)",
			expression, evaluationError.Diagnostic.Code, wantCode, evaluationError.Diagnostic.Message)
	}
	if evaluationError.Diagnostic.Severity != "error" {
		t.Fatalf("Check(%q) severity %q, want error", expression, evaluationError.Diagnostic.Severity)
	}
}

// TestE2ECheckExpressions exercises every check expression that appears
// in tests/e2e/*.md against realistic roots, on both the passing and the
// failing path.
func TestE2ECheckExpressions(t *testing.T) {
	tests := []struct {
		name       string
		expression string
		root       map[string]any
		failing    map[string]any
	}{
		// 01-health-status.md, 08-tls.md, combined health.
		{"status 200", "status == `200`", root(200, `{"ok":true}`, nil), root(500, `{"ok":false}`, nil)},
		{"body ok true", "body.ok == `true`", root(200, `{"ok":true}`, nil), root(200, `{"ok":false}`, nil)},

		// 02-template-query.md.
		{
			"query value round trip",
			"body.query.q == variables.query_value",
			root(200, `{"method":"GET","query":{"q":"space slash/plus+ไทย","page":"2"}}`,
				map[string]any{"query_value": "space slash/plus+ไทย"}),
			root(200, `{"method":"GET","query":{"q":"space%20slash%2Fplus%2B%E0%B9%84%E0%B8%97%E0%B8%A2","page":"2"}}`,
				map[string]any{"query_value": "space slash/plus+ไทย"}),
		},
		{"query page string literal", "body.query.page == '2'", root(200, `{"query":{"page":"2"}}`, nil), root(200, `{"query":{"page":"1"}}`, nil)},

		// 03-jmespath-capture.md.
		{
			"items length",
			"length(body.items) == `3`",
			root(200, `{"items":[{"id":"a"},{"id":"b"},{"id":"c"}]}`, nil),
			root(200, `{"items":[{"id":"a"},{"id":"b"}]}`, nil),
		},
		{
			"captured variable used",
			"body.query.captured == variables.captured_id",
			root(200, `{"query":{"captured":"b"}}`, map[string]any{"captured_id": "b"}),
			root(200, `{"query":{"captured":"a"}}`, map[string]any{"captured_id": "b"}),
		},

		// 04-bearer-auth.md.
		{"authenticated", "body.authenticated == `true`", root(200, `{"authenticated":true,"ok":true}`, nil), root(401, `{"authenticated":false}`, nil)},

		// 05-json-body.md.
		{"method raw literal", "body.method == 'POST'", root(200, `{"method":"POST"}`, nil), root(200, `{"method":"GET"}`, nil)},
		{
			"json body name",
			"body.json.name == variables.payload_name",
			root(200, `{"method":"POST","json":{"name":"Ada","active":true}}`, map[string]any{"payload_name": "Ada"}),
			root(200, `{"method":"POST","json":{"name":"Grace","active":true}}`, map[string]any{"payload_name": "Ada"}),
		},
		{"json body active", "body.json.active == `true`", root(200, `{"json":{"active":true}}`, nil), root(200, `{"json":{"active":false}}`, nil)},

		// 06-cookie-redirect.md.
		{
			"redirect count",
			"transfer.redirect_count == `1`",
			func() map[string]any {
				return NewRoot(&Transfer{Status: 200, RedirectCount: 1}, []byte(`{"cookies":{"fixture":"ok"}}`), nil).Map()
			}(),
			func() map[string]any {
				return NewRoot(&Transfer{Status: 200, RedirectCount: 0}, []byte(`{"cookies":{"fixture":"ok"}}`), nil).Map()
			}(),
		},
		{"cookie survives redirect", "body.cookies.fixture == 'ok'", root(200, `{"cookies":{"fixture":"ok"},"raw":"fixture=ok"}`, nil), root(200, `{"cookies":{},"raw":""}`, nil)},

		// 07-retry.md.
		{"retry attempt", "body.attempt == `3`", root(200, `{"ok":true,"attempt":3}`, nil), root(200, `{"ok":true,"attempt":2}`, nil)},

		// combined-workflow.md.
		{
			"login email",
			"body.user.email == variables.email",
			root(200, `{"access_token":"test-token","user":{"id":"user-abc","email":"combined-e2e@example.com"}}`,
				map[string]any{"email": "combined-e2e@example.com"}),
			root(200, `{"access_token":"test-token","user":{"id":"user-abc","email":"other@example.com"}}`,
				map[string]any{"email": "combined-e2e@example.com"}),
		},
		{"token type", "type(body.access_token) == 'string'", root(200, `{"access_token":"test-token"}`, nil), root(200, `{"access_token":null}`, nil)},
		{"created status", "status == `201`", root(201, `{"id":"combined-e2e-user"}`, nil), root(200, `{"id":"combined-e2e-user"}`, nil)},
		{"created id literal", "body.id == 'combined-e2e-user'", root(201, `{"id":"combined-e2e-user"}`, nil), root(201, `{"id":"other-user"}`, nil)},
		{
			"created email",
			"body.email == variables.email",
			root(201, `{"id":"combined-e2e-user","email":"combined-e2e@example.com"}`, map[string]any{"email": "combined-e2e@example.com"}),
			root(201, `{"id":"combined-e2e-user","email":"other@example.com"}`, map[string]any{"email": "combined-e2e@example.com"}),
		},
		{
			"resource id round trip",
			"body.id == variables.resource_id",
			root(200, `{"id":"combined-e2e-user","name":"Ada"}`, map[string]any{"resource_id": "combined-e2e-user"}),
			root(200, `{"id":"someone-else","name":"Ada"}`, map[string]any{"resource_id": "combined-e2e-user"}),
		},
		{"read name", "body.name == 'Ada'", root(200, `{"name":"Ada"}`, nil), root(200, `{"name":"Grace"}`, nil)},
		{"updated name with space", "body.name == 'Ada Lovelace'", root(200, `{"name":"Ada Lovelace"}`, nil), root(200, `{"name":"Ada"}`, nil)},
		{"deleted true", "body.deleted == `true`", root(200, `{"deleted":true,"id":"combined-e2e-user"}`, nil), root(200, `{"deleted":false,"id":"combined-e2e-user"}`, nil)},
	}

	for _, test := range tests {
		t.Run(test.name+" pass", func(t *testing.T) {
			checkCase(t, test.expression, test.root, true)
		})
		t.Run(test.name+" fail", func(t *testing.T) {
			failingCase(t, test.expression, test.failing, "MDOK-E502")
		})
	}
}

// TestHealthRootStatusVariants pins the smallest e2e file's two lines
// against both fixture outcomes.
func TestHealthRootStatusVariants(t *testing.T) {
	checkCase(t, "status == `200`", healthRoot(200), true)
	checkCase(t, "body.ok == `true`", healthRoot(200), true)
	failingCase(t, "status == `200`", healthRoot(500), "MDOK-E502")
	failingCase(t, "body.ok == `true`", healthRoot(500), "MDOK-E502")
}

// TestE2ECaptureExpressions exercises every capture expression in the
// e2e corpus and checks the merged values.
func TestE2ECaptureExpressions(t *testing.T) {
	sourceRoot := root(200, `{"ok":true,"items":[
		{"id":"a","name":"Alpha","value":1},
		{"id":"b","name":"Beta","value":2},
		{"id":"c","name":"Gamma","value":3}]}`, nil)

	captures, err := Capture("{captured_id: body.items[1].id}", sourceRoot)
	if err != nil {
		t.Fatalf("capture {captured_id: ...} failed: %v", err)
	}
	if len(captures) != 1 || captures["captured_id"] != "b" {
		t.Fatalf("captures = %v, want {captured_id: b}", captures)
	}

	loginRoot := root(200, `{"access_token":"test-token","token_type":"Bearer",
		"user":{"id":"user-abc","email":"combined-e2e@example.com"}}`, nil)
	captures, err = Capture("{access_token: body.access_token}", loginRoot)
	if err != nil {
		t.Fatalf("capture {access_token: ...} failed: %v", err)
	}
	if len(captures) != 1 || captures["access_token"] != "test-token" {
		t.Fatalf("captures = %v, want {access_token: test-token}", captures)
	}

	createRoot := root(201, `{"id":"combined-e2e-user","name":"Ada","email":"combined-e2e@example.com"}`, nil)
	captures, err = Capture("{resource_id: body.id}", createRoot)
	if err != nil {
		t.Fatalf("capture {resource_id: ...} failed: %v", err)
	}
	if len(captures) != 1 || captures["resource_id"] != "combined-e2e-user" {
		t.Fatalf("captures = %v, want {resource_id: combined-e2e-user}", captures)
	}

	// The dependent check from 03-jmespath-capture.md must pass once the
	// capture is merged into the variables.
	variables := map[string]any{}
	if err := MergeCaptures(variables, map[string]any{"captured_id": captures["resource_id"]}); err != nil {
		t.Fatalf("merge failed: %v", err)
	}
	useCaptureRoot := root(200, `{"query":{"captured":"combined-e2e-user"}}`, variables)
	checkCase(t, "body.query.captured == variables.captured_id", useCaptureRoot, true)
}

// TestCaptureFromSourceToDependentCheck replays the full 03 e2e flow:
// capture from /json/standard, merge, then check the dependent echo.
func TestCaptureFromSourceToDependentCheck(t *testing.T) {
	sourceRoot := root(200, `{"items":[{"id":"a"},{"id":"b"},{"id":"c"}]}`, nil)
	captures, err := Capture("{captured_id: body.items[1].id}", sourceRoot)
	if err != nil {
		t.Fatalf("capture failed: %v", err)
	}
	variables := map[string]any{}
	if err := MergeCaptures(variables, captures); err != nil {
		t.Fatalf("merge failed: %v", err)
	}
	echo := NewRoot(&Transfer{Status: 200}, []byte(`{"query":{"captured":"b"}}`), variables).Map()
	checkCase(t, "body.query.captured == variables.captured_id", echo, true)
}

func TestCaptureRejections(t *testing.T) {
	// Non-object results are MDOK-E503.
	for _, expression := range []string{
		"status",
		"body.items[1].id",
		"`[1, 2]`",
	} {
		_, err := Capture(expression, root(200, `{"items":[{"id":"a"},{"id":"b"}]}`, nil))
		var evaluationError *Error
		if !errors.As(err, &evaluationError) {
			t.Fatalf("Capture(%q) error %v is %T, want *jmespath.Error", expression, err, err)
		}
		if evaluationError.Diagnostic.Code != "MDOK-E503" {
			t.Fatalf("Capture(%q) code %q, want MDOK-E503", expression, evaluationError.Diagnostic.Code)
		}
		if evaluationError.Diagnostic.Title != "Invalid capture" {
			t.Fatalf("Capture(%q) title %q, want Invalid capture", expression, evaluationError.Diagnostic.Title)
		}
	}

	// Invalid keys are MDOK-E504 (empty, leading digit, space, 65 chars).
	for _, key := range []string{"", "9bad", "bad key", strings.Repeat("k", 65)} {
		expression := "`{\"" + key + "\": 1}`"
		_, err := Capture(expression, root(200, `{"ok":true}`, nil))
		var evaluationError *Error
		if !errors.As(err, &evaluationError) {
			t.Fatalf("Capture(%q) error %v is %T, want *jmespath.Error", expression, err, err)
		}
		if evaluationError.Diagnostic.Code != "MDOK-E504" {
			t.Fatalf("Capture key %q code %q, want MDOK-E504", key, evaluationError.Diagnostic.Code)
		}
	}

	// Compile failures stay MDOK-E500.
	for _, expression := range []string{"{unclosed: body.id", "", "   "} {
		_, err := Capture(expression, root(200, `{"id":"x"}`, nil))
		var evaluationError *Error
		if !errors.As(err, &evaluationError) {
			t.Fatalf("Capture(%q) error %v is %T, want *jmespath.Error", expression, err, err)
		}
		if evaluationError.Diagnostic.Code != "MDOK-E500" {
			t.Fatalf("Capture(%q) code %q, want MDOK-E500", expression, evaluationError.Diagnostic.Code)
		}
	}
}

func TestCheckErrorCodes(t *testing.T) {
	// Syntax errors are MDOK-E500, including the empty expression.
	for _, expression := range []string{"status ==", "`unclosed", "", "   "} {
		failingCase(t, expression, root(200, `{"ok":true}`, nil), "MDOK-E500")
	}

	// Non-boolean results are MDOK-E501 with the Rust crate's message.
	for _, test := range []struct {
		expression string
		root       map[string]any
		jsonType   string
	}{
		{"status", root(200, `{"ok":true}`, nil), "number"},
		{"body", root(200, `{"ok":true}`, nil), "object"},
		{"body.missing", root(200, `{"ok":true}`, nil), "null"},
		{"body.items", root(200, `{"items":[1]}`, nil), "array"},
		{"body.text", root(200, `{"text":"hello"}`, nil), "string"},
	} {
		passed, err := Check(test.expression, test.root)
		if passed {
			t.Fatalf("Check(%q) passed, want MDOK-E501", test.expression)
		}
		var evaluationError *Error
		if !errors.As(err, &evaluationError) {
			t.Fatalf("Check(%q) error %v is %T, want *jmespath.Error", test.expression, err, err)
		}
		if evaluationError.Diagnostic.Code != "MDOK-E501" {
			t.Fatalf("Check(%q) code %q, want MDOK-E501", test.expression, evaluationError.Diagnostic.Code)
		}
		if !strings.Contains(evaluationError.Diagnostic.Message, "check must return boolean, got "+test.jsonType) {
			t.Fatalf("Check(%q) message %q, want it to mention got %s", test.expression, evaluationError.Diagnostic.Message, test.jsonType)
		}
		if !strings.HasPrefix(evaluationError.Error(), "MDOK-E501 ") {
			t.Fatalf("Error() = %q, want MDOK-E501 prefix", evaluationError.Error())
		}
	}

	// Runtime failures surface as MDOK-E501 (length of null errors).
	failingCase(t, "length(body.items) == `3`", root(200, `{"items":null}`, nil), "MDOK-E501")

	// A false check is MDOK-E502 with the expression echoed back.
	passed, err := Check("status == `200`", root(404, `{"error":"missing"}`, nil))
	if passed {
		t.Fatal("Check passed, want MDOK-E502")
	}
	var evaluationError *Error
	if !errors.As(err, &evaluationError) {
		t.Fatalf("error %v is %T, want *jmespath.Error", err, err)
	}
	if evaluationError.Diagnostic.Code != "MDOK-E502" || evaluationError.Diagnostic.Title != "Check failed" {
		t.Fatalf("diagnostic %+v, want MDOK-E502 Check failed", evaluationError.Diagnostic)
	}
	if !strings.Contains(evaluationError.Diagnostic.Message, "status == `200`") {
		t.Fatalf("message %q, want the expression echoed", evaluationError.Diagnostic.Message)
	}
}

func TestMergeCaptures(t *testing.T) {
	target := map[string]any{"existing": "kept"}
	if err := MergeCaptures(target, map[string]any{"captured_id": "b", "count": float64(3)}); err != nil {
		t.Fatalf("merge failed: %v", err)
	}
	if target["existing"] != "kept" || target["captured_id"] != "b" || target["count"] != float64(3) {
		t.Fatalf("merged variables = %v", target)
	}

	err := MergeCaptures(target, map[string]any{"captured_id": "c"})
	var evaluationError *Error
	if !errors.As(err, &evaluationError) {
		t.Fatalf("collision error %v is %T, want *jmespath.Error", err, err)
	}
	if evaluationError.Diagnostic.Code != "MDOK-E504" {
		t.Fatalf("collision code %q, want MDOK-E504", evaluationError.Diagnostic.Code)
	}
	if target["captured_id"] != "b" {
		t.Fatalf("collision overwrote the variable: %v", target)
	}
}

func TestIsCaptureKey(t *testing.T) {
	for _, key := range []string{"a", "captured_id", "resource-id", "AccessToken", strings.Repeat("k", 64)} {
		if !IsCaptureKey(key) {
			t.Fatalf("IsCaptureKey(%q) = false, want true", key)
		}
	}
	for _, key := range []string{"", "9bad", "-bad", "_bad", "bad key", "bad.key", "bad/key", strings.Repeat("k", 65), "naïve"} {
		if IsCaptureKey(key) {
			t.Fatalf("IsCaptureKey(%q) = true, want false", key)
		}
	}
}

func TestNewRoot(t *testing.T) {
	transfer := &Transfer{
		Status:          200,
		Headers:         map[string][]string{"content-type": {"application/json"}, "x-duplicate": {"one", "two"}},
		RedirectCount:   1,
		Attempt:         3,
		DownloadedBytes: 512,
	}
	variables := map[string]any{"port": int64(39170), "ratio": float32(0.5)}
	body := []byte(`{"ok":true,"count":3}`)

	evaluation := NewRoot(transfer, body, variables).Map()

	if evaluation["status"] != float64(200) {
		t.Fatalf("status = %v (%T), want float64(200)", evaluation["status"], evaluation["status"])
	}
	parsed, ok := evaluation["body"].(map[string]any)
	if !ok {
		t.Fatalf("body = %T, want map[string]any", evaluation["body"])
	}
	if parsed["ok"] != true || parsed["count"] != float64(3) {
		t.Fatalf("body = %v", parsed)
	}
	headers, ok := evaluation["headers"].(map[string]any)
	if !ok {
		t.Fatalf("headers = %T, want map[string]any", evaluation["headers"])
	}
	if _, ok := headers["content-type"].([]string); !ok {
		t.Fatalf("headers[content-type] = %T, want []string", headers["content-type"])
	}
	transferMetrics, ok := evaluation["transfer"].(map[string]any)
	if !ok {
		t.Fatalf("transfer = %T, want map[string]any", evaluation["transfer"])
	}
	if transferMetrics["redirect_count"] != float64(1) || transferMetrics["attempt"] != float64(3) {
		t.Fatalf("transfer = %v", transferMetrics)
	}
	rootVariables, ok := evaluation["variables"].(map[string]any)
	if !ok {
		t.Fatalf("variables = %T, want map[string]any", evaluation["variables"])
	}
	// TOML decodes integers as int64; they must be normalized to float64.
	if rootVariables["port"] != float64(39170) {
		t.Fatalf("variables[port] = %v (%T), want float64(39170)", rootVariables["port"], rootVariables["port"])
	}
	if rootVariables["ratio"] != float64(0.5) {
		t.Fatalf("variables[ratio] = %v (%T), want float64(0.5)", rootVariables["ratio"], rootVariables["ratio"])
	}

	// The normalization quirk: int 200 does not equal the float64 200 of a
	// `200` JSON literal under go-jmespath's reflect.DeepEqual equality.
	if passed, err := Check("variables.port == `39170`", evaluation); !passed || err != nil {
		t.Fatalf("normalized variable comparison failed: %v", err)
	}

	// Non-JSON and empty bodies stay nil, like the Rust executor.
	for _, body := range []string{"", "   ", "<html>not json</html>"} {
		if parsed := NewRoot(transfer, []byte(body), nil).Body; parsed != nil {
			t.Fatalf("body %q parsed to %v, want nil", body, parsed)
		}
	}

	// A nil transfer yields a usable zeroed context.
	zeroed := NewRoot(nil, nil, nil).Map()
	if zeroed["status"] != float64(0) {
		t.Fatalf("status = %v, want 0", zeroed["status"])
	}
	zeroedTransfer := zeroed["transfer"].(map[string]any)
	if zeroedTransfer["redirect_count"] != float64(0) || zeroedTransfer["attempt"] != float64(0) {
		t.Fatalf("transfer = %v, want zeros", zeroedTransfer)
	}
}

func TestNormalizeExpression(t *testing.T) {
	tests := []struct {
		in   string
		want string
	}{
		// Valid JSON literals pass through untouched.
		{"status == `200`", "status == `200`"},
		{"length(body.items) == `3`", "length(body.items) == `3`"},
		{"body.ok == `true`", "body.ok == `true`"},
		{"`{\"a\": 1}`.a == `1`", "`{\"a\": 1}`.a == `1`"},
		// Raw string literals are untouched.
		{"body.method == 'POST'", "body.method == 'POST'"},
		{"type(body.access_token) == 'string'", "type(body.access_token) == 'string'"},
		// Bare identifiers inside backticks become quoted strings.
		{"body.name == `Ada-Lovelace`", "body.name == 'Ada-Lovelace'"},
		{"body.t == `null-ish`", "body.t == 'null-ish'"},
		// Quoted sections are opaque, including backticks inside them.
		{"body.a == 'a`b'", "body.a == 'a`b'"},
		// Unterminated backticks are left for the compiler to reject.
		{"body.a == `oops", "body.a == `oops"},
	}
	for _, test := range tests {
		if got := NormalizeExpression(test.in); got != test.want {
			t.Fatalf("NormalizeExpression(%q) = %q, want %q", test.in, got, test.want)
		}
	}

	// The rewritten bare literal must evaluate like a string comparison.
	evaluation := root(200, `{"name":"Ada-Lovelace"}`, nil)
	checkCase(t, "body.name == `Ada-Lovelace`", evaluation, true)
}

func TestJSONType(t *testing.T) {
	for _, test := range []struct {
		value any
		want  string
	}{
		{nil, "null"},
		{true, "boolean"},
		{float64(1), "number"},
		{"text", "string"},
		{[]any{1}, "array"},
		{map[string]any{}, "object"},
	} {
		if got := JSONType(test.value); got != test.want {
			t.Fatalf("JSONType(%#v) = %q, want %q", test.value, got, test.want)
		}
	}
}
