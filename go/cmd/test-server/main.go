// Command test-server is a small, deterministic HTTP fixture service used by
// mdok integration tests.  It is a Go port of
// crates/mdok-test-server/src/main.rs and intentionally implements the
// HTTP/1.1 loop with the standard library only.  Every piece of mutable state
// is keyed by X-Mdok-Test-Key so parallel tests cannot affect one another.
package main

import (
	"bytes"
	"compress/gzip"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"encoding/pem"
	"flag"
	"fmt"
	"io"
	"math/big"
	"net"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
	"unicode/utf8"
)

const (
	maxHeaderBytes   = 128 * 1024
	maxBodyBytes     = 32 * 1024 * 1024
	maxGeneratedSize = 32 * 1024 * 1024
)

var testKeyHeaders = []string{
	"x-mdok-test-key",
	"x-mdok-fixture-key",
	"x-fixture-test-key",
	"x-test-key",
}

type readyRecord struct {
	HTTPBaseURL  string `json:"http_base_url"`
	HTTPSBaseURL string `json:"https_base_url"`
	ProxyURL     string `json:"proxy_url"`
	CAFile       string `json:"ca_file"`
}

type retryKey struct {
	testKey string
	path    string
}

type serverState struct {
	mu      sync.Mutex
	users   map[string]map[string]any
	retries map[retryKey]uint32
}

type headerPair struct {
	name  string
	value string
}

type request struct {
	method  string
	target  string
	path    string
	query   map[string][]string
	headers []headerPair
	body    []byte
}

// header returns the last occurrence of the named header, like the Rust
// fixture's reversed search.
func (r *request) header(name string) (string, bool) {
	for i := len(r.headers) - 1; i >= 0; i-- {
		if strings.EqualFold(r.headers[i].name, name) {
			return r.headers[i].value, true
		}
	}
	return "", false
}

func (r *request) testKey() string {
	for _, name := range testKeyHeaders {
		if value, ok := r.header(name); ok {
			if value != "" {
				return value
			}
			return "default"
		}
	}
	return "default"
}

func (r *request) queryOne(name string) (string, bool) {
	if values, ok := r.query[name]; ok && len(values) > 0 {
		return values[0], true
	}
	return "", false
}

type responseBody struct {
	fixed   []byte
	chunks  [][]byte
	chunked bool
}

type response struct {
	status       int
	headers      []headerPair
	body         responseBody
	closeAfter   bool
	chunkDelayMs uint64
}

func fixedResponse(status int, body []byte) *response {
	return &response{status: status, body: responseBody{fixed: body}}
}

func jsonResponse(status int, value any) *response {
	result := fixedResponse(status, jsonMarshal(value))
	result.headers = append(result.headers, headerPair{"Content-Type", "application/json"})
	return result
}

func main() {
	listenFlag := flag.String("listen", "127.0.0.1:0", "HTTP fixture listener address")
	tlsListenFlag := flag.String("tls-listen", "127.0.0.1:0", "HTTPS fixture listener address")
	jsonReady := flag.Bool("json-ready", false, "print readiness JSON on stdout")
	flag.Parse()

	httpListener, err := bindLoopback(*listenFlag)
	if err != nil {
		fatalf("binding HTTP fixture listener: %v", err)
	}
	tlsListener, err := bindLoopback(*tlsListenFlag)
	if err != nil {
		fatalf("binding HTTPS fixture listener: %v", err)
	}
	proxyListener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		fatalf("binding loopback proxy listener: %v", err)
	}
	state := &serverState{
		users:   map[string]map[string]any{},
		retries: map[retryKey]uint32{},
	}
	tlsConfig, caFile, err := newFixtureTLS()
	if err != nil {
		fatalf("building fixture TLS material: %v", err)
	}

	ready := readyRecord{
		HTTPBaseURL:  "http://" + httpListener.Addr().String(),
		HTTPSBaseURL: "https://" + tlsListener.Addr().String(),
		ProxyURL:     "http://" + proxyListener.Addr().String(),
		CAFile:       caFile,
	}
	if *jsonReady {
		fmt.Println(string(jsonMarshal(ready)))
	}
	fmt.Fprintf(os.Stderr, "%s\n", jsonMarshal(struct {
		Event string `json:"event"`
		HTTP  string `json:"http"`
		HTTPS string `json:"https"`
	}{Event: "ready", HTTP: ready.HTTPBaseURL, HTTPS: ready.HTTPSBaseURL}))

	spawnHTTP(httpListener, state)
	spawnTLS(tlsListener, state, tlsConfig)
	spawnProxy(proxyListener)
	select {}
}

func fatalf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}

func bindLoopback(spec string) (net.Listener, error) {
	address, err := net.ResolveTCPAddr("tcp", spec)
	if err != nil {
		return nil, fmt.Errorf("listen address resolved to no addresses: %s", spec)
	}
	if address.IP == nil || !address.IP.IsLoopback() {
		return nil, fmt.Errorf("fixture server only accepts loopback listen addresses")
	}
	return net.ListenTCP("tcp", address)
}

