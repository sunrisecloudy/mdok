#![forbid(unsafe_code)]

use comrak::{Arena, Options, nodes::NodeValue, parse_document as parse_comrak_document};
use mdok_core::{
    CapturePlan, CheckPlan, CurlSourcePlan, Diagnostic, DocumentPlan, ExecSourcePlan, SourceSpan,
    StepName, StepPlan, StepSource,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Hard limits applied before and during Markdown planning. These are shared
/// with the CLI's compatibility preflight so the two parser paths cannot
/// silently disagree about resource use.
pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SOURCE_LINES: usize = 100_000;
pub const MAX_AST_NODES: usize = 100_000;
pub const MAX_FENCES: usize = 1_024;
pub const MAX_EXECUTABLE_BLOCKS: usize = 256;
pub const MAX_FENCE_BODY_BYTES: usize = 512 * 1024;
pub const MAX_STEPS: usize = 128;
pub const MAX_CHECKS_PER_STEP: usize = 512;
pub const MAX_CAPTURES_PER_STEP: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FenceInfo {
    pub language: String,
    pub attributes: BTreeMap<String, String>,
    pub flags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutableBlock {
    Variables {
        info: FenceInfo,
        source: String,
        span: SourceSpan,
    },
    Request {
        info: FenceInfo,
        name: StepName,
        source: String,
        heading_path: Vec<String>,
        span: SourceSpan,
    },
    Exec {
        info: FenceInfo,
        name: StepName,
        source: String,
        heading_path: Vec<String>,
        span: SourceSpan,
    },
    Check {
        info: FenceInfo,
        step: StepName,
        source: String,
        span: SourceSpan,
    },
    Capture {
        info: FenceInfo,
        step: StepName,
        source: String,
        span: SourceSpan,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownDocument {
    pub path: PathBuf,
    pub blocks: Vec<ExecutableBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MarkdownError {
    #[error("MDOK-E001 invalid UTF-8 or document input: {0}")]
    InvalidUtf8(String),
    #[error("MDOK-E100 invalid executable fence metadata: {0}")]
    Metadata(String),
    #[error("MDOK-E101 invalid or duplicate step name: {0}")]
    StepName(String),
    #[error("MDOK-E102 unknown step reference or invalid order: {0}")]
    Reference(String),
    #[error("MDOK-E110 invalid TOML variables block: {0}")]
    Variables(String),
    #[error("MDOK-E700 Markdown resource limit exceeded: {0}")]
    ResourceLimit(String),
    #[error("{0}")]
    Core(mdok_core::CoreError),
}

impl MarkdownError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidUtf8(_) => "MDOK-E001",
            Self::Metadata(_) => "MDOK-E100",
            Self::StepName(_) => "MDOK-E101",
            Self::Reference(_) => "MDOK-E102",
            Self::Variables(_) => "MDOK-E110",
            Self::ResourceLimit(_) => "MDOK-E700",
            Self::Core(error) => error.code(),
        }
    }

    pub fn diagnostic(&self, span: Option<SourceSpan>) -> Diagnostic {
        let title = if matches!(self, Self::ResourceLimit(_)) {
            "Markdown resource limit exceeded"
        } else {
            "Markdown planning error"
        };
        Diagnostic::error(self.code(), title, self.to_string()).with_optional_span(span)
    }
}

trait OptionalSpan {
    fn with_optional_span(self, span: Option<SourceSpan>) -> Self;
}
impl OptionalSpan for Diagnostic {
    fn with_optional_span(mut self, span: Option<SourceSpan>) -> Self {
        self.span = span;
        self
    }
}

pub fn parse_bytes(
    bytes: &[u8],
    path: impl Into<PathBuf>,
) -> Result<MarkdownDocument, MarkdownError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(MarkdownError::ResourceLimit(format!(
            "source is {} bytes; the maximum is {} bytes",
            bytes.len(),
            MAX_SOURCE_BYTES
        )));
    }
    let source = std::str::from_utf8(bytes)
        .map_err(|error| MarkdownError::InvalidUtf8(error.to_string()))?;
    parse_document(source, path)
}

