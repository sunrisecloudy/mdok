// Package jmespath compiles and evaluates MDOK check and capture
// expressions against a finished transfer's evaluation context.
//
// It mirrors the semantics of the Rust mdok-jmespath crate:
//
//   - a check line must evaluate to the JSON boolean true: false is the
//     MDOK-E502 assertion failure and any other JSON type is an MDOK-E501
//     evaluation error ("check must return boolean, got <type>");
//   - a capture expression must evaluate to a JSON object (MDOK-E503)
//     whose keys are publishable variable names (MDOK-E504);
//   - compile failures, including empty expressions, are MDOK-E500.
//
// Every failure is returned as an *Error whose Diagnostic field carries
// the code, severity, title, and message for the report; File and Step
// are left empty for the caller to fill in.
package jmespath

import (
	"errors"
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"strings"

	gojmespath "github.com/jmespath/go-jmespath"

	"mdok/core"
)

// Transfer carries the execution facts of one finished HTTP transfer that
// check and capture expressions inspect. The shared core contract does not
// define a transfer type yet, so the evaluation shape lives here; the
// executor fills one in after each request and hands it to NewRoot.
type Transfer struct {
	Status          int
	Headers         map[string][]string
	RedirectCount   int
	Attempt         int
	DownloadedBytes int
	UploadedBytes   int
	UsedProxy       bool
}

// Root is the structured evaluation context for one executed transfer.
// Map renders it as the JSON object handed to expressions:
//
//	{"status": <number>, "body": <parsed JSON or null>, "headers": <map>,
//	 "transfer": {"redirect_count": <number>, "attempt": <number>, ...},
//	 "variables": <map>}
type Root struct {
	Status    int
	Body      any
	Headers   map[string]any
	Transfer  map[string]any
	Variables map[string]any
}

// NewRoot builds the evaluation context for one executed transfer.
//
// body is the raw response body: when it is non-empty and parses as JSON
// it becomes the "body" value (mirroring the Rust executor, which sniffs
// the body rather than trusting Content-Type); otherwise body stays nil.
// variables are the document variables plus captures published by earlier
// steps. All numbers are normalized to float64 so equality against JSON
// literals works under go-jmespath (see NormalizeValue).
func NewRoot(transfer *Transfer, body []byte, variables map[string]any) Root {
	root := Root{
		Variables: map[string]any{},
		Headers:   map[string]any{},
		Transfer: map[string]any{
			"redirect_count":   float64(0),
			"attempt":          float64(0),
			"downloaded_bytes": float64(0),
			"uploaded_bytes":   float64(0),
			"used_proxy":       false,
		},
	}
	if transfer != nil {
		root.Status = transfer.Status
		for name, values := range transfer.Headers {
			root.Headers[name] = values
		}
		root.Transfer = map[string]any{
			"redirect_count":   float64(transfer.RedirectCount),
			"attempt":          float64(transfer.Attempt),
			"downloaded_bytes": float64(transfer.DownloadedBytes),
			"uploaded_bytes":   float64(transfer.UploadedBytes),
			"used_proxy":       transfer.UsedProxy,
		}
	}
	for key, value := range variables {
		root.Variables[key] = NormalizeValue(value)
	}
	root.Body = parseBody(body)
	return root
}

// Map renders the root as the JSON object handed to JMESPath expressions.
func (r Root) Map() map[string]any {
	return map[string]any{
		"status":    float64(r.Status),
		"body":      r.Body,
		"headers":   r.Headers,
		"transfer":  r.Transfer,
		"variables": r.Variables,
	}
}

// parseBody decodes the response body when it is JSON. Like the Rust
// executor's body_value, the body is sniffed: any value that parses as
// JSON is used, anything else yields nil.
func parseBody(body []byte) any {
	trimmed := bytes.TrimSpace(body)
	if len(trimmed) == 0 {
		return nil
	}
	var value any
	if err := json.Unmarshal(trimmed, &value); err != nil {
		return nil
	}
	return NormalizeValue(value)
}