// newFixtureTLS generates a throwaway self-signed CA plus a loopback server
// leaf certificate, writes the CA PEM to a temp file for clients, and returns
// a TLS config serving the leaf.  Clients verify the chain via the CA file,
// so no --insecure equivalent is required.
func newFixtureTLS() (*tls.Config, string, error) {
	now := time.Now()
	caKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, "", err
	}
	caTemplate := &x509.Certificate{
		SerialNumber:          big.NewInt(1),
		Subject:               pkix.Name{CommonName: "mdok-test-ca"},
		NotBefore:             now.Add(-time.Hour),
		NotAfter:              now.AddDate(10, 0, 0),
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageDigitalSignature,
		BasicConstraintsValid: true,
		IsCA:                  true,
	}
	caDER, err := x509.CreateCertificate(rand.Reader, caTemplate, caTemplate, &caKey.PublicKey, caKey)
	if err != nil {
		return nil, "", err
	}
	caCertificate, err := x509.ParseCertificate(caDER)
	if err != nil {
		return nil, "", err
	}
	leafKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, "", err
	}
	leafTemplate := &x509.Certificate{
		SerialNumber:          big.NewInt(2),
		Subject:               pkix.Name{CommonName: "mdok-test-server"},
		NotBefore:             now.Add(-time.Hour),
		NotAfter:              now.AddDate(10, 0, 0),
		KeyUsage:              x509.KeyUsageDigitalSignature,
		ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		BasicConstraintsValid: true,
		DNSNames:              []string{"localhost"},
		IPAddresses:           []net.IP{net.ParseIP("127.0.0.1"), net.ParseIP("::1")},
	}
	leafDER, err := x509.CreateCertificate(rand.Reader, leafTemplate, caCertificate, &leafKey.PublicKey, caKey)
	if err != nil {
		return nil, "", err
	}
	file, err := os.CreateTemp("", "mdok-ca-*.pem")
	if err != nil {
		return nil, "", err
	}
	defer file.Close()
	if err := pem.Encode(file, &pem.Block{Type: "CERTIFICATE", Bytes: caDER}); err != nil {
		return nil, "", err
	}
	config := &tls.Config{
		Certificates: []tls.Certificate{{Certificate: [][]byte{leafDER}, PrivateKey: leafKey}},
		MinVersion:   tls.VersionTLS12,
	}
	return config, file.Name(), nil
}

func spawnHTTP(listener net.Listener, state *serverState) {
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				logEvent("accept_error", err.Error())
				continue
			}
			go serveConnection(conn, state)
		}
	}()
}

func spawnTLS(listener net.Listener, state *serverState, config *tls.Config) {
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				logEvent("accept_error", err.Error())
				continue
			}
			go func(conn net.Conn) {
				tlsConn := tls.Server(conn, config)
				if err := tlsConn.Handshake(); err != nil {
					logEvent("tls_error", err.Error())
					conn.Close()
					return
				}
				serveConnection(tlsConn, state)
			}(conn)
		}
	}()
}

func logEvent(event, message string) {
	fmt.Fprintf(os.Stderr, "%s\n", jsonMarshal(map[string]any{"event": event, "error": message}))
}

func serveConnection(stream io.ReadWriteCloser, state *serverState) {
	defer stream.Close()
	request, err := readRequest(stream)
	if err != nil {
		_ = writeResponse(stream, jsonResponse(400, map[string]any{"error": err.Error()}), false)
		return
	}
	response := route(request, state)
	_ = writeResponse(stream, response, strings.EqualFold(request.method, "HEAD"))
}

func readRequest(reader io.Reader) (*request, error) {
	data := make([]byte, 0, 4096)
	buffer := make([]byte, 4096)
	headerEnd := -1
	for headerEnd < 0 {
		n, err := reader.Read(buffer)
		if n > 0 {
			data = append(data, buffer[:n]...)
			if index := bytes.Index(data, []byte("\r\n\r\n")); index >= 0 {
				headerEnd = index + 4
				break
			}
		}
		if err != nil {
			return nil, fmt.Errorf("connection closed before request headers")
		}
		if len(data) > maxHeaderBytes {
			return nil, fmt.Errorf("request headers exceed %d bytes", maxHeaderBytes)
		}
	}
	headerText := string(data[:headerEnd-4])
	if !utf8.ValidString(headerText) {
		return nil, fmt.Errorf("request headers are not UTF-8")
	}
	lines := strings.Split(headerText, "\r\n")
	fields := strings.Fields(lines[0])
	if len(fields) < 1 {
		return nil, fmt.Errorf("missing request method")
	}
	method := fields[0]
	if len(fields) < 2 {
		return nil, fmt.Errorf("missing request target")
	}
	target := fields[1]
	var headers []headerPair
	for _, line := range lines[1:] {
		if line == "" {
			continue
		}
		name, value, ok := strings.Cut(line, ":")
		if !ok {
			return nil, fmt.Errorf("invalid request header")
		}
		headers = append(headers, headerPair{strings.TrimSpace(name), strings.TrimSpace(value)})
	}
	contentLength := uint64(0)
	for _, header := range headers {
		if strings.EqualFold(header.name, "content-length") {
			parsed, err := strconv.ParseUint(header.value, 10, 63)
			if err != nil {
				return nil, fmt.Errorf("invalid Content-Length")
			}
			contentLength = parsed
			break
		}
	}
	if contentLength > maxBodyBytes {
		return nil, fmt.Errorf("request body exceeds %d bytes", maxBodyBytes)
	}
	body := append([]byte(nil), data[headerEnd:]...)
	for uint64(len(body)) < contentLength {
		needed := contentLength - uint64(len(body))
		if needed > 8192 {
			needed = 8192
		}
		chunk := make([]byte, needed)
		n, err := reader.Read(chunk)
		if err != nil || n == 0 {
			return nil, fmt.Errorf("connection closed before request body")
		}
		body = append(body, chunk[:n]...)
	}
	body = body[:contentLength]
	path, query := splitTarget(target)
	return &request{
		method:  method,
		target:  target,
		path:    path,
		query:   query,
		headers: headers,
		body:    body,
	}, nil
}

