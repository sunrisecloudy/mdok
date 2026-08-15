// Package markdown extracts mdok executable fences from Markdown workflow
// documents, ported from crates/mdok-markdown. The parser is a hand-rolled
// CommonMark subset sufficient for mdok documents: ATX headings and fenced
// code blocks (``` or ~~~, at most three leading spaces, info strings).
// The Rust crate plans into DocumentPlan; this port folds planning into
// Parse and emits the Go core.Document contract instead.
package markdown

import (
	"errors"
	"fmt"
	"strings"
	"unicode/utf8"

	"github.com/BurntSushi/toml"

	"mdok/core"
)

// Hard limits applied before and during Markdown parsing, shared with the
// Rust crate so both parser paths agree about resource use.
const (
	MaxSourceBytes      = 8 * 1024 * 1024
	MaxSourceLines      = 100_000
	MaxASTNodes         = 100_000
	MaxFences           = 1_024
	MaxExecutableBlocks = 256
	MaxFenceBodyBytes   = 512 * 1024
	MaxSteps            = 128
	MaxChecksPerStep    = 512
	MaxCapturesPerStep  = 128
)

// maxStepNameBytes mirrors mdok_core::is_valid_step_name.
const maxStepNameBytes = 64

// Error is a Markdown planning failure carrying the structured
// core.Diagnostic (code, title, message). Error() returns the full
// "MDOK-Exxx category: detail" text the Rust MarkdownError displays.
type Error struct {
	Diagnostic core.Diagnostic
}

func (e *Error) Error() string {
	return e.Diagnostic.Message
}

// DiagnosticOf converts err into the shared core.Diagnostic shape, if err is
// a markdown Error.
func DiagnosticOf(err error) (core.Diagnostic, bool) {
	var markdownErr *Error
	if errors.As(err, &markdownErr) {
		return markdownErr.Diagnostic, true
	}
	return core.Diagnostic{}, false
}

// reservedNames mirrors mdok_core::RESERVED_NAMES.
var reservedNames = map[string]bool{
	"variables":   true,
	"steps":       true,
	"environment": true,
	"request":     true,
	"response":    true,
	"mdok":        true,
}

// Parse reads one Markdown workflow and returns its variables plus the
// ordered stream of executable items (*core.CurlItem, *core.CheckItem,
// *core.CaptureItem). Errors are *core.Diagnostic using the mdok-markdown
// codes: MDOK-E001 invalid UTF-8, MDOK-E100 fence metadata, MDOK-E101 step
// names, MDOK-E102 unknown step references, MDOK-E110 variables blocks, and
// MDOK-E700 resource limits.
func Parse(path string, source []byte) (*core.Document, error) {
	if len(source) > MaxSourceBytes {
		return nil, resourceLimit(fmt.Sprintf("source is %d bytes; the maximum is %d bytes",
			len(source), MaxSourceBytes))
	}
	if !utf8.Valid(source) {
		return nil, planningError("MDOK-E001", "invalid UTF-8 or document input", "source is not valid UTF-8")
	}
	text := strings.TrimPrefix(string(source), "\ufeff")
	if len(text) > MaxSourceBytes {
		return nil, resourceLimit(fmt.Sprintf("source is %d bytes; the maximum is %d bytes",
			len(text), MaxSourceBytes))
	}
	// CommonMark normalizes CRLF and CR line endings to LF.
	text = strings.ReplaceAll(text, "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	if strings.Count(text, "\n")+1 > MaxSourceLines {
		return nil, resourceLimit(fmt.Sprintf("source has %d lines; the maximum is %d lines",
			strings.Count(text, "\n")+1, MaxSourceLines))
	}

	document := &core.Document{Path: path, Vars: map[string]any{}}
	planner := newPlanner(document)

	lines := strings.Split(text, "\n")
	terminated := func(i int) bool { return i < len(lines)-1 }

	astNodes := 1 // the document root
	fenceCount := 0
	executableBlocks := 0
	var fence *openFence
	closeFence := func(info string, body string) error {
		fenceCount++
		if fenceCount > MaxFences {
			return resourceLimit(fmt.Sprintf("document contains more than %d fenced code blocks", MaxFences))
		}
		if len(body) > MaxFenceBodyBytes {
			return resourceLimit(fmt.Sprintf("fenced code block is %d bytes; the maximum is %d bytes",
				len(body), MaxFenceBodyBytes))
		}
		if !hasMdokMarker(info) {
			return nil
		}
		executableBlocks++
		if executableBlocks > MaxExecutableBlocks {
			return resourceLimit(fmt.Sprintf("document contains more than %d executable blocks", MaxExecutableBlocks))
		}
		infoParsed, err := parseInfoString(info)
		if err != nil {
			return planningError("MDOK-E100", "invalid executable fence metadata", err.Error())
		}
		return planner.classify(infoParsed, body, document)
	}

	for i, line := range lines {
		if fence != nil {
			if isClosingFence(line, fence.char, fence.length) {
				if err := closeFence(fence.info, fence.body.String()); err != nil {
					return nil, err
				}
				fence = nil
				continue
			}
			// Content lines lose up to the opening fence's indentation.
			fence.body.WriteString(dedent(line, fence.indent))
			if terminated(i) {
				fence.body.WriteByte('\n')
			}
			continue
		}
		if strings.TrimSpace(line) == "" {
			continue
		}
		if char, length, indent, info, ok := parseOpeningFence(line); ok {
			astNodes++ // one node for the code block itself
			if astNodes > MaxASTNodes {
				return nil, resourceLimit(fmt.Sprintf("AST contains more than %d nodes", MaxASTNodes))
			}
			fence = &openFence{char: char, length: length, indent: indent, info: info}
			continue
		}
		// ATX headings and paragraphs each approximate two AST nodes in
		// comrak (block node plus text child); headings are otherwise unused
		// because the Go Document contract does not carry a heading path.
		astNodes += 2
		if astNodes > MaxASTNodes {
			return nil, resourceLimit(fmt.Sprintf("AST contains more than %d nodes", MaxASTNodes))
		}
	}
	if fence != nil {
		if err := closeFence(fence.info, fence.body.String()); err != nil {
			return nil, err
		}
	}
	return document, nil
}

