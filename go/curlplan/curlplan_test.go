package curlplan

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"mdok/core"
)

func TestParseBareURLDefaultsToGet(t *testing.T) {
	plan, diag := Parse([]string{"curl", "http://127.0.0.1:8080/health"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if plan.Method != "GET" {
		t.Errorf("method = %q, want GET", plan.Method)
	}
	if plan.URL != "http://127.0.0.1:8080/health" {
		t.Errorf("url = %q", plan.URL)
	}
	if plan.MaxRedirs != 50 {
		t.Errorf("max redirs default = %d, want 50", plan.MaxRedirs)
	}
	if plan.Follow || plan.GetFlag || plan.Retry != 0 {
		t.Errorf("unexpected defaults: follow=%v get=%v retry=%d", plan.Follow, plan.GetFlag, plan.Retry)
	}
}

func TestParseRequestFlag(t *testing.T) {
	plan, diag := Parse([]string{"curl", "-X", "DELETE", "http://localhost/x"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if plan.Method != "DELETE" {
		t.Errorf("method = %q, want DELETE", plan.Method)
	}
	plan, diag = Parse([]string{"curl", "-XPUT", "http://localhost/x"})
	if diag != nil || plan.Method != "PUT" {
		t.Errorf("attached -XPUT: diag=%v method=%q", diag, plan.Method)
	}
}

func TestParseDataImpliesPostAndKeepsOrder(t *testing.T) {
	plan, diag := Parse([]string{"curl", "-d", "a=1", "--data-raw", "b=2", "http://localhost/x"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if plan.Method != "POST" {
		t.Errorf("method = %q, want POST", plan.Method)
	}
	if len(plan.BodyParts) != 2 || plan.BodyParts[0] != "a=1" || plan.BodyParts[1] != "b=2" {
		t.Errorf("body parts = %v", plan.BodyParts)
	}
}

func TestParseExplicitMethodAfterDataWins(t *testing.T) {
	plan, diag := Parse([]string{"curl", "-d", "a=1", "-X", "PUT", "http://localhost/x"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if plan.Method != "PUT" {
		t.Errorf("method = %q, want PUT", plan.Method)
	}
}

func TestParseGetFlagForcesGet(t *testing.T) {
	plan, diag := Parse([]string{"curl", "--get", "-d", "page=2", "--data-urlencode", "q=x y", "http://localhost/echo"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if !plan.GetFlag {
		t.Error("GetFlag not set")
	}
	if plan.Method != "GET" {
		t.Errorf("method = %q, want GET", plan.Method)
	}
	if len(plan.BodyParts) != 1 || plan.BodyParts[0] != "page=2" {
		t.Errorf("body parts = %v", plan.BodyParts)
	}
	if len(plan.DataUrlencode) != 1 || plan.DataUrlencode[0] != "q=x y" {
		t.Errorf("data urlencode = %v (raw value expected)", plan.DataUrlencode)
	}
}

func TestParseGetFlagOverridesExplicitMethod(t *testing.T) {
	plan, diag := Parse([]string{"curl", "-X", "POST", "-G", "http://localhost/x"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if plan.Method != "GET" {
		t.Errorf("method = %q, want GET (get wins over -X like curl)", plan.Method)
	}
}

func TestParseHeadersDuplicatesAndCookies(t *testing.T) {
	plan, diag := Parse([]string{"curl", "-H", "Accept: application/json", "--header", "X-Double: 1",
		"-H", "X-Double: 2", "-b", "fixture=ok", "--cookie", "other=v", "http://localhost/x"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if len(plan.Headers) != 3 {
		t.Fatalf("headers = %v", plan.Headers)
	}
	if plan.Headers[0] != (core.KV{Key: "Accept", Value: "application/json"}) {
		t.Errorf("header[0] = %+v", plan.Headers[0])
	}
	if plan.Headers[1].Key != "X-Double" || plan.Headers[2].Value != "2" {
		t.Errorf("duplicate headers lost: %+v", plan.Headers[1:])
	}
	if len(plan.Cookies) != 2 || plan.Cookies[0] != (core.KV{Key: "fixture", Value: "ok"}) {
		t.Errorf("cookies = %v", plan.Cookies)
	}
}

func TestParseLocationAndMaxRedirs(t *testing.T) {
	plan, diag := Parse([]string{"curl", "-L", "http://localhost/x"})
	if diag != nil || !plan.Follow || plan.MaxRedirs != 50 {
		t.Errorf("-L defaults: diag=%v follow=%v max=%d", diag, plan.Follow, plan.MaxRedirs)
	}
	plan, diag = Parse([]string{"curl", "--location", "--max-redirs", "3", "http://localhost/x"})
	if diag != nil || !plan.Follow || plan.MaxRedirs != 3 {
		t.Errorf("--max-redirs: diag=%v follow=%v max=%d", diag, plan.Follow, plan.MaxRedirs)
	}
}

func TestParseRetryAndDurations(t *testing.T) {
	plan, diag := Parse([]string{"curl", "--retry", "2", "--retry-delay", "1.5",
		"--max-time", "2", "--connect-timeout", "0.5", "http://localhost/x"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if plan.Retry != 2 || plan.RetryDelayMS != 1500 {
		t.Errorf("retry=%d delay=%dms", plan.Retry, plan.RetryDelayMS)
	}
	if plan.MaxTimeMS != 2000 || plan.ConnectTimeoutMS != 500 {
		t.Errorf("max-time=%dms connect-timeout=%dms", plan.MaxTimeMS, plan.ConnectTimeoutMS)
	}
}

func TestParseMappedNoOpAndValueOptions(t *testing.T) {
	plan, diag := Parse([]string{"curl", "-s", "-S", "-i", "--compressed",
		"-u", "alice:secret", "-A", "mdok-e2e", "-e", "http://localhost/ref",
		"--url", "http://localhost/x"})
	if diag != nil {
		t.Fatalf("unexpected diagnostic: %+v", diag)
	}
	if !plan.Compressed {
		t.Errorf("compressed=%v", plan.Compressed)
	}
	if _, diag := Parse([]string{"curl", "--fail", "http://localhost/x"}); diag == nil || diag.Code != "MDOK-E300" {
		t.Fatalf("--fail should be rejected like the Rust policy: %+v", diag)
	}
	if plan.User != "alice:secret" || plan.UserAgent != "mdok-e2e" || plan.Referer != "http://localhost/ref" {
		t.Errorf("user/agent/referer = %q/%q/%q", plan.User, plan.UserAgent, plan.Referer)
	}
	if plan.URL != "http://localhost/x" {
		t.Errorf("--url operand = %q", plan.URL)
	}
}

func TestParseUnknownOption(t *testing.T) {
	_, diag := Parse([]string{"curl", "--bogus", "http://localhost/x"})
	if diag == nil {
		t.Fatal("expected a diagnostic for an unknown option")
	}
	if diag.Code != "MDOK-E300" {
		t.Errorf("code = %s, want MDOK-E300", diag.Code)
	}
	if diag.Message != "unknown curl option `--bogus`" {
		t.Errorf("message = %q", diag.Message)
	}
	if diag.Severity != core.SeverityError {
		t.Errorf("severity = %q", diag.Severity)
	}
}

func TestParseExactlyOneURLRequired(t *testing.T) {
	for _, argv := range [][]string{
		{"curl"},
		{"curl", "-L"},
		{"curl", "http://localhost/a", "http://localhost/b"},
	} {
		_, diag := Parse(argv)
		if diag == nil {
			t.Fatalf("Parse(%v) expected a diagnostic", argv)
		}
		if diag.Code != "MDOK-E304" || diag.Message != "exactly one URL is required" {
			t.Errorf("Parse(%v) = %s/%q", argv, diag.Code, diag.Message)
		}
	}
}

func TestParseMissingOptionArgument(t *testing.T) {
	_, diag := Parse([]string{"curl", "-H"})
	if diag == nil || diag.Code != "MDOK-E300" || diag.Message != "option `-H` needs an argument" {
		t.Fatalf("diag = %+v", diag)
	}
	_, diag = Parse([]string{"curl", "--retry"})
	if diag == nil || diag.Message != "option `--retry` needs an argument" {
		t.Fatalf("diag = %+v", diag)
	}
}

func TestParseHeaderValidation(t *testing.T) {
	_, diag := Parse([]string{"curl", "-H", "Accept", "http://localhost/x"})
	if diag == nil || diag.Code != "MDOK-E304" || diag.Message != "header must contain ':'" {
		t.Fatalf("no-colon diag = %+v", diag)
	}
	_, diag = Parse([]string{"curl", "-H", "Bad Header: x", "http://localhost/x"})
	if diag == nil || diag.Message != "invalid header" {
		t.Fatalf("space-in-name diag = %+v", diag)
	}
	_, diag = Parse([]string{"curl", "-H", "X-A: v\r\nX-B: y", "http://localhost/x"})
	if diag == nil || diag.Message != "invalid header" {
		t.Fatalf("crlf diag = %+v", diag)
	}
}

func TestParseInvalidNumbersAndDurations(t *testing.T) {
	cases := []struct {
		argv    []string
		message string
	}{
		{[]string{"curl", "--retry", "soon", "http://localhost/x"}, "invalid retry: soon"},
		{[]string{"curl", "--max-redirs", "-1", "http://localhost/x"}, "invalid max-redirs: -1"},
		{[]string{"curl", "--max-time", "abc", "http://localhost/x"}, "invalid max-time: abc"},
		{[]string{"curl", "--connect-timeout", "-1", "http://localhost/x"}, "invalid connect-timeout: -1"},
		{[]string{"curl", "--retry-delay", "later", "http://localhost/x"}, "invalid retry-delay: later"},
	}
	for _, tc := range cases {
		_, diag := Parse(tc.argv)
		if diag == nil || diag.Code != "MDOK-E304" || diag.Message != tc.message {
			t.Errorf("Parse(%v) = %+v, want MDOK-E304 %q", tc.argv, diag, tc.message)
		}
	}
}

func TestParseMustBeginWithCurl(t *testing.T) {
	_, diag := Parse([]string{"wget", "http://localhost/x"})
	if diag == nil || diag.Code != "MDOK-E300" || diag.Message != "the command must begin with curl" {
		t.Fatalf("diag = %+v", diag)
	}
}

func TestParseInvalidConcreteURL(t *testing.T) {
	_, diag := Parse([]string{"curl", "not a url"})
	if diag == nil || diag.Code != "MDOK-E304" || !strings.HasPrefix(diag.Message, "invalid URL") {
		t.Fatalf("diag = %+v", diag)
	}
}

func TestParseTemplateTextPassesThroughUnvalidated(t *testing.T) {
	plan, diag := Parse([]string{"curl", "--get", "{{base_url}}/echo",
		"--data-urlencode", "q={{query_value|string}}",
		"-H", "Authorization: Bearer {{token|header}}",
		"--cacert", "{{ca_file}}"})
	if diag != nil {
		t.Fatalf("template argv rejected: %+v", diag)
	}
	if plan.URL != "{{base_url}}/echo" {
		t.Errorf("url = %q", plan.URL)
	}
	if plan.DataUrlencode[0] != "q={{query_value|string}}" {
		t.Errorf("urlencode = %v", plan.DataUrlencode)
	}
	if plan.CACert != "{{ca_file}}" {
		t.Errorf("cacert = %q", plan.CACert)
	}
}

func TestCheckPolicySchemeDenied(t *testing.T) {
	plan, _ := Parse([]string{"curl", "ftp://127.0.0.1/file"})
	diag := CheckPolicy(plan, testConfig())
	if diag == nil || diag.Code != "MDOK-E302" {
		t.Fatalf("diag = %+v, want MDOK-E302", diag)
	}
	if diag.Message != "scheme `ftp` is not allowed" {
		t.Errorf("message = %q", diag.Message)
	}
}

func TestCheckPolicySchemeDefaultsToHTTPHTTPS(t *testing.T) {
	cfg := &core.ExecConfig{AllowedHosts: []string{"127.0.0.1"}}
	plan, _ := Parse([]string{"curl", "https://127.0.0.1/x"})
	if diag := CheckPolicy(plan, cfg); diag != nil {
		t.Fatalf("default schemes rejected https: %+v", diag)
	}
}

func TestCheckPolicyHostDenied(t *testing.T) {
	plan, _ := Parse([]string{"curl", "http://evil.test/x"})
	diag := CheckPolicy(plan, testConfig())
	if diag == nil || diag.Code != "MDOK-E304" {
		t.Fatalf("diag = %+v, want MDOK-E304", diag)
	}
	if diag.Message != "host `evil.test` is not allowed" {
		t.Errorf("message = %q", diag.Message)
	}
}

func TestCheckPolicyAllowsConfiguredHost(t *testing.T) {
	plan, _ := Parse([]string{"curl", "http://localhost:8080/health"})
	if diag := CheckPolicy(plan, testConfig()); diag != nil {
		t.Fatalf("allowed host rejected: %+v", diag)
	}
}

func TestCheckPolicyTemplatedPlanPasses(t *testing.T) {
	plan, _ := Parse([]string{"curl", "{{base_url}}/health"})
	if diag := CheckPolicy(plan, testConfig()); diag != nil {
		t.Fatalf("templated plan rejected: %+v", diag)
	}
}

func TestCheckPolicyCACertOutsideReadRoots(t *testing.T) {
	outside := t.TempDir()
	caFile := filepath.Join(outside, "ca.pem")
	if err := os.WriteFile(caFile, []byte("pem"), 0o600); err != nil {
		t.Fatal(err)
	}
	roots := t.TempDir()

	plan, _ := Parse([]string{"curl", "--cacert", caFile, "http://127.0.0.1/x"})
	diag := CheckPolicy(plan, withReadRoots(testConfig(), roots))
	if diag == nil || diag.Code != "MDOK-E303" {
		t.Fatalf("diag = %+v, want MDOK-E303", diag)
	}
	if diag.Message != "file is outside the allowed read roots" {
		t.Errorf("message = %q", diag.Message)
	}
}

func TestCheckPolicyCACertInsideReadRootPasses(t *testing.T) {
	inside := t.TempDir()
	caFile := filepath.Join(inside, "ca.pem")
	if err := os.WriteFile(caFile, []byte("pem"), 0o600); err != nil {
		t.Fatal(err)
	}
	plan, _ := Parse([]string{"curl", "--cacert", caFile, "http://127.0.0.1/x"})
	if diag := CheckPolicy(plan, withReadRoots(testConfig(), inside)); diag != nil {
		t.Fatalf("in-root cacert rejected: %+v", diag)
	}
}

func TestCheckPolicyCACertMissingFile(t *testing.T) {
	roots := t.TempDir()
	plan, _ := Parse([]string{"curl", "--cacert", filepath.Join(roots, "nope.pem"), "http://127.0.0.1/x"})
	diag := CheckPolicy(plan, withReadRoots(testConfig(), roots))
	if diag == nil || diag.Code != "MDOK-E303" || !strings.HasPrefix(diag.Message, "cannot access file") {
		t.Fatalf("diag = %+v", diag)
	}
}

func testConfig() *core.ExecConfig {
	return &core.ExecConfig{
		AllowedHosts:   []string{"127.0.0.1", "localhost"},
		AllowedSchemes: []string{"http", "https"},
		ConnectTimeout: 5 * time.Second,
		TotalTimeout:   30 * time.Second,
	}
}

func withReadRoots(cfg *core.ExecConfig, roots ...string) *core.ExecConfig {
	cfg.AllowedReadPaths = roots
	return cfg
}