func splitTarget(target string) (string, map[string][]string) {
	if rest, ok := strings.CutPrefix(target, "http://"); ok {
		if index := strings.IndexByte(rest, '/'); index >= 0 {
			target = rest[index:]
		}
	} else if rest, ok := strings.CutPrefix(target, "https://"); ok {
		if index := strings.IndexByte(rest, '/'); index >= 0 {
			target = rest[index:]
		}
	}
	rawPath, rawQuery, _ := strings.Cut(target, "?")
	path := rawPath
	if path == "" {
		path = "/"
	}
	query := map[string][]string{}
	for _, item := range strings.Split(rawQuery, "&") {
		if item == "" {
			continue
		}
		key, value, _ := strings.Cut(item, "=")
		decodedKey := percentDecode(key)
		query[decodedKey] = append(query[decodedKey], percentDecode(value))
	}
	return path, query
}

func writeResponse(stream io.Writer, resp *response, head bool) error {
	reason := reasonPhrase(resp.status)
	headers := make([]headerPair, 0, len(resp.headers)+2)
	headers = append(headers, resp.headers...)
	if resp.body.chunked {
		headers = append(headers, headerPair{"Transfer-Encoding", "chunked"})
	} else {
		headers = append(headers, headerPair{"Content-Length", strconv.Itoa(len(resp.body.fixed))})
	}
	headers = append(headers, headerPair{"Connection", "close"})
	if _, err := fmt.Fprintf(stream, "HTTP/1.1 %d %s\r\n", resp.status, reason); err != nil {
		return err
	}
	for _, header := range headers {
		if _, err := fmt.Fprintf(stream, "%s: %s\r\n", header.name, header.value); err != nil {
			return err
		}
	}
	if _, err := stream.Write([]byte("\r\n")); err != nil {
		return err
	}
	if head {
		return nil
	}
	flusher, _ := stream.(interface{ Flush() error })
	if resp.body.chunked {
		for _, chunk := range resp.body.chunks {
			if resp.chunkDelayMs > 0 {
				time.Sleep(time.Duration(resp.chunkDelayMs) * time.Millisecond)
			}
			if _, err := fmt.Fprintf(stream, "%x\r\n", len(chunk)); err != nil {
				return err
			}
			if _, err := stream.Write(chunk); err != nil {
				return err
			}
			if _, err := stream.Write([]byte("\r\n")); err != nil {
				return err
			}
			if flusher != nil {
				_ = flusher.Flush()
			}
		}
		if _, err := stream.Write([]byte("0\r\n\r\n")); err != nil {
			return err
		}
		if flusher != nil {
			_ = flusher.Flush()
		}
		return nil
	}
	body := resp.body.fixed
	if resp.closeAfter {
		if _, err := stream.Write(body[:len(body)/2]); err != nil {
			return err
		}
		if flusher != nil {
			_ = flusher.Flush()
		}
		return nil
	}
	if _, err := stream.Write(body); err != nil {
		return err
	}
	if flusher != nil {
		_ = flusher.Flush()
	}
	return nil
}

func route(request *request, state *serverState) *response {
	path := request.path
	switch {
	case path == "/health":
		return jsonResponse(200, map[string]any{"ok": true})
	case path == "/echo":
		return echoEndpoint(request)
	case path == "/headers":
		return headersEndpoint()
	case path == "/gzip":
		return gzipResponse(request)
	case path == "/upload":
		return uploadEndpoint(request)
	case path == "/multipart":
		return multipartEndpoint(request)
	case path == "/cookies/set":
		return cookiesSet(request)
	case path == "/cookies/echo":
		return cookiesEcho(request)
	case path == "/auth/basic":
		return basicAuth(request)
	case path == "/auth/bearer":
		return bearerAuth(request)
	case request.method == "POST" && path == "/auth/login":
		return login(request)
	case path == "/close/early" || path == "/early":
		return closeEarly()
	case strings.HasPrefix(path, "/status/"):
		return statusEndpoint(path)
	case strings.HasPrefix(path, "/json/"):
		return jsonCase(path)
	case strings.HasPrefix(path, "/redirect/"):
		return redirect(request, path)
	case strings.HasPrefix(path, "/delay/"):
		return delayEndpoint(path)
	case strings.HasPrefix(path, "/stream/"):
		return streamResponse(path)
	case strings.HasPrefix(path, "/binary/"):
		return binaryResponse(path)
	case strings.HasPrefix(path, "/retry/"):
		return retry(request, path, state)
	case strings.HasPrefix(path, "/large/"):
		return largeResponse(path)
	case path == "/users":
		return usersCollection(request, state)
	case strings.HasPrefix(path, "/users/"):
		return userEndpoint(request, path, state)
	default:
		return jsonResponse(404, map[string]any{"error": "not_found", "path": path})
	}
}

