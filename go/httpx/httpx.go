// Package httpx executes curl plans through net/http. It mirrors the
// execution semantics of crates/mdok-curl: --get query building, default
// Content-Type, cookie joining, manual redirect following with method
// changes, retry classification, CA-pinned TLS, and MDOK-Exxx errors.
package httpx

import (
	"compress/gzip"
	"context"
	"crypto/tls"
	"crypto/x509"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"sort"
	"strings"
	"time"

	"mdok/core"
	"mdok/curlplan"
)

// maxBodyBytes caps captured response bodies (8 MiB, mirroring the Rust
// max_body_bytes default). Bodies larger than the cap fail with MDOK-E700.
const maxBodyBytes = 8 << 20

// Error is a transfer failure carrying an MDOK-Exxx code.
type Error struct {
	Code    string
	Title   string
	Message string
}

func (e *Error) Error() string { return e.Message }

func transferError(code string, title string, message string) *Error {
	return &Error{Code: code, Title: title, Message: message}
}

// errRedirectLimit mirrors the Rust RedirectLimitError message.
var errRedirectLimit = errors.New("redirect limit exceeded")

// redirectStatuses are the hops curl follows with --location.
func redirectStatus(code int) bool {
	switch code {
	case 301, 302, 303, 307, 308:
		return true
	}
	return false
}

// retryableStatus mirrors Rust is_retryable_status (including 425).
func retryableStatus(code int) bool {
	switch code {
	case 408, 425, 429, 500, 502, 503, 504:
		return true
	}
	return false
}

// Execute runs one curl plan and returns the final transfer. Retries wrap
// the whole transfer (redirects included); each execution counts as one
// attempt, 1-based.
func Execute(ctx context.Context, plan *curlplan.Plan, cfg *core.ExecConfig) (*core.Transfer, error) {
	started := time.Now()
	if plan == nil {
		return nil, transferError("MDOK-E600", "Transfer failed", "nil curl plan")
	}
	if cfg == nil {
		cfg = &core.ExecConfig{}
	}

	target, body, hasBody, err := buildTarget(plan)
	if err != nil {
		return nil, err
	}
	transport, err := buildTransport(plan, cfg)
	if err != nil {
		return nil, err
	}
	client := &http.Client{
		Transport: transport,
		// Redirects are followed manually so hop counts and 303 method
		// changes stay observable.
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}

	ctx, cancel := applyTimeouts(ctx, plan, cfg)
	defer cancel()

	attempt := 0
	for {
		attempt++
		response, hops, err := doOnce(ctx, client, plan, target, body, hasBody)
		if err == nil && (!retryableStatus(response.StatusCode) || attempt > plan.Retry) {
			return finishTransfer(response, hops, attempt, started, plan)
		}
		if err != nil {
			if attempt > plan.Retry || ctx.Err() != nil {
				return nil, classifyError(ctx.Err() != nil && ctx.Err() == context.Canceled, err)
			}
		} else {
			// Drain and close the retryable response before waiting.
			_, _ = io.Copy(io.Discard, response.Body)
			response.Body.Close()
		}
		if plan.RetryDelayMS > 0 {
			timer := time.NewTimer(time.Duration(plan.RetryDelayMS) * time.Millisecond)
			select {
			case <-ctx.Done():
				timer.Stop()
				return nil, classifyError(ctx.Err() == context.Canceled, ctx.Err())
			case <-timer.C:
			}
		}
	}
}

