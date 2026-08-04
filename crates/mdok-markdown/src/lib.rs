#![forbid(unsafe_code)]

use comrak::{Arena, Options, nodes::NodeValue, parse_document as parse_comrak_document};
use mdok_core::{
    CapturePlan, CheckPlan, CurlSourcePlan, Diagnostic, DocumentPlan, SourceSpan, StepName,
    StepPlan,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
}

impl MarkdownError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidUtf8(_) => "MDOK-E001",
            Self::Metadata(_) => "MDOK-E100",
            Self::StepName(_) => "MDOK-E101",
            Self::Reference(_) => "MDOK-E102",
            Self::Variables(_) => "MDOK-E110",
        }
    }

    pub fn diagnostic(&self, span: Option<SourceSpan>) -> Diagnostic {
        Diagnostic::error(self.code(), "Markdown planning error", self.to_string())
            .with_optional_span(span)
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
    let arena = Arena::new();
    let root = parse_comrak_document(&arena, source, &Options::default());
    let headings = collect_headings(root, source);
    let mut blocks = Vec::new();
    for node in root.descendants() {
        let NodeValue::CodeBlock(code) = &node.data().value else {
            continue;
        };
        if !code.fenced || !code.info.split_whitespace().any(|word| word == "mdok") {
            continue;
        }
        let info = parse_info_string(&code.info).map_err(MarkdownError::Metadata)?;
        let sourcepos = node.data().sourcepos;
        let start = source_offset(source, sourcepos.start.line, sourcepos.start.column);
        let end = source_offset(source, sourcepos.end.line, sourcepos.end.column)
            .saturating_add(1)
            .min(source.len());
        let span = SourceSpan::new(
            path.clone(),
            start,
            end,
            sourcepos.start.line as u32,
            sourcepos.start.column as u32,
        );
        let heading_path = headings
            .iter()
            .filter(|(line, _)| *line < sourcepos.start.line)
            .fold(Vec::<(usize, String)>::new(), |mut stack, (_, heading)| {
                let (level, title) = heading.clone();
                while stack
                    .last()
                    .is_some_and(|(old_level, _)| *old_level >= level)
                {
                    stack.pop();
                }
                stack.push((level, title));
                stack
            })
            .into_iter()
            .map(|(_, title)| title)
            .collect();
        blocks.push(classify(
            info,
            code.literal.to_string(),
            span,
            heading_path,
        )?);
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
                if steps.contains_key(name) {
                    return Err(MarkdownError::StepName(name.to_string()));
                }
                let index = plan.steps.len();
                steps.insert(name.clone(), index);
                plan.steps.push(StepPlan {
                    name: name.clone(),
                    heading_path: heading_path.clone(),
                    curl: CurlSourcePlan {
                        source: source.clone(),
                        span: span.clone(),
                    },
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
                plan.steps[index]
                    .checks
                    .extend(source.lines().enumerate().filter_map(|(_, line)| {
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
    plan.validate()
        .map_err(|error| MarkdownError::StepName(error.to_string()))?;
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

fn source_offset(source: &str, line: usize, column: usize) -> usize {
    let mut current_line = 1;
    let mut offset: usize = 0;
    for part in source.split_inclusive('\n') {
        if current_line == line {
            return offset
                .saturating_add(column.saturating_sub(1))
                .min(source.len());
        }
        offset += part.len();
        current_line += 1;
    }
    offset.min(source.len())
}

fn collect_headings(root: comrak::Node<'_>, _source: &str) -> Vec<(usize, (usize, String))> {
    root.descendants()
        .filter_map(|node| {
            let level = match node.data().value {
                NodeValue::Heading(heading) => heading.level as usize,
                _ => return None,
            };
            let mut title = String::new();
            for child in node.descendants().skip(1) {
                match &child.data().value {
                    NodeValue::Text(text) => title.push_str(text.as_ref()),
                    NodeValue::Code(code) => title.push_str(&code.literal),
                    NodeValue::SoftBreak | NodeValue::LineBreak => title.push(' '),
                    _ => {}
                }
            }
            Some((
                node.data().sourcepos.start.line,
                (level, title.trim().to_owned()),
            ))
        })
        .collect()
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
    }

    #[test]
    fn metadata_rejects_duplicates_and_bad_quotes() {
        assert!(parse_info_string("curl mdok name=a name=b").is_err());
        assert!(parse_info_string("curl mdok name=\"a").is_err());
    }
}