func echoEndpoint(request *request) *response {
	headersObject := map[string]any{}
	for _, header := range request.headers {
		key := strings.ToLower(header.name)
		values, _ := headersObject[key].([]string)
		headersObject[key] = append(values, header.value)
	}
	queryObject := map[string]any{}
	for key, values := range request.query {
		if len(values) == 1 {
			queryObject[key] = values[0]
			continue
		}
		list := make([]any, len(values))
		for i, value := range values {
			list[i] = value
		}
		queryObject[key] = list
	}
	var jsonBody any
	jsonOK := json.Unmarshal(request.body, &jsonBody) == nil
	if !jsonOK {
		jsonBody = nil
	}
	var body any
	if jsonOK {
		body = jsonBody
	} else {
		body = lossyString(request.body)
	}
	rawOK := utf8.Valid(request.body)
	var text any
	if rawOK {
		text = string(request.body)
	}
	var form any
	if contentType, ok := request.header("content-type"); ok {
		mediaType, _, _ := strings.Cut(contentType, ";")
		if strings.EqualFold(strings.TrimSpace(mediaType), "application/x-www-form-urlencoded") {
			form = parseForm(request.body)
		}
	}
	cookieHeader, _ := request.header("cookie")
	result := map[string]any{
		"method":          request.method,
		"path":            request.path,
		"target":          request.target,
		"query":           queryObject,
		"headers":         headersObject,
		"cookies":         parseCookies(cookieHeader),
		"body":            body,
		"json":            jsonBody,
		"form":            form,
		"text":            text,
		"body_size":       len(request.body),
		"raw_body_base64": base64.StdEncoding.EncodeToString(request.body),
	}
	if rawOK {
		result["raw_body"] = string(request.body)
	}
	return jsonResponse(200, result)
}

func parseForm(body []byte) any {
	fields := map[string]any{}
	for _, item := range strings.Split(lossyString(body), "&") {
		if item == "" {
			continue
		}
		key, value, _ := strings.Cut(item, "=")
		decodedKey := percentDecode(key)
		decodedValue := percentDecode(value)
		if existing, ok := fields[decodedKey]; ok {
			if list, ok := existing.([]any); ok {
				fields[decodedKey] = append(list, decodedValue)
			} else {
				fields[decodedKey] = []any{existing, decodedValue}
			}
		} else {
			fields[decodedKey] = decodedValue
		}
	}
	return fields
}

func statusEndpoint(path string) *response {
	code, err := strconv.ParseUint(strings.TrimPrefix(path, "/status/"), 10, 16)
	if err != nil || code < 100 || code > 599 {
		return jsonResponse(400, map[string]any{"error": "invalid_status"})
	}
	return jsonResponse(int(code), map[string]any{
		"status":  code,
		"ok":      code < 400,
		"value":   code,
		"message": fmt.Sprintf("status %d", code),
	})
}

func jsonCase(path string) *response {
	standard := map[string]any{
		"ok": true,
		"items": []any{
			map[string]any{"id": "a", "name": "Alpha", "value": 1},
			map[string]any{"id": "b", "name": "Beta", "value": 2},
			map[string]any{"id": "c", "name": "Gamma", "value": 3},
		},
		"tags":       []any{"red", "blue", "green"},
		"nested":     map[string]any{"value": 42, "array": []any{1, 2, 3}},
		"object":     map[string]any{"answer": 42, "enabled": true},
		"null_value": nil,
		"number":     123.5,
		"unicode":    "こんにちは, fixture 🌱",
	}
	caseName := strings.TrimPrefix(path, "/json/")
	var value any
	switch caseName {
	case "standard":
		value = standard
	case "empty":
		value = map[string]any{}
	case "null":
		value = nil
	case "array":
		value = []any{nil, false, 0, "text", map[string]any{"key": "value"}}
	case "numbers":
		value = map[string]any{"integer": 42, "negative": -7, "decimal": 3.25, "zero": 0}
	case "unicode":
		value = map[string]any{"text": "Grüße — こんにちは — 🌍"}
	case "nested":
		value = map[string]any{"a": map[string]any{"b": map[string]any{"c": []any{
			map[string]any{"value": 1}, map[string]any{"value": 2},
		}}}}
	case "booleans":
		value = map[string]any{"true": true, "false": false}
	default:
		value = map[string]any{"case": caseName, "value": standard}
	}
	return jsonResponse(200, value)
}