type openFence struct {
	char   byte
	length int
	indent int
	info   string
	body   strings.Builder
}

// planner tracks step definitions and per-step check/capture budgets while
// items are appended to the document in order.
type planner struct {
	document *core.Document
	steps    map[string]*stepBudget
	count    int
}

type stepBudget struct {
	checks   int
	captures int
}

func newPlanner(document *core.Document) *planner {
	return &planner{document: document, steps: map[string]*stepBudget{}}
}

// classify turns a parsed info string plus fence body into a document item,
// mirroring mdok_markdown::classify followed by the matching plan_document
// arm. The Go contract has no exec item type, so exec fences are rejected
// like any other unsupported language.
func (p *planner) classify(info *fenceInfo, body string, document *core.Document) error {
	switch info.language {
	case "toml":
		if len(info.flags) == 1 && info.flags[0] == "vars" && len(info.attributes) == 0 {
			return p.addVariables(body, document)
		}
	case "curl":
		if len(info.flags) == 0 {
			name, ok := info.attributes["name"]
			if !ok {
				return planningError("MDOK-E100", "invalid executable fence metadata", "curl fence requires `name`")
			}
			if len(info.attributes) != 1 {
				return planningError("MDOK-E100", "invalid executable fence metadata", "unknown curl fence attribute")
			}
			return p.addRequest(name, body)
		}
	case "jmespath":
		if len(info.flags) == 0 {
			check, hasCheck := info.attributes["check"]
			capture, hasCapture := info.attributes["capture"]
			if hasCheck == hasCapture {
				return planningError("MDOK-E100", "invalid executable fence metadata",
					"jmespath fence requires exactly one of `check` or `capture`")
			}
			if len(info.attributes) != 1 {
				return planningError("MDOK-E100", "invalid executable fence metadata", "unknown jmespath fence attribute")
			}
			if hasCheck {
				return p.addCheck(check, body)
			}
			return p.addCapture(capture, body)
		}
	}
	return planningError("MDOK-E100", "invalid executable fence metadata",
		fmt.Sprintf("unsupported MDOK fence `%s`", info.language))
}

func (p *planner) addVariables(body string, document *core.Document) error {
	var table map[string]any
	if _, err := toml.Decode(body, &table); err != nil {
		return planningError("MDOK-E110", "invalid TOML variables block", err.Error())
	}
	for key, value := range table {
		if _, exists := document.Vars[key]; exists {
			return planningError("MDOK-E110", "invalid TOML variables block",
				fmt.Sprintf("duplicate variable `%s`", key))
		}
		document.Vars[key] = value
	}
	return nil
}

func (p *planner) addRequest(name, body string) error {
	if !isValidStepName(name) {
		return planningError("MDOK-E101", "invalid or duplicate step name",
			fmt.Sprintf("invalid step name: %s", name))
	}
	if p.count >= MaxSteps {
		return resourceLimit(fmt.Sprintf("document contains more than %d steps", MaxSteps))
	}
	if _, duplicate := p.steps[name]; duplicate {
		return planningError("MDOK-E101", "invalid or duplicate step name", name)
	}
	p.steps[name] = &stepBudget{}
	p.count++
	p.document.Items = append(p.document.Items, &core.CurlItem{Name: name, Source: body})
	return nil
}