pub fn parse(source: &str, path: impl Into<PathBuf>) -> Result<MarkdownDocument, MarkdownError> {
    parse_document(source, path)
}

pub fn parse_document(
    source: &str,
    path: impl Into<PathBuf>,
) -> Result<MarkdownDocument, MarkdownError> {
    let path = path.into();
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if source.len() > MAX_SOURCE_BYTES {
        return Err(MarkdownError::ResourceLimit(format!(
            "source is {} bytes; the maximum is {} bytes",
            source.len(),
            MAX_SOURCE_BYTES
        )));
    }
    let source_lines = source.bytes().filter(|byte| *byte == b'\n').count() + 1;
    if source_lines > MAX_SOURCE_LINES {
        return Err(MarkdownError::ResourceLimit(format!(
            "source has {source_lines} lines; the maximum is {MAX_SOURCE_LINES}"
        )));
    }
    let arena = Arena::new();
    let root = parse_comrak_document(&arena, source, &Options::default());
    let line_offsets = source_line_offsets(source);
    let mut blocks = Vec::new();
    let mut heading_stack = Vec::<(usize, String)>::new();
    let mut ast_nodes = 0;
    let mut fences = 0;
    let mut executable_blocks = 0;
    for node in root.descendants() {
        ast_nodes += 1;
        if ast_nodes > MAX_AST_NODES {
            return Err(MarkdownError::ResourceLimit(format!(
                "AST contains more than {MAX_AST_NODES} nodes"
            )));
        }
        match &node.data().value {
            NodeValue::Heading(heading) => {
                let mut title = String::new();
                for child in node.descendants().skip(1) {
                    match &child.data().value {
                        NodeValue::Text(text) => title.push_str(text.as_ref()),
                        NodeValue::Code(code) => title.push_str(&code.literal),
                        NodeValue::SoftBreak | NodeValue::LineBreak => title.push(' '),
                        _ => {}
                    }
                }
                while heading_stack
                    .last()
                    .is_some_and(|(old_level, _)| *old_level >= heading.level as usize)
                {
                    heading_stack.pop();
                }
                heading_stack.push((heading.level as usize, title.trim().to_owned()));
            }
            NodeValue::CodeBlock(code) if code.fenced => {
                fences += 1;
                if fences > MAX_FENCES {
                    return Err(MarkdownError::ResourceLimit(format!(
                        "document contains more than {MAX_FENCES} fenced code blocks"
                    )));
                }
                if code.literal.len() > MAX_FENCE_BODY_BYTES {
                    return Err(MarkdownError::ResourceLimit(format!(
                        "fenced code block is {} bytes; the maximum is {} bytes",
                        code.literal.len(),
                        MAX_FENCE_BODY_BYTES
                    )));
                }
                if !code.info.split_whitespace().any(|word| word == "mdok") {
                    continue;
                }
                executable_blocks += 1;
                if executable_blocks > MAX_EXECUTABLE_BLOCKS {
                    return Err(MarkdownError::ResourceLimit(format!(
                        "document contains more than {MAX_EXECUTABLE_BLOCKS} executable blocks"
                    )));
                }
                let info = parse_info_string(&code.info).map_err(MarkdownError::Metadata)?;
                let sourcepos = node.data().sourcepos;
                let start = source_offset(
                    &line_offsets,
                    source,
                    sourcepos.start.line,
                    sourcepos.start.column,
                );
                let end = source_offset(
                    &line_offsets,
                    source,
                    sourcepos.end.line,
                    sourcepos.end.column,
                )
                .saturating_add(1)
                .min(source.len());
                let span = SourceSpan::new(
                    path.clone(),
                    start,
                    end,
                    sourcepos.start.line as u32,
                    sourcepos.start.column as u32,
                );
                let heading_path = heading_stack
                    .iter()
                    .map(|(_, title)| title.clone())
                    .collect();
                blocks.push(classify(
                    info,
                    code.literal.to_string(),
                    span,
                    heading_path,
                )?);
            }
            _ => {}
        }
    }
    Ok(MarkdownDocument { path, blocks })
}

