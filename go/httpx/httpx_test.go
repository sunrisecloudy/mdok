package httpx

import (
	"context"
	"encoding/pem"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"mdok/core"
	"mdok/curlplan"
)

func mustPlan(t *testing.T, argv ...string) *curlplan.Plan {
	t.Helper()
	plan, diag := curlplan.Parse(argv)
	if diag != nil {
		t.Fatalf("curlplan.Parse(%q) failed: %+v", argv, diag)
	}
	return plan
}

func testConfig() *core.ExecConfig {
	return &core.ExecConfig{
		AllowedHosts:   []string{"127.0.0.1", "localhost"},
		AllowedSchemes: []string{"http", "https"},
		ConnectTimeout: 5 * time.Second,
		TotalTimeout:   10 * time.Second,
	}
}

// captured stores what one request looked like on the server.
type captured struct {
	mu       sync.Mutex
	query    string
	method   string
	body     string
	headers  http.Header
	requests int
	statuses []int
}

func (c *captured) record(r *http.Request) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.requests++
	body, _ := io.ReadAll(r.Body)
	c.query = r.URL.RawQuery
	c.method = r.Method
	c.body = string(body)
	c.headers = r.Header.Clone()
}

func (c *captured) snapshot() (string, string, string, http.Header, int) {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.query, c.method, c.body, c.headers.Clone(), c.requests
}

func TestExecuteGetQueryBuildingOrderAndEncoding(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cap.record(r)
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--get", server.URL+"/echo",
		"--data-urlencode", "q=space slash/plus+ไทย",
		"--data", "page=2")

	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusOK {
		t.Fatalf("status = %d", transfer.Status)
	}
	query, method, body, _, _ := cap.snapshot()
	if method != "GET" || body != "" {
		t.Errorf("method=%q body=%q, want GET with no body", method, body)
	}
	want := "page=2&q=space+slash%2Fplus%2B%E0%B9%84%E0%B8%97%E0%B8%A2"
	if query != want {
		t.Errorf("query = %q, want %q", query, want)
	}
}

func TestExecuteGetAppendsToExistingQueryAndBareUrlencode(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cap.record(r)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "-G", server.URL+"/echo?a=1",
		"--data-urlencode", "hello world")
	_, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	query, _, _, _, _ := cap.snapshot()
	if query != "a=1&hello+world" {
		t.Errorf("query = %q, want a=1&hello+world", query)
	}
}

func TestExecuteDefaultContentTypeAndBodyJoin(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cap.record(r)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "-d", "a=1", "-d", "b=2", server.URL+"/echo")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusOK {
		t.Fatalf("status = %d", transfer.Status)
	}
	_, method, body, headers, _ := cap.snapshot()
	if method != "POST" {
		t.Errorf("method = %q, want POST", method)
	}
	if body != "a=1&b=2" {
		t.Errorf("body = %q, want a=1&b=2", body)
	}
	if got := headers.Get("Content-Type"); got != "application/x-www-form-urlencoded" {
		t.Errorf("Content-Type = %q, want the curl default", got)
	}
}

func TestExecuteExplicitContentTypeIsKept(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cap.record(r)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--request", "POST", server.URL+"/echo",
		"-H", "Content-Type: application/json",
		"--data-raw", `{"name":"Ada","active":true}`)
	_, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	_, _, body, headers, _ := cap.snapshot()
	if body != `{"name":"Ada","active":true}` {
		t.Errorf("raw JSON body = %q", body)
	}
	if got := headers.Values("Content-Type"); len(got) != 1 || got[0] != "application/json" {
		t.Errorf("Content-Type values = %v", got)
	}
}

func TestExecuteCookieJoining(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cap.record(r)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "-b", "fixture=ok", "--cookie", "other=v", server.URL+"/cookies/echo")
	_, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	_, _, _, headers, _ := cap.snapshot()
	if got := headers.Get("Cookie"); got != "fixture=ok; other=v" {
		t.Errorf("Cookie = %q", got)
	}
}

func TestExecuteUserAgentRefererAndBasicAuth(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cap.record(r)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "-A", "mdok-e2e", "-e", "http://localhost/ref",
		"-u", "alice:secret", server.URL+"/echo")
	_, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	_, _, _, headers, _ := cap.snapshot()
	if got := headers.Get("User-Agent"); got != "mdok-e2e" {
		t.Errorf("User-Agent = %q", got)
	}
	if got := headers.Get("Referer"); got != "http://localhost/ref" {
		t.Errorf("Referer = %q", got)
	}
	if got := headers.Get("Authorization"); got != "Basic YWxpY2U6c2VjcmV0" {
		t.Errorf("Authorization = %q", got)
	}
}