func (p *planner) addCheck(step, body string) error {
	if !isValidStepName(step) {
		return planningError("MDOK-E101", "invalid or duplicate step name",
			fmt.Sprintf("invalid step name: %s", step))
	}
	budget, ok := p.steps[step]
	if !ok {
		return planningError("MDOK-E102", "unknown step reference or invalid order", step)
	}
	var lines []string
	for _, line := range strings.Split(body, "\n") {
		if trimmed := strings.TrimSpace(line); trimmed != "" {
			lines = append(lines, trimmed)
		}
	}
	if budget.checks+len(lines) > MaxChecksPerStep {
		return resourceLimit(fmt.Sprintf("step `%s` contains more than %d checks", step, MaxChecksPerStep))
	}
	budget.checks += len(lines)
	p.document.Items = append(p.document.Items, &core.CheckItem{Step: step, Lines: lines})
	return nil
}

func (p *planner) addCapture(step, body string) error {
	if !isValidStepName(step) {
		return planningError("MDOK-E101", "invalid or duplicate step name",
			fmt.Sprintf("invalid step name: %s", step))
	}
	budget, ok := p.steps[step]
	if !ok {
		return planningError("MDOK-E102", "unknown step reference or invalid order", step)
	}
	if budget.captures >= MaxCapturesPerStep {
		return resourceLimit(fmt.Sprintf("step `%s` contains more than %d captures", step, MaxCapturesPerStep))
	}
	var parts []string
	for _, line := range strings.Split(body, "\n") {
		if trimmed := strings.TrimSpace(line); trimmed != "" {
			parts = append(parts, trimmed)
		}
	}
	expression := strings.Join(parts, "\n")
	if expression == "" {
		return planningError("MDOK-E110", "invalid TOML variables block", "capture expression is empty")
	}
	budget.captures++
	p.document.Items = append(p.document.Items, &core.CaptureItem{Step: step, Expr: expression})
	return nil
}

// parseOpeningFence recognizes an opening code fence: at most three leading
// spaces, then three or more backticks or tildes. Backtick fences may not
// carry a backtick in their info string (CommonMark).
func parseOpeningFence(line string) (char byte, length, indent int, info string, ok bool) {
	i := 0
	for i < len(line) && line[i] == ' ' {
		i++
	}
	if i > 3 || i >= len(line) {
		return 0, 0, 0, "", false
	}
	c := line[i]
	if c != '`' && c != '~' {
		return 0, 0, 0, "", false
	}
	j := i
	for j < len(line) && line[j] == c {
		j++
	}
	if j-i < 3 {
		return 0, 0, 0, "", false
	}
	rest := line[j:]
	if c == '`' && strings.ContainsRune(rest, '`') {
		return 0, 0, 0, "", false
	}
	return c, j - i, i, strings.TrimSpace(rest), true
}

// isClosingFence reports whether line closes a fence opened with char and
// length: at most three leading spaces, at least length fence characters,
// and nothing but spaces or tabs afterwards.
func isClosingFence(line string, char byte, length int) bool {
	i := 0
	for i < len(line) && line[i] == ' ' {
		i++
	}
	if i > 3 {
		return false
	}
	j := i
	for j < len(line) && line[j] == char {
		j++
	}
	if j-i < length {
		return false
	}
	for k := j; k < len(line); k++ {
		if line[k] != ' ' && line[k] != '\t' {
			return false
		}
	}
	return true
}

// dedent removes up to n leading spaces from a fence content line.
func dedent(line string, n int) string {
	i := 0
	for i < n && i < len(line) && line[i] == ' ' {
		i++
	}
	return line[i:]
}

// hasMdokMarker mirrors the comrak-level check: the mdok marker is any
// whitespace-separated info-string word equal to "mdok".
func hasMdokMarker(info string) bool {
	for _, word := range strings.Fields(info) {
		if word == "mdok" {
			return true
		}
	}
	return false
}

// isValidStepName mirrors mdok_core::is_valid_step_name: an identifier of at
// most 64 bytes ([A-Za-z][A-Za-z0-9_-]*) that is not reserved.
func isValidStepName(value string) bool {
	if value == "" || len(value) > maxStepNameBytes || reservedNames[value] {
		return false
	}
	first := value[0]
	if !((first >= 'a' && first <= 'z') || (first >= 'A' && first <= 'Z')) {
		return false
	}
	for i := 1; i < len(value); i++ {
		b := value[i]
		if !((b >= 'a' && b <= 'z') || (b >= 'A' && b <= 'Z') || (b >= '0' && b <= '9') || b == '_' || b == '-') {
			return false
		}
	}
	return true
}

// fenceInfo is the parsed executable fence metadata.
type fenceInfo struct {
	language   string
	attributes map[string]string
	flags      []string
}