pub fn parse_info_string(info: &str) -> Result<FenceInfo, String> {
    let mut cursor = 0;
    skip_spaces(info, &mut cursor);
    let language = read_identifier(info, &mut cursor)?;
    skip_spaces(info, &mut cursor);
    let marker = read_identifier(info, &mut cursor)?;
    if marker != "mdok" {
        return Err("executable fences must include the `mdok` marker".into());
    }
    let mut attributes = BTreeMap::new();
    let mut flags = Vec::new();
    while cursor < info.len() {
        skip_spaces(info, &mut cursor);
        if cursor == info.len() {
            break;
        }
        let key = read_identifier(info, &mut cursor)?;
        skip_spaces(info, &mut cursor);
        if info.as_bytes().get(cursor) == Some(&b'=') {
            cursor += 1;
            skip_spaces(info, &mut cursor);
            let value = read_value(info, &mut cursor)?;
            if attributes.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate attribute `{key}`"));
            }
        } else {
            if flags.iter().any(|flag| flag == &key) {
                return Err(format!("duplicate flag `{key}`"));
            }
            flags.push(key);
        }
    }
    Ok(FenceInfo {
        language,
        attributes,
        flags,
    })
}

pub fn plan_document(document: &MarkdownDocument) -> Result<DocumentPlan, MarkdownError> {
    if document.blocks.len() > MAX_EXECUTABLE_BLOCKS {
        return Err(MarkdownError::ResourceLimit(format!(
            "document contains more than {MAX_EXECUTABLE_BLOCKS} executable blocks"
        )));
    }
    let mut plan = DocumentPlan::new(document.path.clone());
    let mut steps: BTreeMap<StepName, usize> = BTreeMap::new();
    for block in &document.blocks {
        match block {
            ExecutableBlock::Variables { source, .. } => {
                let parsed: toml::Table = source
                    .parse::<toml::Table>()
                    .map_err(|error| MarkdownError::Variables(error.to_string()))?;
                let json = serde_json::to_value(parsed)
                    .map_err(|error| MarkdownError::Variables(error.to_string()))?;
                let Value::Object(object) = json else {
                    return Err(MarkdownError::Variables(
                        "variables root must be a table".into(),
                    ));
                };
                for (key, value) in object {
                    if plan.variables.insert(key.clone(), value).is_some() {
                        return Err(MarkdownError::Variables(format!(
                            "duplicate variable `{key}`"
                        )));
                    }
                }
            }
            ExecutableBlock::Request {
                name,
                source,
                heading_path,
                span,
                ..
            } => {
                if plan.steps.len() >= MAX_STEPS {
                    return Err(MarkdownError::ResourceLimit(format!(
                        "document contains more than {MAX_STEPS} steps"
                    )));
                }
                if steps.contains_key(name) {
                    return Err(MarkdownError::StepName(name.to_string()));
                }
                let index = plan.steps.len();
                steps.insert(name.clone(), index);
                plan.steps.push(StepPlan {
                    name: name.clone(),
                    heading_path: heading_path.clone(),
                    source: StepSource::Curl(CurlSourcePlan {
                        source: source.clone(),
                        span: span.clone(),
                    }),
                    checks: Vec::new(),
                    captures: Vec::new(),
                    span: span.clone(),
                });
            }
            ExecutableBlock::Exec {
                name,
                source,
                heading_path,
                span,
                ..
            } => {
                if plan.steps.len() >= MAX_STEPS {
                    return Err(MarkdownError::ResourceLimit(format!(
                        "document contains more than {MAX_STEPS} steps"
                    )));
                }
                if steps.contains_key(name) {
                    return Err(MarkdownError::StepName(name.to_string()));
                }
                let index = plan.steps.len();
                steps.insert(name.clone(), index);
                plan.steps.push(StepPlan {
                    name: name.clone(),
                    heading_path: heading_path.clone(),
                    source: StepSource::Exec(ExecSourcePlan {
                        source: source.clone(),
                        span: span.clone(),
                    }),
                    checks: Vec::new(),
                    captures: Vec::new(),
                    span: span.clone(),
                });
            }
            ExecutableBlock::Check {
                step, source, span, ..
            } => {
                let Some(index) = steps.get(step).copied() else {
                    return Err(MarkdownError::Reference(step.to_string()));
                };
                let additional_checks = source
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count();
                if plan.steps[index]
                    .checks
                    .len()
                    .checked_add(additional_checks)
                    .is_none_or(|count| count > MAX_CHECKS_PER_STEP)
                {
                    return Err(MarkdownError::ResourceLimit(format!(
                        "step `{step}` contains more than {MAX_CHECKS_PER_STEP} checks"
                    )));
                }
                plan.steps[index]
                    .checks
                    .extend(source.lines().filter_map(|line| {
                        let expression = line.trim();
                        (!expression.is_empty()).then(|| CheckPlan {
                            expression: expression.to_owned(),
                            span: span.clone(),
                        })
                    }));
            }
            ExecutableBlock::Capture {
                step, source, span, ..
            } => {
                let Some(index) = steps.get(step).copied() else {
                    return Err(MarkdownError::Reference(step.to_string()));
                };
                if plan.steps[index].captures.len() >= MAX_CAPTURES_PER_STEP {
                    return Err(MarkdownError::ResourceLimit(format!(
                        "step `{step}` contains more than {MAX_CAPTURES_PER_STEP} captures"
                    )));
                }
                let expression = source.trim();
                if expression.is_empty() {
                    return Err(MarkdownError::Variables(
                        "capture expression is empty".into(),
                    ));
                }
                plan.steps[index].captures.push(CapturePlan {
                    expression: expression.to_owned(),
                    span: span.clone(),
                });
            }
        }
    }
    plan.validate().map_err(MarkdownError::Core)?;
    Ok(plan)
}

