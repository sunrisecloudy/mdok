#![no_main]

use libfuzzer_sys::fuzz_target;
use mdok_core::ValueMap;
use serde_json::json;
use std::path::PathBuf;

const MAX_INPUT_BYTES: usize = 32 * 1024;

fuzz_target!(|data: &[u8]| {
    let input = &data[..data.len().min(MAX_INPUT_BYTES)];
    let text = String::from_utf8_lossy(input);
    let mut values = ValueMap::new();
    values.insert("value".to_owned(), json!(text.as_ref()));

    let _ = mdok_template::parse(&text);
    let _ = mdok_template::parse_expression(&text);
    let _ = mdok_template::render("{{value|raw}}", &values);

    let source = format!("curl 'https://example.test/{text}' --header 'X-Mdok: {{{{value|raw}}}}'");
    if let Ok(plan) = mdok_shell::parse_with_path(&source, PathBuf::from("<fuzz>/curl.md")) {
        let _ = plan.evaluate(&values);
        let _ = plan.templates().count();
    }

    let _ = mdok_shell::parse_with_path(&text, PathBuf::from("<fuzz>/raw.md"));
});