func headersEndpoint() *response {
	response := jsonResponse(200, map[string]any{"ok": true, "headers": "deterministic"})
	response.headers = append(response.headers,
		headerPair{"X-Duplicate", "one"},
		headerPair{"X-Duplicate", "two"},
		headerPair{"X-Mixed-Case", "Value"},
		headerPair{"X-Empty", ""},
		headerPair{"X-Long", strings.Repeat("x", 4096)},
	)
	return response
}

func basicAuth(request *request) *response {
	value, _ := request.header("authorization")
	trimmed := strings.TrimSpace(value)
	valid := trimmed == "Basic bWRvazpzZWNyZXQ=" || trimmed == "Basic mdok:secret"
	return authResponse(valid, "Basic realm=mdok")
}

func bearerAuth(request *request) *response {
	value, _ := request.header("authorization")
	valid := strings.TrimSpace(value) == "Bearer test-token"
	return authResponse(valid, "Bearer")
}

func authResponse(valid bool, challenge string) *response {
	status := 200
	if !valid {
		status = 401
	}
	response := jsonResponse(status, map[string]any{"authenticated": valid, "ok": valid})
	if !valid {
		response.headers = append(response.headers, headerPair{"WWW-Authenticate", challenge})
	}
	return response
}

func login(request *request) *response {
	var input any
	if err := json.Unmarshal(request.body, &input); err != nil {
		input = nil
	}
	email := ""
	password := ""
	if object, ok := input.(map[string]any); ok {
		if s, ok := object["email"].(string); ok {
			email = s
		}
		if s, ok := object["password"].(string); ok {
			password = s
		}
	}
	if email == "" || password != "test-password" {
		return jsonResponse(401, map[string]any{"authenticated": false, "error": "invalid_credentials"})
	}
	userID := "user-" + digestHex([]byte(email))[:12]
	name, _, _ := strings.Cut(email, "@")
	return jsonResponse(200, map[string]any{
		"access_token": "test-token",
		"token_type":   "Bearer",
		"user":         map[string]any{"id": userID, "email": email, "name": name},
	})
}

func usersCollection(request *request, state *serverState) *response {
	if request.method == "GET" {
		prefix := request.testKey() + "\x00"
		state.mu.Lock()
		users := []any{}
		for key, user := range state.users {
			if strings.HasPrefix(key, prefix) {
				users = append(users, cloneMap(user))
			}
		}
		state.mu.Unlock()
		return jsonResponse(200, map[string]any{"users": users, "count": len(users)})
	}
	if request.method != "POST" {
		return jsonResponse(405, map[string]any{"error": "method_not_allowed"})
	}
	var input any
	if err := json.Unmarshal(request.body, &input); err != nil {
		return jsonResponse(400, map[string]any{"error": "expected_json_object"})
	}
	object, ok := input.(map[string]any)
	if !ok {
		return jsonResponse(400, map[string]any{"error": "expected_json_object"})
	}
	id, hasID := object["id"].(string)
	if !hasID {
		id = "user-" + digestHex(request.body)[:12]
	}
	fields := cloneMap(object)
	user := userMap(id, fields)
	state.mu.Lock()
	state.users[request.testKey()+"\x00"+id] = cloneMap(user)
	state.mu.Unlock()
	return jsonResponse(201, user)
}

func userEndpoint(request *request, path string, state *serverState) *response {
	id := strings.TrimPrefix(path, "/users/")
	if id == "" {
		return jsonResponse(400, map[string]any{"error": "missing_user_id"})
	}
	key := request.testKey() + "\x00" + id
	switch request.method {
	case "GET":
		state.mu.Lock()
		user, ok := state.users[key]
		state.mu.Unlock()
		if !ok {
			user = userMap(id, map[string]any{})
		} else {
			user = cloneMap(user)
		}
		return jsonResponse(200, user)
	case "PUT", "PATCH":
		var input any
		if err := json.Unmarshal(request.body, &input); err != nil {
			return jsonResponse(400, map[string]any{"error": "expected_json_object"})
		}
		object, ok := input.(map[string]any)
		if !ok {
			return jsonResponse(400, map[string]any{"error": "expected_json_object"})
		}
		state.mu.Lock()
		var user map[string]any
		if existing, ok := state.users[key]; ok {
			user = cloneMap(existing)
		} else {
			user = userMap(id, map[string]any{})
		}
		for field, value := range object {
			user[field] = value
		}
		user["id"] = id
		state.users[key] = user
		state.mu.Unlock()
		return jsonResponse(200, user)
	case "DELETE":
		state.mu.Lock()
		_, removed := state.users[key]
		delete(state.users, key)
		state.mu.Unlock()
		return jsonResponse(200, map[string]any{"deleted": removed, "id": id})
	default:
		return jsonResponse(405, map[string]any{"error": "method_not_allowed"})
	}
}

func userMap(id string, fields map[string]any) map[string]any {
	fields["id"] = id
	if _, ok := fields["name"]; !ok {
		fields["name"] = "User " + id
	}
	if _, ok := fields["email"]; !ok {
		fields["email"] = id + "@example.com"
	}
	return fields
}

func cloneMap(source map[string]any) map[string]any {
	clone := make(map[string]any, len(source))
	for key, value := range source {
		clone[key] = value
	}
	return clone
}