// NormalizeValue converts Go values (JSON- or TOML-decoded) to the
// canonical in-memory JSON shape used by go-jmespath: every number becomes
// float64, exactly like the encoding/json decoder.
//
// This is the workaround for the one go-jmespath quirk that matters here:
// the library implements == with reflect.DeepEqual, so an int-typed 200
// (as TOML decoders produce) would never equal the float64-typed 200 of a
// `200` JSON literal. Normalizing both sides of every comparison makes
// `status == `200“ behave like the Rust implementation.
func NormalizeValue(value any) any {
	switch value.(type) {
	case nil, bool, string, float64:
		return value
	}
	reflected := reflect.ValueOf(value)
	switch reflected.Kind() {
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return float64(reflected.Int())
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		return float64(reflected.Uint())
	case reflect.Float32:
		return reflected.Float()
	case reflect.Slice, reflect.Array:
		items := make([]any, reflected.Len())
		for index := 0; index < reflected.Len(); index++ {
			items[index] = NormalizeValue(reflected.Index(index).Interface())
		}
		return items
	case reflect.Map:
		normalized := make(map[string]any, reflected.Len())
		for _, key := range reflected.MapKeys() {
			normalized[fmt.Sprint(key.Interface())] = NormalizeValue(reflected.MapIndex(key).Interface())
		}
		return normalized
	default:
		return value
	}
}

// Error is a failed check or capture evaluation. Diagnostic carries the
// MDOK-E5xx code and message for the report.
type Error struct {
	Diagnostic core.Diagnostic
}

// Error renders the failure the way the Rust crate displays it, for
// example "MDOK-E500 invalid JMESPath syntax: expression is empty".
func (e *Error) Error() string {
	return e.Diagnostic.Code + " " + e.Diagnostic.Message
}

func newDiagnostic(code, title, message string) core.Diagnostic {
	return core.Diagnostic{
		Severity: core.SeverityError,
		Code:     code,
		Title:    title,
		Message:  message,
	}
}

func syntaxError(detail string) *Error {
	return &Error{Diagnostic: newDiagnostic(
		"MDOK-E500", "Invalid JMESPath",
		"invalid JMESPath syntax: "+detail)}
}

func runtimeError(title, detail string) *Error {
	return &Error{Diagnostic: newDiagnostic(
		"MDOK-E501", title,
		"JMESPath runtime or result type error: "+detail)}
}

func checkFailed(expression string) *Error {
	return &Error{Diagnostic: newDiagnostic(
		"MDOK-E502", "Check failed",
		"JMESPath check evaluated to false: "+expression)}
}

func captureNotObject() *Error {
	return &Error{Diagnostic: newDiagnostic(
		"MDOK-E503", "Invalid capture",
		"capture did not evaluate to an object")}
}

func invalidCaptureKey(detail string) *Error {
	return &Error{Diagnostic: newDiagnostic(
		"MDOK-E504", "Invalid capture name",
		"invalid or colliding capture key: "+detail)}
}

// Check evaluates one check-fence line and reports whether it passed.
//
// The result must be the JSON boolean true. False fails with MDOK-E502,
// any other JSON type fails with MDOK-E501, and compilation failures
// (including an empty line) fail with MDOK-E500. The returned error is
// always an *Error carrying the diagnostic.
func Check(line string, root map[string]any) (bool, error) {
	expression, err := compileExpression(line)
	if err != nil {
		return false, err
	}
	result, err := expression.Search(root)
	if err != nil {
		return false, runtimeError("Check evaluation failed", err.Error())
	}
	switch value := result.(type) {
	case bool:
		if value {
			return true, nil
		}
		return false, checkFailed(line)
	default:
		return false, runtimeError("Check evaluation failed",
			fmt.Sprintf("check must return boolean, got %s", JSONType(result)))
	}
}

// Capture evaluates one capture-fence expression. The result must be a
// JSON object and every key must be a valid capture key; the returned map
// is ready for the caller's merge step (MergeCaptures applies the Rust
// collision rules). Non-object results fail with MDOK-E503, invalid keys
// with MDOK-E504, and compile failures with MDOK-E500.
func Capture(expr string, root map[string]any) (map[string]any, error) {
	expression, err := compileExpression(expr)
	if err != nil {
		return nil, err
	}
	result, err := expression.Search(root)
	if err != nil {
		return nil, runtimeError("Capture evaluation failed", err.Error())
	}
	object, ok := result.(map[string]any)
	if !ok {
		return nil, captureNotObject()
	}
	captures := make(map[string]any, len(object))
	for key, value := range object {
		if !IsCaptureKey(key) {
			return nil, invalidCaptureKey(key)
		}
		captures[key] = NormalizeValue(value)
	}
	return captures, nil
}

// MergeCaptures merges captures into target. Mirroring the Rust crate,
// any key already present in target fails with MDOK-E504 instead of
// overwriting an existing variable.
func MergeCaptures(target map[string]any, captures map[string]any) error {
	for key := range captures {
		if _, exists := target[key]; exists {
			return invalidCaptureKey("capture key collision")
		}
	}
	for key, value := range captures {
		target[key] = value
	}
	return nil
}