func TestExecuteFollowRedirectsAndCount(t *testing.T) {
	var requests int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&requests, 1)
		switch r.URL.Path {
		case "/start":
			http.Redirect(w, r, "/final", http.StatusFound)
		case "/final":
			_, _ = w.Write([]byte(`{"ok":true}`))
		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--location", "--max-redirs", "3", "-b", "fixture=ok", server.URL+"/start")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusOK {
		t.Fatalf("status = %d", transfer.Status)
	}
	if transfer.RedirectCount != 1 {
		t.Errorf("redirect count = %d, want 1", transfer.RedirectCount)
	}
	if string(transfer.Body) != `{"ok":true}` {
		t.Errorf("body = %q", transfer.Body)
	}
	if requests != 2 {
		t.Errorf("server saw %d requests, want 2", requests)
	}
}

func TestExecuteNoFollowReturnsRedirectResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/final", http.StatusFound)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", server.URL+"/start")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusFound || transfer.RedirectCount != 0 {
		t.Errorf("status=%d redirects=%d, want 302/0", transfer.Status, transfer.RedirectCount)
	}
	if location := headerValue(transfer.Headers, "Location"); location != "/final" {
		t.Errorf("Location header = %q", location)
	}
}

func TestExecuteRedirect303ChangesPostToGet(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/see-other" {
			http.Redirect(w, r, "/sink", http.StatusSeeOther)
			return
		}
		cap.record(r)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "-d", "a=1", "-L", server.URL+"/see-other")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusOK || transfer.RedirectCount != 1 {
		t.Fatalf("status=%d redirects=%d", transfer.Status, transfer.RedirectCount)
	}
	_, method, body, _, _ := cap.snapshot()
	if method != "GET" || body != "" {
		t.Errorf("after 303: method=%q body=%q, want GET with dropped body", method, body)
	}
}

func TestExecuteRedirect307PreservesPostBody(t *testing.T) {
	cap := &captured{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/keep" {
			http.Redirect(w, r, "/sink", http.StatusTemporaryRedirect)
			return
		}
		cap.record(r)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "-d", "a=1", "-L", server.URL+"/keep")
	_, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	_, method, body, _, _ := cap.snapshot()
	if method != "POST" || body != "a=1" {
		t.Errorf("after 307: method=%q body=%q, want POST with body", method, body)
	}
}

func TestExecuteRedirectLimitExceeded(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/loop", http.StatusFound)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--location", "--max-redirs", "2", server.URL+"/loop")
	_, err := Execute(context.Background(), plan, testConfig())
	if err == nil {
		t.Fatal("expected a redirect limit error")
	}
	var transferErr *Error
	if !errors.As(err, &transferErr) || transferErr.Code != "MDOK-E603" {
		t.Fatalf("err = %v, want MDOK-E603", err)
	}
	if transferErr.Message != "redirect limit exceeded" {
		t.Errorf("message = %q", transferErr.Message)
	}
}

func TestExecuteRetryUntilSuccess(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		attempt := atomic.AddInt32(&attempts, 1)
		if attempt <= 2 {
			http.Error(w, "try later", http.StatusServiceUnavailable)
			return
		}
		_, _ = w.Write([]byte(`{"ok":true,"attempt":3}`))
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--retry", "2", "--retry-delay", "0",
		"-H", "X-Mdok-Test-Key: e2e-retry", server.URL+"/retry/2")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusOK {
		t.Fatalf("status = %d, want 200", transfer.Status)
	}
	if transfer.Attempt != 3 {
		t.Errorf("attempt = %d, want 3", transfer.Attempt)
	}
	if attempts != 3 {
		t.Errorf("server attempts = %d, want 3", attempts)
	}
}

func TestExecuteRetryExhaustedReturnsLastResponse(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&attempts, 1)
		http.Error(w, "down", http.StatusBadGateway)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--retry", "1", "--retry-delay", "0", server.URL+"/down")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusBadGateway {
		t.Errorf("status = %d, want 502", transfer.Status)
	}
	if transfer.Attempt != 2 {
		t.Errorf("attempt = %d, want 2", transfer.Attempt)
	}
}