func cookiesSet(request *request) *response {
	setObject := map[string]any{}
	for key, values := range request.query {
		list := make([]any, len(values))
		for i, value := range values {
			list[i] = value
		}
		setObject[key] = list
	}
	response := jsonResponse(200, map[string]any{"ok": true, "set": setObject})
	var cookies []string
	name, hasName := request.queryOne("name")
	value, hasValue := request.queryOne("value")
	if hasName && hasValue {
		cookies = append(cookies, fmt.Sprintf("%s=%s; Path=/", name, value))
	} else {
		keys := make([]string, 0, len(request.query))
		for key := range request.query {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			switch key {
			case "path", "domain", "max_age", "secure", "http_only":
				continue
			}
			for _, value := range request.query[key] {
				cookies = append(cookies, fmt.Sprintf("%s=%s; Path=/", key, value))
			}
		}
	}
	if secure, ok := request.queryOne("secure"); ok && secure == "true" {
		for i := range cookies {
			cookies[i] += "; Secure"
		}
	}
	if len(cookies) == 0 {
		cookies = []string{"fixture=ok; Path=/"}
	}
	for _, cookie := range cookies {
		response.headers = append(response.headers, headerPair{"Set-Cookie", cookie})
	}
	return response
}

func cookiesEcho(request *request) *response {
	raw, _ := request.header("cookie")
	return jsonResponse(200, map[string]any{"cookies": parseCookies(raw), "raw": raw})
}

func parseCookies(header string) map[string]string {
	cookies := map[string]string{}
	for _, item := range strings.Split(header, ";") {
		name, value, ok := strings.Cut(strings.TrimSpace(item), "=")
		if ok {
			cookies[strings.TrimSpace(name)] = strings.TrimSpace(value)
		}
	}
	return cookies
}

func redirect(request *request, path string) *response {
	count := uint64(0)
	if parsed, err := strconv.ParseUint(strings.TrimPrefix(path, "/redirect/"), 10, 32); err == nil {
		count = parsed
	}
	var result *response
	if count == 0 {
		if final, ok := request.queryOne("final"); ok && final == "/cookies/echo" {
			result = cookiesEcho(request)
		} else if final, ok := request.queryOne("final"); ok && final == "/health" {
			result = jsonResponse(200, map[string]any{"ok": true})
		} else {
			result = echoEndpoint(request)
		}
	} else {
		var target string
		if external, ok := request.queryOne("external"); ok && external == "true" {
			target = fmt.Sprintf("http://example.invalid/redirect/%d", count-1)
		} else if host, ok := request.queryOne("host"); ok {
			scheme := "http"
			if value, ok := request.queryOne("scheme"); ok {
				scheme = value
			}
			target = fmt.Sprintf("%s://%s/redirect/%d", scheme, host, count-1)
		} else {
			target = fmt.Sprintf("/redirect/%d", count-1)
		}
		if final, ok := request.queryOne("final"); ok {
			target = fmt.Sprintf("%s?final=%s", target, final)
		}
		result = fixedResponse(302, nil)
		result.headers = append(result.headers, headerPair{"Location", target})
	}
	if count == 0 {
		if statusText, ok := request.queryOne("redirect_status"); ok {
			status := 200
			if parsed, err := strconv.ParseUint(statusText, 10, 16); err == nil {
				status = int(parsed)
			}
			result.status = status
		}
	}
	return result
}

func delayEndpoint(path string) *response {
	ms := uint64(0)
	if parsed, err := strconv.ParseUint(strings.TrimPrefix(path, "/delay/"), 10, 64); err == nil {
		ms = parsed
	}
	ms = min(ms, 60_000)
	time.Sleep(time.Duration(ms) * time.Millisecond)
	return jsonResponse(200, map[string]any{"ok": true, "delay_ms": ms})
}

func streamResponse(path string) *response {
	values := strings.Split(strings.TrimPrefix(path, "/stream/"), "/")
	chunkCount := uint64(1)
	if len(values) >= 1 {
		if parsed, err := strconv.ParseUint(values[0], 10, 64); err == nil {
			chunkCount = min(parsed, 1024)
		}
	}
	delayMs := uint64(0)
	if len(values) >= 2 {
		if parsed, err := strconv.ParseUint(values[1], 10, 64); err == nil {
			delayMs = min(parsed, 60_000)
		}
	}
	chunks := make([][]byte, 0, chunkCount)
	for index := uint64(0); index < chunkCount; index++ {
		chunks = append(chunks, []byte(fmt.Sprintf("chunk-%d\n", index)))
	}
	result := &response{
		status: 200,
		headers: []headerPair{
			{"Content-Type", "text/plain; charset=utf-8"},
			{"X-Chunk-Delay-Ms", strconv.FormatUint(delayMs, 10)},
		},
		body:         responseBody{chunks: chunks, chunked: true},
		chunkDelayMs: delayMs,
	}
	if delayMs > 0 {
		result.headers = append(result.headers, headerPair{"X-Stream-Delay-Ms", strconv.FormatUint(delayMs, 10)})
	}
	return result
}

