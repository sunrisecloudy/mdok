// Package template implements the mdok {{path|filter}} expansion engine,
// ported from crates/mdok-template. Templates may appear anywhere inside a
// word; every "{{" ... "}}" region is replaced by a variable rendered through
// an optional single filter (string, raw, json, url, header, base64).
package template

import (
	"encoding/base64"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"

	"mdok/core"
)

// Resource limits shared with the shell tokenizer and the CLI preflight.
const (
	MaxSourceBytes    = 1024 * 1024
	MaxParts          = 4096
	MaxExpansionDepth = 32
	MaxRenderedBytes  = 8 * 1024 * 1024
)

// Filter selects how a resolved value is rendered.
type Filter string

// The six language-version-1 filters.
const (
	FilterString Filter = "string"
	FilterRaw    Filter = "raw"
	FilterJSON   Filter = "json"
	FilterURL    Filter = "url"
	FilterHeader Filter = "header"
	FilterBase64 Filter = "base64"
)

// PathPart is one dotted key or [index] segment of a variable path.
type PathPart struct {
	Key     string
	Index   int
	IsIndex bool
}

// Expression is a parsed {{path|filter}} region.
type Expression struct {
	Path   []PathPart
	Filter Filter
}

// Part is one parsed template segment: either Literal text or an Expression.
// Exactly one of the fields is meaningful, discriminated by Expr == nil.
type Part struct {
	Literal string
	Expr    *Expression
}

// Expand parses input and renders it against vars. Missing variables,
// syntax errors, filter type errors, and resource limit breaches are
// reported as *core.Diagnostic with the MDOK-E4xx codes used by the Rust
// engine (E400 syntax, E401 missing, E402 type/filter, E403 unsafe header,
// E404 limits).
func Expand(input string, vars map[string]any) (string, *core.Diagnostic) {
	parts, diag := Parse(input)
	if diag != nil {
		return "", diag
	}
	var out strings.Builder
	for _, part := range parts {
		if part.Expr == nil {
			if out.Len()+len(part.Literal) > MaxRenderedBytes {
				return "", limitDiag(fmt.Sprintf("rendered template exceeds %d bytes", MaxRenderedBytes))
			}
			out.WriteString(part.Literal)
			continue
		}
		remaining := MaxRenderedBytes - out.Len()
		rendered, diag := renderExpression(part.Expr, vars, remaining)
		if diag != nil {
			return "", diag
		}
		out.WriteString(rendered)
	}
	return out.String(), nil
}

// Parse splits source into literal and expression parts, mirroring
// mdok_template::parse. A "}}" that appears before any "{{" is an unmatched
// closing delimiter error.
func Parse(source string) ([]Part, *core.Diagnostic) {
	if len(source) > MaxSourceBytes {
		return nil, limitDiag(fmt.Sprintf("template source exceeds %d bytes", MaxSourceBytes))
	}
	parts := make([]Part, 0, 2)
	push := func(part Part) *core.Diagnostic {
		if len(parts) >= MaxParts {
			return limitDiag(fmt.Sprintf("template has more than %d parts", MaxParts))
		}
		parts = append(parts, part)
		return nil
	}
	cursor := 0
	for cursor < len(source) {
		relStart := strings.Index(source[cursor:], "{{")
		relEnd := strings.Index(source[cursor:], "}}")
		if relEnd >= 0 && (relStart < 0 || relEnd < relStart) {
			return nil, syntaxDiag("unmatched `}}`")
		}
		if relStart < 0 {
			if diag := push(Part{Literal: source[cursor:]}); diag != nil {
				return nil, diag
			}
			break
		}
		start := cursor + relStart
		if start > cursor {
			if diag := push(Part{Literal: source[cursor:start]}); diag != nil {
				return nil, diag
			}
		}
		relClose := strings.Index(source[start+2:], "}}")
		if relClose < 0 {
			return nil, syntaxDiag("unclosed `{{`")
		}
		end := start + 2 + relClose
		expression, diag := parseExpression(source[start+2 : end])
		if diag != nil {
			return nil, diag
		}
		if diag := push(Part{Expr: expression}); diag != nil {
			return nil, diag
		}
		cursor = end + 2
	}
	if source == "" {
		if diag := push(Part{Literal: ""}); diag != nil {
			return nil, diag
		}
	}
	return parts, nil
}