fn classify(
    info: FenceInfo,
    source: String,
    span: SourceSpan,
    headings: Vec<String>,
) -> Result<ExecutableBlock, MarkdownError> {
    let language = info.language.as_str();
    match (language, info.flags.as_slice()) {
        ("toml", [flag]) if flag == "vars" && info.attributes.is_empty() => {
            Ok(ExecutableBlock::Variables { info, source, span })
        }
        ("curl", []) => {
            let value = info
                .attributes
                .get("name")
                .ok_or_else(|| MarkdownError::Metadata("curl fence requires `name`".into()))?;
            if info.attributes.len() != 1 {
                return Err(MarkdownError::Metadata(
                    "unknown curl fence attribute".into(),
                ));
            }
            let name = StepName::new(value.clone())
                .map_err(|error| MarkdownError::StepName(error.to_string()))?;
            Ok(ExecutableBlock::Request {
                info,
                name,
                source,
                heading_path: headings,
                span,
            })
        }
        ("exec", []) => {
            let value = info
                .attributes
                .get("name")
                .ok_or_else(|| MarkdownError::Metadata("exec fence requires `name`".into()))?;
            if info.attributes.len() != 1 {
                return Err(MarkdownError::Metadata(
                    "unknown exec fence attribute".into(),
                ));
            }
            let name = StepName::new(value.clone())
                .map_err(|error| MarkdownError::StepName(error.to_string()))?;
            Ok(ExecutableBlock::Exec {
                info,
                name,
                source,
                heading_path: headings,
                span,
            })
        }
        ("jmespath", []) => {
            let (role, target) =
                match (info.attributes.get("check"), info.attributes.get("capture")) {
                    (Some(value), None) => ("check", value),
                    (None, Some(value)) => ("capture", value),
                    _ => {
                        return Err(MarkdownError::Metadata(
                            "jmespath fence requires exactly one of `check` or `capture`".into(),
                        ));
                    }
                };
            if info.attributes.len() != 1 {
                return Err(MarkdownError::Metadata(
                    "unknown jmespath fence attribute".into(),
                ));
            }
            let step = StepName::new(target.clone())
                .map_err(|error| MarkdownError::StepName(error.to_string()))?;
            if role == "check" {
                Ok(ExecutableBlock::Check {
                    info,
                    step,
                    source,
                    span,
                })
            } else {
                Ok(ExecutableBlock::Capture {
                    info,
                    step,
                    source,
                    span,
                })
            }
        }
        _ => Err(MarkdownError::Metadata(format!(
            "unsupported MDOK fence `{language}`"
        ))),
    }
}

