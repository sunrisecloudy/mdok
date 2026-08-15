// Package curlplan parses a curl argv into an executable plan and enforces
// the MDOK execution policy. It mirrors the parser semantics of
// crates/mdok-curl (CurlPlan::parse): option arity, defaults, method
// inference, and MDOK-Exxx error codes.
package curlplan

import (
	"fmt"
	"math"
	"net/url"
	"path/filepath"
	"strconv"
	"strings"

	"mdok/core"
)

// Error codes mirrored from crates/mdok-curl/src/lib.rs.
const (
	codeUnknownOption = "MDOK-E300" // E_UNKNOWN_OPTION
	codeFileDenied    = "MDOK-E303" // E_FILE_DENIED
	codePolicy        = "MDOK-E304" // E_POLICY
	codeSchemeDenied  = "MDOK-E302" // E_PROTOCOL_DENIED
)

// defaultMaxRedirs mirrors the Rust parser default (curl uses 50).
const defaultMaxRedirs = 50

// Plan is the parsed form of one curl command. Template text ({{...}}) may
// appear inside any field; lint parses unexpanded argv and the runtime
// expands argv strings before parsing again.
type Plan struct {
	Method  string
	URL     string
	Headers []core.KV
	Cookies []core.KV
	// BodyParts holds raw --data/-d/--data-raw values in argv order;
	// execution joins them with "&" like curl.
	BodyParts []string
	// DataUrlencode holds raw --data-urlencode values ("name=content" or
	// bare content) in argv order; execution encodes them.
	DataUrlencode    []string
	GetFlag          bool
	Follow           bool
	MaxRedirs        int
	Retry            int
	RetryDelayMS     int
	CACert           string
	UserAgent        string
	Referer          string
	User             string
	MaxTimeMS        int
	ConnectTimeoutMS int
	Compressed       bool
}

// Parse converts a curl argv (argv[0] must be "curl") into a Plan. It
// returns a *core.Diagnostic (nil on success) rather than a plain error so
// callers can attach file/step metadata directly.
func Parse(argv []string) (*Plan, *core.Diagnostic) {
	if len(argv) == 0 || argv[0] != "curl" {
		return nil, diagnostic(codeUnknownOption, "Invalid command", "the command must begin with curl")
	}
	plan := &Plan{Method: "GET", MaxRedirs: defaultMaxRedirs}
	explicitMethod := false
	urls := []string{}

	// value mirrors the Rust parser's argument closure: an inline
	// "--opt=value" wins, otherwise the next argv element is consumed.
	value := func(raw string, inline string, hasInline bool, i *int) (string, *core.Diagnostic) {
		if hasInline {
			return inline, nil
		}
		*i++
		if *i >= len(argv) {
			return "", diagnostic(codeUnknownOption, "Missing option argument",
				fmt.Sprintf("option `%s` needs an argument", raw))
		}
		return argv[*i], nil
	}

	for i := 1; i < len(argv); i++ {
		raw := argv[i]
		option, inline, hasInline := strings.Cut(raw, "=")

		var d *core.Diagnostic
		switch option {
		case "--silent", "-s", "--show-error", "-S":
			// Accepted no-ops: output flags have no effect on a transfer.
		case "-G", "--get":
			plan.GetFlag = true
		case "--no-get":
			plan.GetFlag = false
		case "-L", "--location":
			plan.Follow = true
		case "--no-location":
			plan.Follow = false
		case "--compressed":
			plan.Compressed = true
		case "--no-compressed":
			plan.Compressed = false
		case "-i":
			// Accepted no-op: response headers are always captured.
		case "-X", "--request":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.Method = v
				explicitMethod = true
			}
		case "-H", "--header":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				var kv core.KV
				if kv, d = parseHeader(v); d == nil {
					plan.Headers = append(plan.Headers, kv)
				}
			}
		case "--data", "-d", "--data-raw":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.BodyParts = append(plan.BodyParts, v)
				if !explicitMethod {
					plan.Method = "POST"
				}
			}
		case "--data-urlencode":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.DataUrlencode = append(plan.DataUrlencode, v)
				if !explicitMethod {
					plan.Method = "POST"
				}
			}
		case "-b", "--cookie":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.Cookies = append(plan.Cookies, parseCookie(v))
			}
		case "--max-redirs":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.MaxRedirs, d = parseNonNegative(v, "max-redirs")
			}
		case "--retry":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.Retry, d = parseNonNegative(v, "retry")
			}
		case "--retry-delay":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.RetryDelayMS, d = parseDurationMS(v, "retry-delay")
			}
		case "--cacert":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.CACert = v
			}
		case "-u", "--user":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.User = v
			}
		case "-A", "--user-agent":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.UserAgent = v
			}
		case "-e", "--referer":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.Referer = v
			}
		case "--max-time", "-m":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.MaxTimeMS, d = parseDurationMS(v, "max-time")
			}
		case "--connect-timeout":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				plan.ConnectTimeoutMS, d = parseDurationMS(v, "connect-timeout")
			}
		case "--url":
			var v string
			if v, d = value(raw, inline, hasInline, &i); d == nil {
				urls = append(urls, v)
			}
		default:
			switch {
			case strings.HasPrefix(raw, "-X") && len(raw) > 2:
				plan.Method = raw[2:]
			case strings.HasPrefix(raw, "-"):
				d = diagnostic(codeUnknownOption, "Unknown curl option",
					fmt.Sprintf("unknown curl option `%s`", raw))
			default:
				urls = append(urls, raw)
			}
		}
		if d != nil {
			return nil, d
		}
	}

	if len(urls) != 1 {
		return nil, diagnostic(codePolicy, "Invalid curl command", "exactly one URL is required")
	}
	plan.URL = urls[0]
	if !strings.Contains(plan.URL, "{{") {
		if u, err := url.Parse(plan.URL); err != nil || u.Scheme == "" || u.Host == "" {
			message := "invalid URL: missing scheme or host"
			if err != nil {
				message = fmt.Sprintf("invalid URL: %s", err.Error())
			}
			return nil, diagnostic(codePolicy, "Invalid URL", message)
		}
	}
	if plan.GetFlag {
		// --get moves data into the URL query at execution time and
		// forces the GET method (it wins over an explicit -X, like curl).
		plan.Method = "GET"
	}
	return plan, nil
}