// IsCaptureKey reports whether key is a publishable variable name: 1 to
// 64 bytes, starting with an ASCII letter and continuing with ASCII
// letters, digits, underscores, or hyphens.
func IsCaptureKey(key string) bool {
	if key == "" || len(key) > 64 {
		return false
	}
	for index := 0; index < len(key); index++ {
		character := key[index]
		isLetter := character >= 'a' && character <= 'z' ||
			character >= 'A' && character <= 'Z'
		isDigit := character >= '0' && character <= '9'
		if index == 0 && !isLetter {
			return false
		}
		if !isLetter && !isDigit && character != '_' && character != '-' {
			return false
		}
	}
	return true
}

// JSONType names the JSON type of a decoded value the way the Rust crate
// reports it in "check must return boolean, got <type>" messages.
func JSONType(value any) string {
	switch value.(type) {
	case nil:
		return "null"
	case bool:
		return "boolean"
	case float64:
		return "number"
	case string:
		return "string"
	case []any:
		return "array"
	case map[string]any:
		return "object"
	default:
		return "unknown"
	}
}

// compileExpression compiles source after normalization. Empty and
// whitespace-only sources are MDOK-E500, matching the Rust crate.
func compileExpression(source string) (*gojmespath.JMESPath, error) {
	if strings.TrimSpace(source) == "" {
		return nil, syntaxError("expression is empty")
	}
	expression, err := gojmespath.Compile(NormalizeExpression(source))
	if err != nil {
		return nil, syntaxError(err.Error())
	}
	return expression, nil
}

// Validate compiles an expression without evaluating it, for plan-time
// syntax checking. Returns an MDOK-E500 diagnostic on failure, nil when the
// expression compiles (mirrors the Rust crate compiling checks at plan).
func Validate(expr string) *core.Diagnostic {
	if _, err := compileExpression(expr); err != nil {
		var jerr *Error
		if errors.As(err, &jerr) {
			return &jerr.Diagnostic
		}
		return &core.Diagnostic{Severity: core.SeverityError, Code: "MDOK-E500",
			Title: "Invalid JMESPath", Message: err.Error()}
	}
	return nil
}

// NormalizeExpression rewrites backtick literals that hold a bare
// identifier (for example `foo-bar`) into quoted raw strings ('foo-bar').
//
// The JMESPath grammar only allows valid JSON inside backticks, but the
// Rust CLI accepts bare identifiers there and rewrites them before
// compiling; this shim mirrors that behavior so the same documents work
// in both implementations. Valid JSON literals such as `200`, `true`, or
// `{"a":1}` pass through unchanged, which is why every expression in the
// e2e corpus compiles exactly as written.
func NormalizeExpression(expression string) string {
	runes := []rune(expression)
	var normalized strings.Builder
	normalized.Grow(len(expression))
	var quote rune
	escaped := false
	for index := 0; index < len(runes); index++ {
		character := runes[index]
		if quote != 0 {
			normalized.WriteRune(character)
			switch {
			case escaped:
				escaped = false
			case character == '\\':
				escaped = true
			case character == quote:
				quote = 0
			}
			continue
		}
		if character == '\'' || character == '"' {
			quote = character
			normalized.WriteRune(character)
			continue
		}
		if character != '`' {
			normalized.WriteRune(character)
			continue
		}
		// Scan the backtick literal.
		end := index + 1
		for end < len(runes) && runes[end] != '`' {
			end++
		}
		closed := end < len(runes)
		literal := string(runes[index+1 : end])
		if closed && isBareLiteral(literal) {
			normalized.WriteByte('\'')
			normalized.WriteString(literal)
			normalized.WriteByte('\'')
		} else {
			normalized.WriteByte('`')
			normalized.WriteString(literal)
			if closed {
				normalized.WriteByte('`')
			}
		}
		index = end
	}
	return normalized.String()
}

// isBareLiteral reports whether a backtick literal looks like a bare
// identifier (alphabetic first character, then alphanumerics, underscores,
// or hyphens) and is not itself valid JSON.
func isBareLiteral(literal string) bool {
	if literal == "" {
		return false
	}
	first := literal[0]
	if !(first >= 'a' && first <= 'z' || first >= 'A' && first <= 'Z' || first == '_') {
		return false
	}
	for index := 1; index < len(literal); index++ {
		character := literal[index]
		isAlphanumeric := character >= 'a' && character <= 'z' ||
			character >= 'A' && character <= 'Z' ||
			character >= '0' && character <= '9'
		if !isAlphanumeric && character != '_' && character != '-' {
			return false
		}
	}
	return !json.Valid([]byte(literal))
}