fn source_line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(source.bytes().filter(|byte| *byte == b'\n').count() + 1);
    offsets.push(0);
    offsets.extend(
        source
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
    );
    offsets
}

fn source_offset(line_offsets: &[usize], source: &str, line: usize, column: usize) -> usize {
    line_offsets
        .get(line.saturating_sub(1))
        .copied()
        .unwrap_or(source.len())
        .saturating_add(column.saturating_sub(1))
        .min(source.len())
}

fn skip_spaces(input: &str, cursor: &mut usize) {
    while input
        .as_bytes()
        .get(*cursor)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        *cursor += 1;
    }
}

fn read_identifier(input: &str, cursor: &mut usize) -> Result<String, String> {
    let start = *cursor;
    if !input
        .as_bytes()
        .get(*cursor)
        .is_some_and(|byte| byte.is_ascii_alphabetic())
    {
        return Err("expected identifier".into());
    }
    *cursor += 1;
    while input
        .as_bytes()
        .get(*cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        *cursor += 1;
    }
    Ok(input[start..*cursor].to_owned())
}

fn read_value(input: &str, cursor: &mut usize) -> Result<String, String> {
    let Some(&byte) = input.as_bytes().get(*cursor) else {
        return Err("missing attribute value".into());
    };
    if matches!(byte, b'\'' | b'"') {
        let quote = byte;
        *cursor += 1;
        let mut value = String::new();
        while let Some(byte) = input.as_bytes().get(*cursor).copied() {
            *cursor += 1;
            if byte == quote {
                return Ok(value);
            }
            if quote == b'"' && byte == b'\\' {
                let escaped = input
                    .as_bytes()
                    .get(*cursor)
                    .copied()
                    .ok_or_else(|| "unterminated escape".to_owned())?;
                *cursor += 1;
                value.push(match escaped {
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    b'\\' => '\\',
                    b'"' => '"',
                    other => other as char,
                });
            } else {
                value.push(byte as char);
            }
        }
        Err("unterminated quoted value".into())
    } else {
        let start = *cursor;
        while input
            .as_bytes()
            .get(*cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            *cursor += 1;
        }
        if start == *cursor {
            return Err("missing attribute value".into());
        }
        Ok(input[start..*cursor].to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_marked_fences_and_tracks_headings() {
        let source = "# Users\n\n```text\ncurl ignored\n```\n\n```curl mdok name=get_user\ncurl https://example.test\n```\n";
        let document = parse_document(source, "test.md").unwrap();
        assert_eq!(document.blocks.len(), 1);
        let ExecutableBlock::Request {
            name, heading_path, ..
        } = &document.blocks[0]
        else {
            panic!()
        };
        assert_eq!(name.as_str(), "get_user");
        assert_eq!(heading_path, &["Users"]);
    }

    #[test]
    fn parses_variables_and_plan_associations() {
        let source = "```toml mdok vars\nbase = \"https://example.test\"\n```\n```curl mdok name=one\ncurl {{base}}\n```\n```jmespath mdok check=one\nstatus == `200`\n```\n";
        let plan = plan_document(&parse_document(source, "test.md").unwrap()).unwrap();
        assert_eq!(
            plan.variables["base"],
            Value::String("https://example.test".into())
        );
        assert_eq!(plan.steps[0].checks[0].expression, "status == `200`");
        assert!(matches!(&plan.steps[0].source, StepSource::Curl(_)));
    }

    #[test]
    fn classifies_exec_and_preserves_checks_and_captures_in_core_plan() {
        let source = "# Agent tools\n\n```exec mdok name=validate\nprintf '{\"ok\":true}'\n```\n\n```jmespath mdok check=validate\nsuccess == `true`\n```\n\n```jmespath mdok capture=validate\n{tool_ok: stdout_json.ok}\n```\n";
        let document = parse_document(source, "test.md").unwrap();
        assert_eq!(document.blocks.len(), 3);
        let ExecutableBlock::Exec {
            name,
            source,
            heading_path,
            ..
        } = &document.blocks[0]
        else {
            panic!("expected exec block");
        };
        assert_eq!(name.as_str(), "validate");
        assert_eq!(source, "printf '{\"ok\":true}'\n");
        assert_eq!(heading_path, &["Agent tools"]);

        let plan = plan_document(&document).unwrap();
        let StepSource::Exec(exec) = &plan.steps[0].source else {
            panic!("expected typed exec source");
        };
        assert_eq!(exec.source, "printf '{\"ok\":true}'\n");
        assert_eq!(plan.steps[0].checks.len(), 1);
        assert_eq!(plan.steps[0].checks[0].expression, "success == `true`");
        assert_eq!(plan.steps[0].captures.len(), 1);
        assert_eq!(
            plan.steps[0].captures[0].expression,
            "{tool_ok: stdout_json.ok}"
        );
    }

    #[test]
    fn keeps_curl_and_exec_as_distinct_block_and_plan_sources() {
        let source = "```curl mdok name=http\ncurl https://example.test\n```\n```exec mdok name=tool\nprintf ok\n```\n";
        let document = parse_document(source, "test.md").unwrap();
        assert!(matches!(
            &document.blocks[0],
            ExecutableBlock::Request { .. }
        ));
        assert!(matches!(&document.blocks[1], ExecutableBlock::Exec { .. }));

        let plan = plan_document(&document).unwrap();
        assert!(matches!(&plan.steps[0].source, StepSource::Curl(_)));
        assert!(matches!(&plan.steps[1].source, StepSource::Exec(_)));
    }

    #[test]
    fn heading_context_is_updated_without_affecting_later_blocks() {
        let source = "# API\n\n## Users\n\n```curl mdok name=list_users\ncurl https://example.test/users\n```\n\n### Details\n\n```curl mdok name=get_user\ncurl https://example.test/users/1\n```\n\n## Teams\n\n```curl mdok name=list_teams\ncurl https://example.test/teams\n```\n";
        let document = parse_document(source, "test.md").unwrap();
        let headings = document
            .blocks
            .iter()
            .map(|block| match block {
                ExecutableBlock::Request { heading_path, .. } => heading_path,
                _ => panic!("expected request block"),
            })
            .collect::<Vec<_>>();
        assert_eq!(headings[0], &["API", "Users"]);
        assert_eq!(headings[1], &["API", "Users", "Details"]);
        assert_eq!(headings[2], &["API", "Teams"]);
    }

    #[test]
    fn metadata_rejects_duplicates_and_bad_quotes() {
        assert!(parse_info_string("curl mdok name=a name=b").is_err());
        assert!(parse_info_string("curl mdok name=\"a").is_err());
    }

    #[test]
    fn rejects_oversized_source_before_parsing() {
        let source = "x".repeat(MAX_SOURCE_BYTES + 1);
        let error = parse(&source, "oversized.md").unwrap_err();
        assert!(
            matches!(error, MarkdownError::ResourceLimit(message) if message.contains("source is"))
        );
    }

    #[test]
    fn rejects_fence_and_step_budget_overflows() {
        let fenced = (0..=MAX_FENCES)
            .map(|_| "```text\nignored\n```\n")
            .collect::<String>();
        let error = parse(&fenced, "fences.md").unwrap_err();
        assert!(
            matches!(error, MarkdownError::ResourceLimit(message) if message.contains("fenced code blocks"))
        );

        let steps = (0..=MAX_STEPS)
            .map(|index| {
                format!("```curl mdok name=step-{index}\ncurl https://example.test\n```\n")
            })
            .collect::<String>();
        let document = parse(&steps, "steps.md").unwrap();
        let error = plan_document(&document).unwrap_err();
        assert!(
            matches!(error, MarkdownError::ResourceLimit(message) if message.contains("steps"))
        );
    }

    #[test]
    fn rejects_ast_budget_overflow() {
        let source = (0..(MAX_AST_NODES / 2 + 1))
            .map(|index| format!("# heading-{index}\n"))
            .collect::<String>();
        let error = parse(&source, "ast.md").unwrap_err();
        assert!(matches!(error, MarkdownError::ResourceLimit(message) if message.contains("AST")));
    }
}