// buildTarget resolves the final request URL and body string. With --get,
// body parts and urlencoded entries move into the query string (existing
// query first, then --data parts, then --data-urlencode entries, joined
// with "&"); otherwise they form the request body.
func buildTarget(plan *curlplan.Plan) (*url.URL, string, bool, error) {
	target, err := url.Parse(plan.URL)
	if err != nil {
		return nil, "", false, transferError("MDOK-E304", "Invalid URL",
			fmt.Sprintf("invalid URL: %s", err.Error()))
	}
	encoded := make([]string, 0, len(plan.DataUrlencode))
	for _, entry := range plan.DataUrlencode {
		encoded = append(encoded, urlencodeEntry(entry))
	}
	hasBody := len(plan.BodyParts) > 0 || len(plan.DataUrlencode) > 0
	if plan.GetFlag {
		if len(plan.BodyParts) > 0 || len(encoded) > 0 {
			entries := make([]string, 0, len(plan.BodyParts)+len(encoded))
			entries = append(entries, plan.BodyParts...)
			entries = append(entries, encoded...)
			query := strings.Join(entries, "&")
			if target.RawQuery != "" {
				target.RawQuery = target.RawQuery + "&" + query
			} else {
				target.RawQuery = query
			}
		}
		return target, "", false, nil
	}
	if !hasBody {
		return target, "", false, nil
	}
	parts := make([]string, 0, len(plan.BodyParts)+len(encoded))
	parts = append(parts, plan.BodyParts...)
	parts = append(parts, encoded...)
	return target, strings.Join(parts, "&"), true, nil
}

// urlencodeEntry mirrors the Rust data_urlencode encoding: "name=content"
// encodes both sides, bare content is encoded whole. The encoder leaves
// ASCII alphanumerics untouched, maps space to '+', and percent-encodes
// every other byte in uppercase hex (form_encode parity).
func urlencodeEntry(entry string) string {
	name, content, found := strings.Cut(entry, "=")
	if !found {
		return formEncode(entry)
	}
	return formEncode(name) + "=" + formEncode(content)
}

func formEncode(value string) string {
	var builder strings.Builder
	for i := 0; i < len(value); i++ {
		c := value[i]
		switch {
		case c >= '0' && c <= '9', c >= 'A' && c <= 'Z', c >= 'a' && c <= 'z':
			builder.WriteByte(c)
		case c == ' ':
			builder.WriteByte('+')
		default:
			fmt.Fprintf(&builder, "%%%02X", c)
		}
	}
	return builder.String()
}

// buildTLSConfig resolves the TLS root pool (from --cacert) and the
// effective connect timeout shared by the transport and the raw HEAD path.
func buildTLSConfig(plan *curlplan.Plan, cfg *core.ExecConfig) (*tls.Config, time.Duration, error) {
	tlsConfig := &tls.Config{}
	if plan.CACert != "" {
		data, err := os.ReadFile(plan.CACert)
		if err != nil {
			return nil, 0, transferError("MDOK-E303", "File read denied",
				fmt.Sprintf("cannot read file: %s", err.Error()))
		}
		pool := x509.NewCertPool()
		if !pool.AppendCertsFromPEM(data) {
			return nil, 0, transferError("MDOK-E602", "TLS error", "invalid CA certificate")
		}
		tlsConfig.RootCAs = pool
	}
	connect := cfg.ConnectTimeout
	if connect <= 0 {
		connect = 5 * time.Second
	}
	if plan.ConnectTimeoutMS > 0 {
		if capped := time.Duration(plan.ConnectTimeoutMS) * time.Millisecond; capped < connect {
			connect = capped
		}
	}
	return tlsConfig, connect, nil
}

// buildTransport assembles the HTTP transport: CA pool from --cacert, a
// dialer with the connect timeout, and no environment proxy inheritance
// (the Rust client sets no_proxy for the same reason).
func buildTransport(plan *curlplan.Plan, cfg *core.ExecConfig) (*http.Transport, error) {
	tlsConfig, connect, err := buildTLSConfig(plan, cfg)
	if err != nil {
		return nil, err
	}
	return &http.Transport{
		TLSClientConfig: tlsConfig,
		Proxy:           nil,
		// curl never requests compression unless --compressed is given;
		// Go's transparent gzip would otherwise change server behavior.
		DisableCompression:  true,
		DialContext:         (&net.Dialer{Timeout: connect, KeepAlive: 30 * time.Second}).DialContext,
		MaxIdleConns:        100,
		IdleConnTimeout:     90 * time.Second,
		TLSHandshakeTimeout: 10 * time.Second,
	}, nil
}

