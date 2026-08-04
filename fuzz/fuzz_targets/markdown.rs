#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

const MAX_INPUT_BYTES: usize = 128 * 1024;

fuzz_target!(|data: &[u8]| {
    let input = &data[..data.len().min(MAX_INPUT_BYTES)];
    let path = PathBuf::from("<fuzz>/markdown.md");

    let _ = mdok_markdown::parse_info_string(std::str::from_utf8(input).unwrap_or_default());
    if let Ok(document) = mdok_markdown::parse_bytes(input, path) {
        let _ = mdok_markdown::plan_document(&document);
    }
});