// parseInfoString ports mdok_markdown::parse_info_string: an identifier
// language, the literal marker `mdok`, then attributes (key=value) and bare
// flags, with duplicates rejected.
func parseInfoString(info string) (*fenceInfo, error) {
	b := []byte(info)
	cursor := 0
	skipSpaces := func() {
		for cursor < len(b) && isASCIIWhitespace(b[cursor]) {
			cursor++
		}
	}
	skipSpaces()
	language, err := readIdentifier(b, &cursor)
	if err != nil {
		return nil, err
	}
	skipSpaces()
	marker, err := readIdentifier(b, &cursor)
	if err != nil {
		return nil, err
	}
	if marker != "mdok" {
		return nil, fmt.Errorf("executable fences must include the `mdok` marker")
	}
	parsed := &fenceInfo{language: language, attributes: map[string]string{}}
	for cursor < len(b) {
		skipSpaces()
		if cursor == len(b) {
			break
		}
		key, err := readIdentifier(b, &cursor)
		if err != nil {
			return nil, err
		}
		skipSpaces()
		if cursor < len(b) && b[cursor] == '=' {
			cursor++
			skipSpaces()
			value, err := readValue(b, &cursor)
			if err != nil {
				return nil, err
			}
			if _, duplicate := parsed.attributes[key]; duplicate {
				return nil, fmt.Errorf("duplicate attribute `%s`", key)
			}
			parsed.attributes[key] = value
			continue
		}
		for _, flag := range parsed.flags {
			if flag == key {
				return nil, fmt.Errorf("duplicate flag `%s`", key)
			}
		}
		parsed.flags = append(parsed.flags, key)
	}
	return parsed, nil
}

// isASCIIWhitespace mirrors Rust's u8::is_ascii_whitespace (no vertical tab).
func isASCIIWhitespace(b byte) bool {
	return b == ' ' || b == '\t' || b == '\n' || b == '\f' || b == '\r'
}

func readIdentifier(b []byte, cursor *int) (string, error) {
	start := *cursor
	if *cursor >= len(b) || !isASCIIAlphabetic(b[*cursor]) {
		return "", fmt.Errorf("expected identifier")
	}
	*cursor++
	for *cursor < len(b) && (isASCIIAlphanumeric(b[*cursor]) || b[*cursor] == '_' || b[*cursor] == '-') {
		*cursor++
	}
	return string(b[start:*cursor]), nil
}

// readValue ports mdok_markdown::read_value, including its double-quoted
// escape handling and Latin-1 byte-to-char fallback for unknown escapes.
func readValue(b []byte, cursor *int) (string, error) {
	if *cursor >= len(b) {
		return "", fmt.Errorf("missing attribute value")
	}
	if quote := b[*cursor]; quote == '\'' || quote == '"' {
		*cursor++
		var value strings.Builder
		for *cursor < len(b) {
			byte_ := b[*cursor]
			*cursor++
			if byte_ == quote {
				return value.String(), nil
			}
			if quote == '"' && byte_ == '\\' {
				if *cursor >= len(b) {
					return "", fmt.Errorf("unterminated escape")
				}
				escaped := b[*cursor]
				*cursor++
				switch escaped {
				case 'n':
					value.WriteRune('\n')
				case 'r':
					value.WriteRune('\r')
				case 't':
					value.WriteRune('\t')
				case '\\':
					value.WriteRune('\\')
				case '"':
					value.WriteRune('"')
				default:
					value.WriteRune(rune(escaped))
				}
				continue
			}
			value.WriteRune(rune(byte_))
		}
		return "", fmt.Errorf("unterminated quoted value")
	}
	start := *cursor
	for *cursor < len(b) && !isASCIIWhitespace(b[*cursor]) {
		*cursor++
	}
	if start == *cursor {
		return "", fmt.Errorf("missing attribute value")
	}
	return string(b[start:*cursor]), nil
}

func isASCIIAlphabetic(b byte) bool {
	return (b >= 'a' && b <= 'z') || (b >= 'A' && b <= 'Z')
}

func isASCIIAlphanumeric(b byte) bool {
	return isASCIIAlphabetic(b) || (b >= '0' && b <= '9')
}

// planningError mirrors MarkdownError::diagnostic for planning failures.
func planningError(code, category, detail string) error {
	return &Error{Diagnostic: core.Diagnostic{
		Severity: core.SeverityError,
		Code:     code,
		Title:    "Markdown planning error",
		Message:  fmt.Sprintf("%s %s: %s", code, category, detail),
	}}
}

// resourceLimit mirrors the resource-limit diagnostic (distinct title).
func resourceLimit(detail string) error {
	return &Error{Diagnostic: core.Diagnostic{
		Severity: core.SeverityError,
		Code:     "MDOK-E700",
		Title:    "Markdown resource limit exceeded",
		Message:  fmt.Sprintf("MDOK-E700 Markdown resource limit exceeded: %s", detail),
	}}
}
