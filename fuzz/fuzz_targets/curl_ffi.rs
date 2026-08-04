#![no_main]

use libfuzzer_sys::fuzz_target;
use mdok_curl_sys::Plan;
use std::sync::OnceLock;

const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_ARGUMENTS: usize = 32;
const MAX_ARGUMENT_BYTES: usize = 4096;

fn initialize() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| mdok_curl_sys::initialize().is_ok())
}

fuzz_target!(|data: &[u8]| {
    if !initialize() {
        return;
    }
    let input = &data[..data.len().min(MAX_INPUT_BYTES)];
    let mut arguments = vec![b"curl".to_vec(), b"https://example.test/".to_vec()];
    for chunk in input
        .split(|byte| *byte == 0)
        .take(MAX_ARGUMENTS - arguments.len())
    {
        arguments.push(chunk[..chunk.len().min(MAX_ARGUMENT_BYTES)].to_vec());
    }
    let references: Vec<&[u8]> = arguments.iter().map(Vec::as_slice).collect();
    let _ = Plan::parse(&references);
});
