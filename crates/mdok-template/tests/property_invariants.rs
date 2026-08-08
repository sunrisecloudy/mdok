//! Property-based tests for the mdok-template engine's security invariants.

use mdok_core::ValueMap;
use mdok_template::Template;
use proptest::prelude::*;
use serde_json::json;

fn adversarial_string() -> impl Strategy<Value = String> {
    "[\u{0000}-\u{007f}\u{0080}-\u{ffff}]{0,40}"
}

proptest! {
    /// INVARIANT (no CRLF in header filter output): the `header` filter is used
    /// to render values into HTTP request headers. Its output must NEVER contain
    /// a CR (`\r`) or LF (`\n`), regardless of input — otherwise an
    /// attacker-controlled capture could inject additional headers (request
    /// splitting/smuggling).
    #[test]
    fn header_filter_output_never_contains_crlf(value in adversarial_string()) {
        let template = Template::parse("{{value|header}}").expect("header template parses");
        let mut vars = ValueMap::new();
        let safe_value: String = value.chars().filter(|c| *c != '\u{0}').collect();
        vars.insert("value".to_owned(), json!(safe_value));
        match template.render(&vars) {
            Ok(rendered) => {
                prop_assert!(
                    !rendered.contains('\r') && !rendered.contains('\n'),
                    "header filter output must not contain CR/LF: rendered={rendered:?}"
                );
            }
            // A rejection (UnsafeHeader) is an acceptable outcome — the filter
            // may reject rather than emit. The invariant is that it never
            // *emits* CR/LF.
            Err(_) => {}
        }
    }

    /// INVARIANT (url filter output is safe for a URL segment): the `url`
    /// filter percent-encodes, so its output must not contain raw characters
    /// that could change the URL's host/path structure. At minimum, no raw `\n`
    /// (which would break HTTP request framing if placed in a URL).
    #[test]
    fn url_filter_output_never_contains_newline(value in adversarial_string()) {
        let template = Template::parse("{{value|url}}").expect("url template parses");
        let mut vars = ValueMap::new();
        let safe_value: String = value.chars().filter(|c| *c != '\u{0}').collect();
        vars.insert("value".to_owned(), json!(safe_value));
        match template.render(&vars) {
            Ok(rendered) => {
                prop_assert!(
                    !rendered.contains('\n') && !rendered.contains('\r'),
                    "url filter output must not contain CR/LF: rendered={rendered:?}"
                );
            }
            Err(_) => {}
        }
    }
}