// parseExpression parses the inner text of a {{...}} region into a path and
// an optional filter.
func parseExpression(source string) (*Expression, *core.Diagnostic) {
	input := strings.TrimSpace(source)
	if input == "" {
		return nil, syntaxDiag("empty template")
	}
	pieces := strings.Split(input, "|")
	pathText := strings.TrimSpace(pieces[0])
	filterNames := pieces[1:]
	if len(filterNames) > 1 || (len(filterNames) == 1 && strings.TrimSpace(filterNames[0]) == "") {
		return nil, syntaxDiag("template must contain at most one filter")
	}
	filter := FilterString
	if len(filterNames) == 1 {
		name := strings.TrimSpace(filterNames[0])
		switch Filter(name) {
		case FilterString, FilterRaw, FilterJSON, FilterURL, FilterHeader, FilterBase64:
			filter = Filter(name)
		default:
			return nil, syntaxDiag(fmt.Sprintf("unknown filter `%s`", name))
		}
	}

	runes := []rune(pathText)
	pos := 0
	readIdentifier := func() (string, *core.Diagnostic) {
		if pos >= len(runes) || !isIdentStart(runes[pos]) {
			return "", syntaxDiag("path must start with an identifier")
		}
		start := pos
		for pos < len(runes) && isIdentChar(runes[pos]) {
			pos++
		}
		return string(runes[start:pos]), nil
	}

	first, diag := readIdentifier()
	if diag != nil {
		return nil, diag
	}
	path := []PathPart{{Key: first}}
	if len(path) > MaxExpansionDepth {
		return nil, limitDiag(fmt.Sprintf("template expansion depth exceeds %d", MaxExpansionDepth))
	}
	for pos < len(runes) {
		switch runes[pos] {
		case '.':
			pos++
			identifier, diag := readIdentifier()
			if diag != nil {
				return nil, diag
			}
			path = append(path, PathPart{Key: identifier})
		case '[':
			pos++
			start := pos
			for pos < len(runes) && runes[pos] >= '0' && runes[pos] <= '9' {
				pos++
			}
			if pos == start || pos >= len(runes) || runes[pos] != ']' {
				return nil, syntaxDiag("array index must be a non-negative integer")
			}
			index, err := strconv.Atoi(string(runes[start:pos]))
			if err != nil {
				return nil, syntaxDiag("array index is too large")
			}
			pos++
			path = append(path, PathPart{Index: index, IsIndex: true})
		default:
			return nil, syntaxDiag("unexpected character in variable path")
		}
		if len(path) > MaxExpansionDepth {
			return nil, limitDiag(fmt.Sprintf("template expansion depth exceeds %d", MaxExpansionDepth))
		}
	}
	return &Expression{Path: path, Filter: filter}, nil
}

