use mdok_core::ValueMap;
use mdok_curl_sys::Plan;
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

const DEFAULT_RUNS: usize = 128;
const DEFAULT_MAX_LEN: usize = 4096;

fn setting(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn generated_inputs() -> Vec<Vec<u8>> {
    let runs = setting("MDOK_FUZZ_RUNS", DEFAULT_RUNS);
    let max_len = setting("MDOK_FUZZ_MAX_LEN", DEFAULT_MAX_LEN);
    let mut state = 0x4d44_4f4b_u64;
    let mut inputs = vec![
        Vec::new(),
        b"curl https://example.test/".to_vec(),
        b"curl 'https://example.test/{{value|raw}}' --header 'X-Test: \\x80'".to_vec(),
        b"```curl mdok name=seed\ncurl https://example.test/\n```\n".to_vec(),
    ];
    for _ in 0..runs {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let length = if max_len == 0 {
            0
        } else {
            (state as usize) % max_len.saturating_add(1)
        };
        let mut input = Vec::with_capacity(length);
        for _ in 0..length {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            input.push((state >> 24) as u8);
        }
        inputs.push(input);
    }
    inputs
}

#[test]
fn arbitrary_bytes_do_not_panic_parser_boundaries() {
    let inputs = generated_inputs();
    let initialized = mdok_curl_sys::initialize().is_ok();

    for input in inputs {
        let markdown = input.clone();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let path = PathBuf::from("<fuzz-smoke>/markdown.md");
                let _ = mdok_markdown::parse_info_string(
                    std::str::from_utf8(&markdown).unwrap_or_default(),
                );
                if let Ok(document) = mdok_markdown::parse_bytes(&markdown, path) {
                    let _ = mdok_markdown::plan_document(&document);
                }
            }))
            .is_ok(),
            "Markdown parser panicked for {} bytes",
            markdown.len()
        );

        let shell = input.clone();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let text = String::from_utf8_lossy(&shell);
                let mut values = ValueMap::new();
                values.insert("value".to_owned(), json!(text.as_ref()));
                let _ = mdok_template::parse(&text);
                let _ = mdok_template::parse_expression(&text);
                let _ = mdok_template::render("{{value|raw}}", &values);
                let path = PathBuf::from("<fuzz-smoke>/curl.md");
                if let Ok(plan) = mdok_shell::parse_with_path(&text, path) {
                    let _ = plan.evaluate(&values);
                    let _ = plan.templates().count();
                }
            }))
            .is_ok(),
            "shell/template parser panicked for {} bytes",
            shell.len()
        );

        if initialized {
            let curl = input;
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let mut arguments = vec![b"curl".to_vec(), b"https://example.test/".to_vec()];
                    for chunk in curl.split(|byte| *byte == 0).take(30) {
                        arguments.push(chunk[..chunk.len().min(4096)].to_vec());
                    }
                    let references: Vec<&[u8]> = arguments.iter().map(Vec::as_slice).collect();
                    let _ = Plan::parse(&references);
                }))
                .is_ok(),
                "native curl parser panicked for {} bytes",
                curl.len()
            );
        }
    }
}