// applyTimeouts layers the configured total timeout and --max-time onto the
// caller context (the shorter deadline wins).
func applyTimeouts(ctx context.Context, plan *curlplan.Plan, cfg *core.ExecConfig) (context.Context, context.CancelFunc) {
	total := cfg.TotalTimeout
	if total <= 0 {
		total = 30 * time.Second
	}
	if plan.MaxTimeMS > 0 {
		if capped := time.Duration(plan.MaxTimeMS) * time.Millisecond; capped < total {
			total = capped
		}
	}
	return context.WithTimeout(ctx, total)
}

// doOnce performs one transfer attempt, following redirects manually when
// the plan asks for it. It returns the final response and the number of
// followed hops.
func doOnce(ctx context.Context, client *http.Client, plan *curlplan.Plan, target *url.URL, body string, hasBody bool) (*http.Response, int, error) {
	current := *target
	method := plan.Method
	currentBody := body
	currentHasBody := hasBody
	hops := 0
	for {
		request, err := buildRequest(ctx, plan, current, method, currentBody, currentHasBody)
		if err != nil {
			return nil, hops, err
		}
		response, err := client.Do(request)
		if err != nil {
			return nil, hops, err
		}
		location := response.Header.Get("Location")
		if plan.Follow && redirectStatus(response.StatusCode) && location != "" {
			if hops >= plan.MaxRedirs {
				response.Body.Close()
				return nil, hops, errRedirectLimit
			}
			next, parseErr := current.Parse(location)
			if parseErr != nil {
				response.Body.Close()
				return nil, hops, transferError("MDOK-E603", "Redirect error",
					fmt.Sprintf("invalid redirect location: %s", parseErr.Error()))
			}
			hops++
			// 303 always becomes GET; 301/302 rewrite POST to GET like
			// curl. 307/308 preserve the method and body.
			if response.StatusCode == http.StatusSeeOther ||
				((response.StatusCode == http.StatusMovedPermanently || response.StatusCode == http.StatusFound) &&
					method == http.MethodPost) {
				method = http.MethodGet
				currentBody = ""
				currentHasBody = false
			}
			response.Body.Close()
			current = *next
			continue
		}
		return response, hops, nil
	}
}

// buildRequest assembles one hop's request: ordered headers (duplicates
// allowed), the default form Content-Type, joined cookies, User-Agent,
// Referer, and basic auth from --user.
func buildRequest(ctx context.Context, plan *curlplan.Plan, target url.URL, method string, body string, hasBody bool) (*http.Request, error) {
	var reader io.Reader
	if hasBody && body != "" {
		reader = strings.NewReader(body)
	}
	request, err := http.NewRequestWithContext(ctx, method, target.String(), reader)
	if err != nil {
		return nil, transferError("MDOK-E600", "Transfer failed", err.Error())
	}
	for _, header := range plan.Headers {
		request.Header.Add(header.Key, header.Value)
	}
	if plan.Compressed && !hasHeader(plan.Headers, "Accept-Encoding") {
		request.Header.Add("Accept-Encoding", "gzip, deflate")
	}
	if hasBody && !hasHeader(plan.Headers, "Content-Type") {
		request.Header.Add("Content-Type", "application/x-www-form-urlencoded")
	}
	if len(plan.Cookies) > 0 {
		request.Header.Add("Cookie", cookieHeader(plan.Cookies))
	}
	if plan.UserAgent != "" && !hasHeader(plan.Headers, "User-Agent") {
		request.Header.Set("User-Agent", plan.UserAgent)
	}
	if plan.Referer != "" && !hasHeader(plan.Headers, "Referer") {
		request.Header.Set("Referer", plan.Referer)
	}
	if plan.User != "" {
		user, password, _ := strings.Cut(plan.User, ":")
		request.SetBasicAuth(user, password)
	}
	return request, nil
}