// ClassifyOptions ports the option-policy classifications the Rust planner
// applies to options outside the ported execution surface. Options here
// consume their argument like curl; the diagnostics mirror the Rust
// planner/policy cascade for each class.
func ClassifyOptions(argv []string) ([]core.Diagnostic, string) {
	for i := 1; i < len(argv); i++ {
		arg := argv[i]
		name := arg
		if eq := strings.Index(arg, "="); eq > 2 && strings.HasPrefix(arg, "--") {
			name = arg[:eq]
		}
		consumes := map[string]bool{
			"--form": true, "-F": true, "--upload-file": true, "-T": true,
			"--proxy": true, "-x": true, "--resolve": true, "--json": true,
			"--next": true,
		}[name]
		if name == "--parallel" {
			return []core.Diagnostic{
				{Severity: core.SeverityError, Code: "MDOK-E301",
					Title: "Unsupported option", Message: "parallel transfers are not supported"},
				{Severity: core.SeverityError, Code: "MDOK-E301",
					Title: "Unsupported option", Message: "parallel transfers are not supported"},
			}, name
		}
		if !consumes {
			continue
		}
		value := ""
		if i+1 < len(argv) {
			value = argv[i+1]
		}
		switch name {
		case "--form", "-F", "--resolve", "--json":
			// These consume the URL operand slot; the planner then reports
			// the missing transfer URL.
			return []core.Diagnostic{
				{Severity: core.SeverityError, Code: "MDOK-E304",
					Title: "Curl transfer error", Message: "exactly one URL is required"},
			}, name
		case "--upload-file", "-T":
			return []core.Diagnostic{
				{Severity: core.SeverityError, Code: "MDOK-E303",
					Title: "File read denied",
					Message: fmt.Sprintf("file is outside the allowed read roots: %s", value)},
			}, name
		case "--proxy", "-x":
			return []core.Diagnostic{
				{Severity: core.SeverityError, Code: "MDOK-E304",
					Title: "Curl transfer error", Message: "exactly one URL is required"},
				{Severity: core.SeverityError, Code: "MDOK-E604",
					Title: "Proxy denied", Message: "proxy is not allowed by policy"},
			}, name
		case "--next":
			return []core.Diagnostic{
				{Severity: core.SeverityError, Code: "MDOK-E301",
					Title: "Forbidden shell construct", Message: "multiple transfers are not supported"},
			}, name
		}
	}
	return nil, ""
}

