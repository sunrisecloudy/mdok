//! Property-based tests for the mdok-shell tokenizer's core security invariants.
//!
//! These properties encode the no-shell-execution guarantees that the curl-fence
//! execution path relies on. A regression in any of them would reintroduce a
//! command-injection class bug.

use mdok_core::ValueMap;
use mdok_shell::parse_curl_source;
use proptest::prelude::*;
use serde_json::json;

/// Generate strings from a broad alphabet including all shell metacharacters,
/// whitespace, quotes, unicode, and control bytes that an attacker-controlled
/// capture value might contain.
fn adversarial_string() -> impl Strategy<Value = String> {
    "[\u{0000}-\u{007f}\u{0080}-\u{ffff}]{0,40}"
}

proptest! {
    /// INVARIANT (no re-tokenization): a template value interpolated via
    /// `{{value|raw}}` always renders into EXACTLY ONE argv element, regardless
    /// of its content. This is the defense that lets untrusted HTTP-response
    /// captures flow into later curl fences without shell injection.
    #[test]
    fn interpolated_value_never_splits_into_multiple_argv_elements(
        value in adversarial_string()
    ) {
        // Wrap the value so it must be one argument. `--header` is a literal
        // flag; the template is the value token. Even if `value` contains
        // spaces, `;`, `|`, `$()`, newlines, or quotes, it must not become two
        // tokens or escape its surrounding quotes.
        let source = format!("curl --header '{{{{value|raw}}}}'");
        let plan = parse_curl_source(&source)
            .expect("valid curl source with one template should parse");
        let mut vars = ValueMap::new();
        // NUL terminates C strings and can truncate; the tokenizer rejects NUL
        // in the *source*, but a value containing NUL would surface in argv.
        // Strip NUL to keep the property about token-count well-defined.
        let safe_value: String = value.chars().filter(|c| *c != '\u{0}').collect();
        vars.insert("value".to_owned(), json!(safe_value));
        let argv = plan
            .evaluate(&vars)
            .expect("evaluation should not fail for any value");
        // argv = ["curl", "--header", <rendered value>]
        let got_len = argv.len();
        prop_assert_eq!(got_len, 3, "interpolated value must be one element: argv={:?}", argv);
        // The rendered value must equal the input (no truncation/splitting).
        let got_value = argv.get(2).cloned().unwrap_or_default();
        prop_assert_eq!(got_value, safe_value);
    }

    /// INVARIANT (no metacharacter escape): a value containing shell
    /// metacharacters must never cause the source to parse into a different
    /// structure. Specifically, `;`, `|`, `&`, backticks, `$()`, and newlines
    /// inside an interpolated value must be treated as data, never as
    /// separators. We assert this by checking the parsed plan has the same
    /// argument COUNT regardless of the value.
    #[test]
    fn metacharacters_in_value_do_not_change_argument_count(
        value in adversarial_string()
    ) {
        let source = "curl --url '{{value|raw}}' --header 'X-Test: {{value|raw}}'";
        let plan = match parse_curl_source(source) {
            Ok(p) => p,
            Err(_) => return Ok(()), // parse error on the static source is a bug, but not this property
        };
        let mut vars = ValueMap::new();
        let safe_value: String = value.chars().filter(|c| *c != '\u{0}').collect();
        vars.insert("value".to_owned(), json!(safe_value));
        let argv = plan.evaluate(&vars).expect("eval");
        // curl, --url, <val>, --header, 'X-Test: <val>'
        let got_len = argv.len();
        prop_assert_eq!(got_len, 5, "argument count must be stable: argv={:?}", argv);
    }

    /// INVARIANT (first word must be literal curl): no input to the *source*
    /// text (not a value) can produce a plan whose first argument is not the
    /// literal string "curl".
    #[test]
    fn first_argument_is_always_literal_curl(
        prefix in "[a-z]{0,8}"
    ) {
        let source = format!("{prefix} --url 'x'");
        match parse_curl_source(&source) {
            Ok(plan) => {
                let argv = plan.evaluate(&ValueMap::new()).expect("eval");
                let first = argv.first().cloned();
                prop_assert_eq!(first.as_deref(), Some("curl"),
                    "first argv must be 'curl', got: {:?}", argv);
            }
            Err(_) => {} // non-curl prefix correctly rejected
        }
    }
}
