// Package shell implements mdok's restricted shell word tokenizer for curl
// fences, ported from crates/mdok-shell. It accepts exactly one curl simple
// command: no pipes, redirections, substitutions, semicolons, assignments, or
// expansions; quoting and backslash-newline continuations follow POSIX word
// splitting. "{{...}}" regions are kept as literal word content and validated
// for template syntax; expansion happens later, per rendered argument.
package shell

import (
	"errors"
	"fmt"
	"strings"
	"unicode/utf8"

	"mdok/core"
	"mdok/template"
)

// Resource limits applied while tokenizing curl command sources.
const (
	MaxArgvArguments = 64
	MaxArgBytes      = 64 * 1024
	MaxArgvBytes     = 1024 * 1024
)

// Error is a shell tokenizing failure carrying its MDOK-Exxx code. Its
// Error() text mirrors the Rust ShellError display: "{code}: {message}".
type Error struct {
	Code    string
	Title   string
	Message string
}

func (e *Error) Error() string {
	return e.Code + ": " + e.Message
}

// Diagnostic converts err into the shared core.Diagnostic shape, if err is
// a shell Error.
func Diagnostic(err error) (core.Diagnostic, bool) {
	var shellErr *Error
	if errors.As(err, &shellErr) {
		return core.Diagnostic{
			Severity: core.SeverityError,
			Code:     shellErr.Code,
			Title:    shellErr.Title,
			Message:  shellErr.Message,
		}, true
	}
	return core.Diagnostic{}, false
}

// IsCurl reports whether argv is a curl simple command: at least one word
// whose first word is the literal "curl". The tokenizer rejects every
// construct that could chain a second command, so a successfully parsed
// argv is always exactly one simple command (MDOK-E202 otherwise).
func IsCurl(argv []string) bool {
	return len(argv) > 0 && argv[0] == "curl"
}

// ParseCurlSource tokenizes a curl fence body into argument words. Fence
// bodies end with a newline, so — exactly like the Rust pipeline, which
// calls mdok_shell::parse(source.trim()) — the input is trimmed first.
// Errors are *core.Diagnostic with the mdok-shell codes: MDOK-E200 word
// syntax (trailing backslash, unterminated quote), MDOK-E201 forbidden
// shell construct, MDOK-E202 not exactly one curl simple command,
// MDOK-E400/E404 invalid template text, and MDOK-E405 argument limits.
func ParseCurlSource(source string) ([]string, error) {
	source = strings.TrimSpace(source)
	if len(source) > template.MaxSourceBytes {
		return nil, diag("MDOK-E405", "curl command limit exceeded",
			fmt.Sprintf("curl command source exceeds %d bytes", template.MaxSourceBytes))
	}

	const (
		unquoted = 0
		single   = '\''
		double   = '"'
	)

	var words []string
	var current strings.Builder
	wordActive := false // mirrors a non-empty Rust segment list (an empty quoted word counts)
	quote := unquoted
	index := 0
	bytes := len(source)

	finishWord := func() {
		if !wordActive {
			return
		}
		words = append(words, current.String())
		current.Reset()
		wordActive = false
	}
	writeRune := func(at int) {
		r, size := utf8.DecodeRuneInString(source[at:])
		current.WriteString(string(r))
		index = at + size
		wordActive = true
	}

	for index < bytes {
		// Templates are recognized in every quote state and stay literal here.
		if strings.HasPrefix(source[index:], "{{") {
			relative := strings.Index(source[index+2:], "}}")
			if relative < 0 {
				return nil, diag("MDOK-E400", "invalid template syntax", "unclosed template")
			}
			end := index + 2 + relative
			templateText := source[index : end+2]
			if _, templateDiag := template.Parse(templateText); templateDiag != nil {
				return nil, wrapTemplateDiag(templateDiag)
			}
			current.WriteString(templateText)
			wordActive = true
			index = end + 2
			continue
		}

		b := source[index]
		switch quote {
		case unquoted:
			switch b {
			case ' ', '\t', '\r':
				finishWord()
				if len(words) > MaxArgvArguments {
					return nil, diag("MDOK-E405", "curl command limit exceeded",
						fmt.Sprintf("curl command has more than %d arguments", MaxArgvArguments))
				}
				index++
			case '\n':
				return nil, diag("MDOK-E201", "forbidden shell construct",
					"unescaped newline would terminate the curl command")
			case '#':
				if wordActive {
					writeRune(index)
					continue
				}
				for index < bytes && source[index] != '\n' {
					index++
				}
				if index < bytes {
					return nil, diag("MDOK-E201", "forbidden shell construct",
						"a comment cannot be followed by another command")
				}
			case '\'':
				wordActive = true
				quote = single
				index++
			case '"':
				wordActive = true
				quote = double
				index++
			case '\\':
				index++
				if index >= bytes {
					return nil, diag("MDOK-E200", "curl source syntax error", "trailing backslash")
				}
				if source[index] == '\n' {
					index++ // line continuation joins the next line
				} else {
					writeRune(index)
				}
			case ';', '|', '&', '<', '>', '(', ')', '`', '$', '{', '}', '*', '?':
				return nil, diag("MDOK-E201", "forbidden shell construct", "forbidden shell construct")
			default:
				if b == '=' && !wordActive {
					return nil, diag("MDOK-E201", "forbidden shell construct",
						"shell assignments are not allowed")
				}
				writeRune(index)
			}
		case single:
			if b == '\'' {
				quote = unquoted
				index++
			} else {
				writeRune(index)
			}
		case double:
			switch b {
			case '"':
				quote = unquoted
				index++
			case '\\':
				index++
				if index >= bytes {
					return nil, diag("MDOK-E200", "curl source syntax error",
						"trailing backslash in quoted word")
				}
				switch source[index] {
				case '"', '\\', '$', '`':
					writeRune(index)
				case '\n':
					index++ // line continuation inside double quotes
				default:
					// Unknown escapes keep the backslash; the escaped
					// character is processed by the next iteration.
					current.WriteByte('\\')
					wordActive = true
				}
			case '$', '`':
				return nil, diag("MDOK-E201", "forbidden shell construct",
					"shell expansion is not allowed")
			default:
				writeRune(index)
			}
		}
	}

	if quote != unquoted {
		return nil, diag("MDOK-E200", "curl source syntax error", "unterminated shell quote")
	}
	finishWord()
	if len(words) > MaxArgvArguments {
		return nil, diag("MDOK-E405", "curl command limit exceeded",
			fmt.Sprintf("curl command has more than %d arguments", MaxArgvArguments))
	}
	if len(words) == 0 {
		return nil, diag("MDOK-E202", "not a curl command", "curl fence is empty")
	}
	if words[0] != "curl" {
		return nil, diag("MDOK-E202", "not a curl command",
			"first word must be the literal `curl`")
	}
	return words, nil
}

// wrapTemplateDiag re-reports a template diagnostic with the full
// "MDOK-Exxx title: message" text the Rust ShellError carries.
func wrapTemplateDiag(diagnostic *core.Diagnostic) error {
	return diag(diagnostic.Code, diagnostic.Title,
		fmt.Sprintf("%s %s: %s", diagnostic.Code, diagnostic.Title, diagnostic.Message))
}

func diag(code, title, message string) error {
	return &Error{Code: code, Title: title, Message: message}
}