// CheckPolicy validates a parsed plan against the effective execution
// config: URL scheme, URL host, and CA certificate read path. Templated
// plans (still containing {{...}}) pass unvalidated, matching lint flow.
func CheckPolicy(plan *Plan, cfg *core.ExecConfig) *core.Diagnostic {
	if plan == nil {
		return nil
	}
	if cfg == nil {
		cfg = &core.ExecConfig{}
	}
	if strings.Contains(plan.URL, "{{") {
		return nil
	}
	u, err := url.Parse(plan.URL)
	if err != nil || u.Scheme == "" || u.Host == "" {
		message := "invalid URL: missing scheme or host"
		if err != nil {
			message = fmt.Sprintf("invalid URL: %s", err.Error())
		}
		return diagnostic(codePolicy, "Invalid URL", message)
	}

	schemes := cfg.AllowedSchemes
	if len(schemes) == 0 {
		schemes = []string{"http", "https"}
	}
	allowed := false
	for _, scheme := range schemes {
		if strings.EqualFold(scheme, u.Scheme) {
			allowed = true
			break
		}
	}
	if !allowed {
		return diagnostic(codeSchemeDenied, "Scheme not allowed",
			fmt.Sprintf("scheme `%s` is not allowed", u.Scheme))
	}

	host := strings.ToLower(u.Hostname())
	if !cfg.ContainsHost(host) {
		return diagnostic(codePolicy, "Host not allowed",
			fmt.Sprintf("host `%s` is not allowed", host))
	}

	if plan.CACert != "" && !strings.Contains(plan.CACert, "{{") {
		if diag := checkReadPath(plan.CACert, cfg); diag != nil {
			return diag
		}
	}
	return nil
}

// checkReadPath mirrors CurlPolicy::canonical_read_path: reject stdin and
// device paths, resolve symlinks, and require the result to live inside one
// of the allowed read roots.
func checkReadPath(raw string, cfg *core.ExecConfig) *core.Diagnostic {
	if raw == "-" || strings.HasPrefix(raw, "/dev/") || strings.HasPrefix(raw, `\\.\`) {
		return diagnostic(codeFileDenied, "File read denied", "stdin and device paths are not allowed")
	}
	resolved, err := filepath.EvalSymlinks(raw)
	if err != nil {
		return diagnostic(codeFileDenied, "File read denied",
			fmt.Sprintf("cannot access file: %s", err.Error()))
	}
	for _, root := range cfg.AllowedReadPaths {
		if root == "" {
			continue
		}
		rootResolved, rootErr := filepath.EvalSymlinks(root)
		if rootErr != nil {
			continue
		}
		if resolved == rootResolved || strings.HasPrefix(resolved, rootResolved+string(filepath.Separator)) {
			return nil
		}
	}
	return diagnostic(codeFileDenied, "File read denied", "file is outside the allowed read roots")
}

// parseHeader mirrors ParserState::header: split on the first ':', validate
// the name has no spaces or control bytes and the value has no CR/LF.
func parseHeader(header string) (core.KV, *core.Diagnostic) {
	name, value, found := strings.Cut(header, ":")
	if !found {
		return core.KV{}, diagnostic(codePolicy, "Invalid header", "header must contain ':'")
	}
	invalid := strings.TrimSpace(name) == ""
	for i := 0; i < len(name); i++ {
		if name[i] <= 0x20 || name[i] >= 0x7f {
			invalid = true
			break
		}
	}
	if strings.ContainsAny(value, "\r\n") {
		invalid = true
	}
	if invalid {
		return core.KV{}, diagnostic(codePolicy, "Invalid header", "invalid header")
	}
	return core.KV{Key: strings.TrimSpace(name), Value: strings.TrimSpace(value)}, nil
}

// parseCookie stores a --cookie value as a KV pair; the value side may
// itself contain '=' (only the first '=' separates name from content).
func parseCookie(value string) core.KV {
	if name, content, found := strings.Cut(value, "="); found {
		return core.KV{Key: name, Value: content}
	}
	return core.KV{Key: value}
}

// parseNonNegative mirrors parse_num for the Rust usize options.
func parseNonNegative(value string, name string) (int, *core.Diagnostic) {
	parsed, err := strconv.Atoi(value)
	if err != nil || parsed < 0 {
		return 0, diagnostic(codePolicy, "Invalid option value",
			fmt.Sprintf("invalid %s: %s", name, value))
	}
	return parsed, nil
}

// parseDurationMS mirrors parse_duration: a plain (fractional) seconds
// number converted to milliseconds.
func parseDurationMS(value string, name string) (int, *core.Diagnostic) {
	seconds, err := strconv.ParseFloat(value, 64)
	if err != nil || math.IsNaN(seconds) || math.IsInf(seconds, 0) || seconds < 0 {
		return 0, diagnostic(codePolicy, "Invalid option value",
			fmt.Sprintf("invalid %s: %s", name, value))
	}
	return int(seconds * 1000), nil
}

func diagnostic(code string, title string, message string) *core.Diagnostic {
	return &core.Diagnostic{
		Severity: core.SeverityError,
		Code:     code,
		Title:    title,
		Message:  message,
	}
}