func TestExecuteNoRetryOnNonRetryableStatus(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&attempts, 1)
		http.NotFound(w, r)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--retry", "3", "--retry-delay", "0", server.URL+"/missing")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusNotFound || transfer.Attempt != 1 || attempts != 1 {
		t.Errorf("status=%d attempt=%d attempts=%d, want 404/1/1", transfer.Status, transfer.Attempt, attempts)
	}
}

func TestExecuteTLSWithCustomCAPool(t *testing.T) {
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	caDir := t.TempDir()
	caFile := filepath.Join(caDir, "ca.pem")
	pemBytes := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: server.Certificate().Raw})
	if err := os.WriteFile(caFile, pemBytes, 0o600); err != nil {
		t.Fatal(err)
	}

	plan := mustPlan(t, "curl", server.URL+"/health", "--cacert", caFile)
	cfg := testConfig()
	cfg.AllowedReadPaths = []string{caDir}
	if diag := curlplan.CheckPolicy(plan, cfg); diag != nil {
		t.Fatalf("policy rejected in-root CA: %+v", diag)
	}
	transfer, err := Execute(context.Background(), plan, cfg)
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Status != http.StatusOK || string(transfer.Body) != `{"ok":true}` {
		t.Errorf("status=%d body=%q", transfer.Status, transfer.Body)
	}
}

func TestExecuteTLSWithoutCACertFails(t *testing.T) {
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", server.URL+"/health")
	_, err := Execute(context.Background(), plan, testConfig())
	if err == nil {
		t.Fatal("expected a TLS verification error")
	}
	var transferErr *Error
	if !errors.As(err, &transferErr) || transferErr.Code != "MDOK-E602" {
		t.Fatalf("err = %v (%T), want MDOK-E602", err, err)
	}
}

func TestExecuteInvalidCACertPEM(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {}))
	defer server.Close()

	caDir := t.TempDir()
	caFile := filepath.Join(caDir, "bad.pem")
	if err := os.WriteFile(caFile, []byte("not a pem"), 0o600); err != nil {
		t.Fatal(err)
	}
	plan := mustPlan(t, "curl", server.URL+"/health", "--cacert", caFile)
	_, err := Execute(context.Background(), plan, testConfig())
	var transferErr *Error
	if !errors.As(err, &transferErr) || transferErr.Code != "MDOK-E602" {
		t.Fatalf("err = %v, want MDOK-E602", err)
	}
}

func TestExecuteFailFlagTurns4xxIntoError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.NotFound(w, r)
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", "--fail", server.URL+"/missing")
	_, err := Execute(context.Background(), plan, testConfig())
	if err == nil {
		t.Fatal("expected --fail to error on 404")
	}

	plan = mustPlan(t, "curl", server.URL+"/missing")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil || transfer.Status != http.StatusNotFound {
		t.Errorf("without --fail: err=%v status=%d", err, transfer.Status)
	}
}

func TestExecuteContextCancellation(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {}))
	defer server.Close()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	plan := mustPlan(t, "curl", server.URL+"/health")
	_, err := Execute(ctx, plan, testConfig())
	if err == nil {
		t.Fatal("expected an error for a cancelled context")
	}
	var transferErr *Error
	if !errors.As(err, &transferErr) || transferErr.Code != "MDOK-E605" {
		t.Fatalf("err = %v, want MDOK-E605", err)
	}
}

func TestExecuteRecordsTimingsAndAttempt(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("ok"))
	}))
	defer server.Close()

	plan := mustPlan(t, "curl", server.URL+"/health")
	transfer, err := Execute(context.Background(), plan, testConfig())
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if transfer.Attempt != 1 {
		t.Errorf("attempt = %d, want 1", transfer.Attempt)
	}
	if transfer.Timings.TotalMS <= 0 {
		t.Errorf("total ms = %f, want > 0", transfer.Timings.TotalMS)
	}
}

func TestFormEncodeMatchesRustFormEncode(t *testing.T) {
	cases := map[string]string{
		"space slash/plus+ไทย": "space+slash%2Fplus%2B%E0%B9%84%E0%B8%97%E0%B8%A2",
		"a.b-c_d~e":            "a%2Eb%2Dc%5Fd%7Ee",
		"":                     "",
		"123abcABC":            "123abcABC",
	}
	for input, want := range cases {
		if got := formEncode(input); got != want {
			t.Errorf("formEncode(%q) = %q, want %q", input, got, want)
		}
	}
}

func headerValue(headers []core.KV, name string) string {
	for _, header := range headers {
		if header.Key == name {
			return header.Value
		}
	}
	return ""
}