func binaryResponse(path string) *response {
	size := boundedSize(strings.TrimPrefix(path, "/binary/"))
	body := make([]byte, size)
	for index := range body {
		body[index] = byte(index)*31 + 7
	}
	result := fixedResponse(200, body)
	result.headers = append(result.headers, headerPair{"Content-Type", "application/octet-stream"})
	return result
}

func gzipResponse(request *request) *response {
	fixtureCase := "standard"
	if value, ok := request.queryOne("case"); ok {
		fixtureCase = value
	}
	value := map[string]any{
		"ok":       true,
		"encoding": "gzip",
		"case":     fixtureCase,
		"payload":  "deterministic",
	}
	var buffer bytes.Buffer
	encoder := gzip.NewWriter(&buffer)
	if _, err := encoder.Write(jsonMarshal(value)); err != nil {
		panic(fmt.Sprintf("gzip writes to memory: %v", err))
	}
	if err := encoder.Close(); err != nil {
		panic(fmt.Sprintf("gzip finishes in memory: %v", err))
	}
	result := fixedResponse(200, buffer.Bytes())
	result.headers = append(result.headers,
		headerPair{"Content-Type", "application/json"},
		headerPair{"Content-Encoding", "gzip"},
	)
	return result
}

func uploadEndpoint(request *request) *response {
	return jsonResponse(200, map[string]any{
		"size":   len(request.body),
		"sha256": digestHex(request.body),
	})
}

func multipartEndpoint(request *request) *response {
	contentType, _ := request.header("content-type")
	boundary := ""
	found := false
	for _, part := range strings.Split(contentType, ";") {
		if value, ok := strings.CutPrefix(strings.TrimSpace(part), "boundary="); ok {
			boundary = strings.Trim(value, "\"")
			found = true
			break
		}
	}
	if !found {
		return jsonResponse(400, map[string]any{"error": "missing_multipart_boundary"})
	}
	marker := []byte("--" + boundary)
	fields := map[string]any{}
	files := []any{}
	for _, part := range splitBytes(request.body, marker)[1:] {
		part = bytes.TrimPrefix(part, []byte("\r\n"))
		if bytes.HasPrefix(part, []byte("--")) {
			continue
		}
		headerEnd := bytes.Index(part, []byte("\r\n\r\n"))
		if headerEnd < 0 {
			continue
		}
		headerText := lossyString(part[:headerEnd])
		content := bytes.TrimSuffix(part[headerEnd+4:], []byte("\r\n"))
		disposition := ""
		for _, line := range strings.Split(headerText, "\n") {
			line = strings.TrimSuffix(line, "\r")
			if strings.HasPrefix(strings.ToLower(line), "content-disposition:") {
				disposition = line
				break
			}
		}
		if disposition == "" {
			continue
		}
		name, _ := dispositionParameter(disposition, "name")
		if filename, ok := dispositionParameter(disposition, "filename"); ok {
			files = append(files, map[string]any{
				"name":     name,
				"filename": filename,
				"size":     len(content),
				"sha256":   digestHex(content),
			})
		} else {
			fields[name] = lossyString(content)
		}
	}
	return jsonResponse(200, map[string]any{
		"fields": fields,
		"files":  files,
		"multipart": map[string]any{
			"fields": fields,
			"files":  files,
		},
	})
}

func splitBytes(input, marker []byte) [][]byte {
	if len(marker) == 0 {
		return [][]byte{input}
	}
	var result [][]byte
	start := 0
	for {
		relative := bytes.Index(input[start:], marker)
		if relative < 0 {
			break
		}
		index := start + relative
		result = append(result, input[start:index])
		start = index + len(marker)
	}
	result = append(result, input[start:])
	return result
}

func dispositionParameter(line, parameter string) (string, bool) {
	for _, item := range strings.Split(line, ";")[1:] {
		key, value, ok := strings.Cut(strings.TrimSpace(item), "=")
		if !ok {
			continue
		}
		if key == parameter {
			return strings.Trim(value, "\""), true
		}
	}
	return "", false
}

func closeEarly() *response {
	result := fixedResponse(200, []byte("this body is intentionally truncated"))
	result.closeAfter = true
	return result
}

func retry(request *request, path string, state *serverState) *response {
	failures := uint32(0)
	if parsed, err := strconv.ParseUint(strings.TrimPrefix(path, "/retry/"), 10, 32); err == nil {
		failures = uint32(min(parsed, 1000))
	}
	key := retryKey{testKey: request.testKey(), path: path}
	state.mu.Lock()
	attempt := state.retries[key] + 1
	state.retries[key] = attempt
	state.mu.Unlock()
	if attempt <= failures {
		return jsonResponse(503, map[string]any{
			"ok":        false,
			"attempt":   attempt,
			"remaining": failures + 1 - attempt,
		})
	}
	return jsonResponse(200, map[string]any{"ok": true, "attempt": attempt, "failures": failures})
}

func largeResponse(path string) *response {
	size := boundedSize(strings.TrimPrefix(path, "/large/"))
	pattern := []byte("mdok-large-fixture\n")
	body := make([]byte, 0, size)
	for len(body) < size {
		remaining := size - len(body)
		if remaining > len(pattern) {
			remaining = len(pattern)
		}
		body = append(body, pattern[:remaining]...)
	}
	result := fixedResponse(200, body)
	result.headers = append(result.headers, headerPair{"Content-Type", "application/octet-stream"})
	return result
}