func isIdentStart(r rune) bool {
	return (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || r == '_'
}

func isIdentChar(r rune) bool {
	return isIdentStart(r) || (r >= '0' && r <= '9') || r == '-'
}

// lookup resolves a dotted/[index] path against vars. Any missing segment is
// reported as MDOK-E401 with the missing key (or "[index]") as the message,
// matching mdok_template::lookup.
func lookup(vars map[string]any, path []PathPart) (any, *core.Diagnostic) {
	if len(path) == 0 || path[0].IsIndex {
		return nil, syntaxDiag("path must start with a key")
	}
	value, ok := vars[path[0].Key]
	if !ok {
		return nil, missingDiag(path[0].Key)
	}
	for _, part := range path[1:] {
		if part.IsIndex {
			array, ok := value.([]any)
			if !ok || part.Index >= len(array) {
				return nil, missingDiag(fmt.Sprintf("[%d]", part.Index))
			}
			value = array[part.Index]
			continue
		}
		object, ok := value.(map[string]any)
		if !ok {
			return nil, missingDiag(part.Key)
		}
		value, ok = object[part.Key]
		if !ok {
			return nil, missingDiag(part.Key)
		}
	}
	return value, nil
}

// renderExpression renders one expression with at most maxBytes of budget,
// mirroring mdok_template::render_expression_with_limit.
func renderExpression(expression *Expression, vars map[string]any, maxBytes int) (string, *core.Diagnostic) {
	value, diag := lookup(vars, expression.Path)
	if diag != nil {
		return "", diag
	}
	switch expression.Filter {
	case FilterJSON:
		encoded := encodeJSON(value)
		if len(encoded) > maxBytes {
			return "", limitDiag(fmt.Sprintf("rendered value exceeds %d bytes", maxBytes))
		}
		return encoded, nil
	case FilterBase64:
		byteCount, bytes, ok := base64Input(value)
		if !ok {
			return "", typeDiag("base64 expects a string or byte array")
		}
		if (byteCount+2)/3*4 > maxBytes {
			return "", limitDiag(fmt.Sprintf("rendered value exceeds %d bytes", maxBytes))
		}
		return base64.StdEncoding.EncodeToString(bytes), nil
	case FilterURL:
		scalar, diag := renderScalar(value, maxBytes)
		if diag != nil {
			return "", diag
		}
		if len(scalar)*3 > maxBytes {
			return "", limitDiag(fmt.Sprintf("rendered URL value exceeds %d bytes", maxBytes))
		}
		return percentEncode(scalar), nil
	case FilterHeader:
		scalar, diag := renderScalar(value, maxBytes)
		if diag != nil {
			return "", diag
		}
		if strings.ContainsAny(scalar, "\r\n") {
			return "", &core.Diagnostic{
				Severity: core.SeverityError,
				Code:     "MDOK-E403",
				Title:    "unsafe header value",
				Message:  "unsafe header value",
			}
		}
		return scalar, nil
	default: // string | raw
		return renderScalar(value, maxBytes)
	}
}

// renderScalar renders a scalar value (string, bool, number, null) and
// rejects composite values, mirroring mdok_template::scalar_limited.
func renderScalar(value any, maxBytes int) (string, *core.Diagnostic) {
	rendered, ok := scalarText(value)
	if !ok {
		return "", typeDiag("filter expects a scalar value")
	}
	if len(rendered) > maxBytes {
		return "", limitDiag(fmt.Sprintf("rendered value exceeds %d bytes", maxBytes))
	}
	return rendered, nil
}

// scalarText returns the unquoted text for scalar values; ok is false for
// values the scalar filters reject (arrays, objects, unknown Go types).
func scalarText(value any) (string, bool) {
	switch typed := value.(type) {
	case string:
		return typed, true
	case bool:
		return strconv.FormatBool(typed), true
	case float64:
		return formatFloat(typed), true
	case int64:
		return strconv.FormatInt(typed, 10), true
	case int:
		return strconv.Itoa(typed), true
	case int32:
		return strconv.FormatInt(int64(typed), 10), true
	case nil:
		return "null", true
	default:
		return "", false
	}
}

// formatFloat renders numbers the way serde_json does: JSON integer literals
// (integral float64 values, as produced by encoding/json) render without a
// decimal point, everything else uses the shortest round-trip form with a
// bare exponent ("1e30", not "1e+30").
func formatFloat(f float64) string {
	if math.IsNaN(f) || math.IsInf(f, 0) {
		return strconv.FormatFloat(f, 'g', -1, 64)
	}
	if f == math.Trunc(f) && math.Abs(f) < 1e15 {
		return strconv.FormatInt(int64(f), 10)
	}
	text := strconv.FormatFloat(f, 'g', -1, 64)
	if i := strings.IndexAny(text, "eE"); i >= 0 {
		mantissa, exponent := text[:i], text[i+1:]
		negative := false
		if strings.HasPrefix(exponent, "+") || strings.HasPrefix(exponent, "-") {
			negative = exponent[0] == '-'
			exponent = exponent[1:]
		}
		exponent = strings.TrimLeft(exponent, "0")
		if exponent == "" {
			exponent = "0"
		}
		if negative {
			exponent = "-" + exponent
		}
		text = mantissa + "e" + exponent
	}
	return text
}

// base64Input validates a base64 filter input: a string (encoded as UTF-8)
// or an array of byte-sized numbers. It returns the byte count, the bytes,
// and whether the value was acceptable.
func base64Input(value any) (int, []byte, bool) {
	switch typed := value.(type) {
	case string:
		return len(typed), []byte(typed), true
	case []any:
		bytes := make([]byte, 0, len(typed))
		for _, element := range typed {
			b, ok := byteValue(element)
			if !ok {
				return 0, nil, false
			}
			bytes = append(bytes, b)
		}
		return len(bytes), bytes, true
	default:
		return 0, nil, false
	}
}

// byteValue accepts integer-valued numbers in 0..255. The Rust engine only
// accepts serde_json integer numbers; JSON decoding in Go yields float64, so
// integral float64 values are accepted to keep captured byte arrays working.
func byteValue(value any) (byte, bool) {
	switch typed := value.(type) {
	case float64:
		if typed == math.Trunc(typed) && typed >= 0 && typed <= 255 {
			return byte(typed), true
		}
	case int64:
		if typed >= 0 && typed <= 255 {
			return byte(typed), true
		}
	case int:
		if typed >= 0 && typed <= 255 {
			return byte(typed), true
		}
	}
	return 0, false
}

// encodeJSON serialises value as compact JSON with serde_json semantics:
// sorted object keys, no HTML escaping, \b/\f/\n/\r/\t and \u00xx control
// escapes, lowercase hex, raw UTF-8 otherwise.
func encodeJSON(value any) string {
	var out strings.Builder
	encodeJSONValue(&out, value)
	return out.String()
}

func encodeJSONValue(out *strings.Builder, value any) {
	switch typed := value.(type) {
	case nil:
		out.WriteString("null")
	case bool:
		out.WriteString(strconv.FormatBool(typed))
	case string:
		encodeJSONString(out, typed)
	case float64:
		out.WriteString(formatFloat(typed))
	case int64:
		out.WriteString(strconv.FormatInt(typed, 10))
	case int:
		out.WriteString(strconv.Itoa(typed))
	case int32:
		out.WriteString(strconv.FormatInt(int64(typed), 10))
	case []any:
		out.WriteByte('[')
		for i, element := range typed {
			if i > 0 {
				out.WriteByte(',')
			}
			encodeJSONValue(out, element)
		}
		out.WriteByte(']')
	case map[string]any:
		keys := make([]string, 0, len(typed))
		for key := range typed {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		out.WriteByte('{')
		for i, key := range keys {
			if i > 0 {
				out.WriteByte(',')
			}
			encodeJSONString(out, key)
			out.WriteByte(':')
			encodeJSONValue(out, typed[key])
		}
		out.WriteByte('}')
	default:
		encodeJSONString(out, fmt.Sprintf("%v", value))
	}
}

const upperHex = "0123456789ABCDEF"

func encodeJSONString(out *strings.Builder, s string) {
	out.WriteByte('"')
	for _, r := range s {
		switch r {
		case '"':
			out.WriteString("\\\"")
		case '\\':
			out.WriteString("\\\\")
		case '\n':
			out.WriteString("\\n")
		case '\r':
			out.WriteString("\\r")
		case '\t':
			out.WriteString("\\t")
		case '\b':
			out.WriteString("\\b")
		case '\f':
			out.WriteString("\\f")
		default:
			if r < 0x20 {
				out.WriteString("\\u00")
				out.WriteByte(upperHex[r>>4])
				out.WriteByte(upperHex[r&0xF])
			} else {
				out.WriteRune(r)
			}
		}
	}
	out.WriteByte('"')
}

// percentEncode applies the RFC 3986 component percent-encode set used by
// mdok-template: C0 controls, DEL, the reserved/sub-delims punctuation set,
// space, and every non-ASCII byte (percent_encoding always escapes
// non-ASCII). Unreserved characters (alnum, "-", ".", "_", "~") pass
// through, as do all other ASCII printable characters.
func percentEncode(s string) string {
	var out strings.Builder
	for i := 0; i < len(s); i++ {
		b := s[i]
		if needsPercentEncoding(b) {
			out.WriteByte('%')
			out.WriteByte(upperHex[b>>4])
			out.WriteByte(upperHex[b&0xF])
		} else {
			out.WriteByte(b)
		}
	}
	return out.String()
}

func needsPercentEncoding(b byte) bool {
	if b >= 0x80 || b <= 0x1F || b == 0x7F {
		return true
	}
	switch b {
	case ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '/',
		':', ';', '<', '=', '>', '?', '@', '[', '\\', ']', '^', '`', '{', '|', '}':
		return true
	}
	return false
}

func syntaxDiag(message string) *core.Diagnostic {
	return &core.Diagnostic{
		Severity: core.SeverityError,
		Code:     "MDOK-E400",
		Title:    "invalid template syntax",
		Message:  message,
	}
}

func missingDiag(name string) *core.Diagnostic {
	return &core.Diagnostic{
		Severity: core.SeverityError,
		Code:     "MDOK-E401",
		Title:    "missing variable",
		Message:  name,
	}
}

func typeDiag(message string) *core.Diagnostic {
	return &core.Diagnostic{
		Severity: core.SeverityError,
		Code:     "MDOK-E402",
		Title:    "template type/filter error",
		Message:  message,
	}
}

func limitDiag(message string) *core.Diagnostic {
	return &core.Diagnostic{
		Severity: core.SeverityError,
		Code:     "MDOK-E404",
		Title:    "template expansion exceeds resource limits",
		Message:  message,
	}
}