func hasHeader(headers []core.KV, name string) bool {
	for _, header := range headers {
		if strings.EqualFold(header.Key, name) {
			return true
		}
	}
	return false
}

// cookieHeader joins --cookie values with "; " into one Cookie header.
func cookieHeader(cookies []core.KV) string {
	parts := make([]string, 0, len(cookies))
	for _, cookie := range cookies {
		if cookie.Value == "" {
			parts = append(parts, cookie.Key)
			continue
		}
		parts = append(parts, cookie.Key+"="+cookie.Value)
	}
	return strings.Join(parts, "; ")
}

// finishTransfer reads the response (capped), records headers, timings, and
// the --fail verdict.
func finishTransfer(response *http.Response, hops int, attempt int, started time.Time, plan *curlplan.Plan) (*core.Transfer, error) {
	defer response.Body.Close()
	var reader io.Reader = response.Body
	// --compressed: curl sends Accept-Encoding and decompresses itself.
	if plan.Compressed && response.Header.Get("Content-Encoding") == "gzip" {
		gzipReader, err := gzip.NewReader(response.Body)
		if err != nil {
			return nil, transferError("MDOK-E600", "Transfer failed",
				"invalid gzip response: "+err.Error())
		}
		defer gzipReader.Close()
		reader = gzipReader
	}
	body, err := io.ReadAll(io.LimitReader(reader, maxBodyBytes+1))
	if err != nil {
		return nil, transferError("MDOK-E600", "Transfer failed", err.Error())
	}
	if len(body) > maxBodyBytes {
		return nil, transferError("MDOK-E700", "Body limit exceeded",
			"response body exceeds the configured limit")
	}
	keys := make([]string, 0, len(response.Header))
	for key := range response.Header {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	headers := make([]core.KV, 0, len(response.Header))
	for _, key := range keys {
		for _, value := range response.Header[key] {
			headers = append(headers, core.KV{Key: key, Value: value})
		}
	}
	return &core.Transfer{
		Status:        response.StatusCode,
		Body:          body,
		Headers:       headers,
		RedirectCount: hops,
		Attempt:       attempt,
		Timings:       core.Timings{TotalMS: float64(time.Since(started).Nanoseconds()) / 1e6},
	}, nil
}

// classifyError maps a transport failure onto MDOK-Exxx codes, mirroring
// classify_reqwest_error_code: cancelled > redirect > timeout > TLS >
// generic transfer.
func classifyError(cancelled bool, err error) error {
	if cancelled || errors.Is(err, context.Canceled) {
		return transferError("MDOK-E605", "Transfer cancelled", "transfer cancelled")
	}
	if errors.Is(err, errRedirectLimit) {
		return transferError("MDOK-E603", "Too many redirects", errRedirectLimit.Error())
	}
	if errors.Is(err, context.DeadlineExceeded) {
		return transferError("MDOK-E601", "Transfer timed out", "operation timed out")
	}
	var tlsErr *tls.CertificateVerificationError
	if errors.As(err, &tlsErr) {
		return transferError("MDOK-E602", "TLS error", tlsErr.Error())
	}
	var netErr net.Error
	if errors.As(err, &netErr) && netErr.Timeout() {
		return transferError("MDOK-E601", "Transfer timed out", "operation timed out")
	}
	if os.IsTimeout(err) {
		return transferError("MDOK-E601", "Transfer timed out", "operation timed out")
	}
	message := strings.ToLower(err.Error())
	for _, marker := range []string{"certificate", " tls", "tls ", "x509"} {
		if strings.Contains(message, marker) {
			return transferError("MDOK-E602", "TLS error", err.Error())
		}
	}
	return transferError("MDOK-E600", "Transfer failed", err.Error())
}