func boundedSize(value string) int {
	size := uint64(0)
	if parsed, err := strconv.ParseUint(value, 10, 64); err == nil {
		size = parsed
	}
	size = min(size, maxGeneratedSize)
	return int(size)
}

func digestHex(value []byte) string {
	sum := sha256.Sum256(value)
	return hex.EncodeToString(sum[:])
}

func percentDecode(value string) string {
	data := []byte(value)
	output := make([]byte, 0, len(data))
	index := 0
	for index < len(data) {
		if data[index] == '+' {
			output = append(output, ' ')
			index++
		} else if data[index] == '%' && index+2 < len(data) {
			high, highOK := hexDigit(data[index+1])
			low, lowOK := hexDigit(data[index+2])
			if highOK && lowOK {
				output = append(output, high*16+low)
				index += 3
				continue
			}
			output = append(output, data[index])
			index++
		} else {
			output = append(output, data[index])
			index++
		}
	}
	return lossyString(output)
}

func hexDigit(value byte) (byte, bool) {
	switch {
	case value >= '0' && value <= '9':
		return value - '0', true
	case value >= 'a' && value <= 'f':
		return value - 'a' + 10, true
	case value >= 'A' && value <= 'F':
		return value - 'A' + 10, true
	default:
		return 0, false
	}
}

func reasonPhrase(status int) string {
	switch status {
	case 200:
		return "OK"
	case 201:
		return "Created"
	case 204:
		return "No Content"
	case 302:
		return "Found"
	case 400:
		return "Bad Request"
	case 401:
		return "Unauthorized"
	case 404:
		return "Not Found"
	case 405:
		return "Method Not Allowed"
	case 413:
		return "Payload Too Large"
	case 503:
		return "Service Unavailable"
	}
	switch {
	case status >= 100 && status < 200:
		return "Informational"
	case status >= 200 && status < 300:
		return "Success"
	case status >= 300 && status < 400:
		return "Redirect"
	case status >= 400 && status < 500:
		return "Client Error"
	default:
		return "Server Error"
	}
}

func lossyString(value []byte) string {
	return strings.ToValidUTF8(string(value), "�")
}

func jsonMarshal(value any) []byte {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		panic(fmt.Sprintf("JSON values in fixtures are serializable: %v", err))
	}
	return bytes.TrimSuffix(buffer.Bytes(), []byte("\n"))
}

func spawnProxy(listener net.Listener) {
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				continue
			}
			go proxyConnection(conn)
		}
	}()
}

func proxyConnection(client net.Conn) {
	defer client.Close()
	request, err := readRequest(client)
	if err != nil {
		return
	}
	if strings.EqualFold(request.method, "CONNECT") {
		address, ok := loopbackDestination(request.target)
		if !ok {
			return
		}
		upstream, err := net.Dial("tcp", address.String())
		if err != nil {
			return
		}
		defer upstream.Close()
		if _, err := client.Write([]byte("HTTP/1.1 200 Connection Established\r\n\r\n")); err != nil {
			return
		}
		done := make(chan struct{})
		go func() {
			defer close(done)
			_, _ = io.Copy(upstream, client)
			if tcp, ok := upstream.(*net.TCPConn); ok {
				_ = tcp.CloseWrite()
			}
		}()
		_, _ = io.Copy(client, upstream)
		<-done
		return
	}
	address, originForm, ok := proxyTarget(request)
	if !ok {
		return
	}
	upstream, err := net.Dial("tcp", address.String())
	if err != nil {
		return
	}
	defer upstream.Close()
	var head bytes.Buffer
	fmt.Fprintf(&head, "%s %s HTTP/1.1\r\n", request.method, originForm)
	for _, header := range request.headers {
		if !strings.EqualFold(header.name, "proxy-connection") {
			fmt.Fprintf(&head, "%s: %s\r\n", header.name, header.value)
		}
	}
	head.WriteString("\r\n")
	if _, err := upstream.Write(head.Bytes()); err != nil {
		return
	}
	if _, err := upstream.Write(request.body); err != nil {
		return
	}
	_, _ = io.Copy(client, upstream)
}

func proxyTarget(request *request) (*net.TCPAddr, string, bool) {
	rest, ok := strings.CutPrefix(request.target, "http://")
	if !ok {
		return nil, "", false
	}
	slash := strings.IndexByte(rest, '/')
	authority := rest
	originForm := "/"
	if slash >= 0 {
		authority = rest[:slash]
		originForm = rest[slash:]
	}
	address, ok := loopbackDestination(authority)
	if !ok {
		return nil, "", false
	}
	return address, originForm, true
}

func loopbackDestination(authority string) (*net.TCPAddr, bool) {
	if at := strings.LastIndexByte(authority, '@'); at >= 0 {
		authority = authority[at+1:]
	}
	address, err := net.ResolveTCPAddr("tcp", authority)
	if err != nil || address.IP == nil || !address.IP.IsLoopback() {
		return nil, false
	}
	return address, true
}
