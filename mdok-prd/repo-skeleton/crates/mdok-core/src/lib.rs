#![forbid(unsafe_code)]

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    pub path: PathBuf,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StepName(pub String);

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Option<SourceSpan>,
}
