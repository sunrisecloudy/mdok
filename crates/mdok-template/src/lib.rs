#![forbid(unsafe_code)]

use base64::Engine;
use mdok_core::{SourceSpan, ValueMap};
use percent_encoding::{AsciiSet, CONTROLS};
use serde_json::Value;
use std::fmt;
use std::io::{self, Write};

pub const MAX_TEMPLATE_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_TEMPLATE_PARTS: usize = 4096;
pub const MAX_TEMPLATE_EXPANSION_DEPTH: usize = 32;
pub const MAX_TEMPLATE_RENDERED_BYTES: usize = 8 * 1024 * 1024;

const RFC3986_COMPONENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    String,
    Raw,
    Json,
    Url,
    Header,
    Base64,
}

impl Filter {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "string" => Self::String,
            "raw" => Self::Raw,
            "json" => Self::Json,
            "url" => Self::Url,
            "header" => Self::Header,
            "base64" => Self::Base64,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Raw => "raw",
            Self::Json => "json",
            Self::Url => "url",
            Self::Header => "header",
            Self::Base64 => "base64",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathPart {
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateExpression {
    pub path: Vec<PathPart>,
    pub filter: Filter,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplatePart {
    Literal(String),
    Expression(TemplateExpression),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub source: String,
    pub parts: Vec<TemplatePart>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TemplateError {
    #[error("MDOK-E400 invalid template syntax: {0}")]
    Syntax(String),
    #[error("MDOK-E401 missing variable: {0}")]
    MissingVariable(String),
    #[error("MDOK-E402 template type/filter error: {0}")]
    Type(String),
    #[error("MDOK-E403 unsafe header value")]
    UnsafeHeader,
    #[error("MDOK-E404 template expansion exceeds resource limits: {0}")]
    Limit(String),
}

impl TemplateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Syntax(_) => "MDOK-E400",
            Self::MissingVariable(_) => "MDOK-E401",
            Self::Type(_) => "MDOK-E402",
            Self::UnsafeHeader => "MDOK-E403",
            Self::Limit(_) => "MDOK-E404",
        }
    }
}

impl Template {
    pub fn parse(source: &str) -> Result<Self, TemplateError> {
        parse(source)
    }

    pub fn render(&self, values: &ValueMap) -> Result<String, TemplateError> {
        let mut output = String::with_capacity(self.source.len().min(MAX_TEMPLATE_RENDERED_BYTES));
        for part in &self.parts {
            match part {
                TemplatePart::Literal(value) => {
                    append_limited(&mut output, value, MAX_TEMPLATE_RENDERED_BYTES)?
                }
                TemplatePart::Expression(expression) => {
                    let remaining = MAX_TEMPLATE_RENDERED_BYTES.saturating_sub(output.len());
                    let rendered = render_expression_with_limit(expression, values, remaining)?;
                    output.push_str(&rendered);
                }
            }
        }
        Ok(output)
    }

    pub fn expressions(&self) -> impl Iterator<Item = &TemplateExpression> {
        self.parts.iter().filter_map(|part| match part {
            TemplatePart::Expression(value) => Some(value),
            TemplatePart::Literal(_) => None,
        })
    }
}

pub fn parse(source: &str) -> Result<Template, TemplateError> {
    if source.len() > MAX_TEMPLATE_SOURCE_BYTES {
        return Err(TemplateError::Limit(format!(
            "template source exceeds {} bytes",
            MAX_TEMPLATE_SOURCE_BYTES
        )));
    }
    let mut parts = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let relative_start = source[cursor..].find("{{");
        let relative_end = source[cursor..].find("}}");
        if relative_end.is_some_and(|end| relative_start.is_none_or(|start| end < start)) {
            return Err(TemplateError::Syntax("unmatched `}}`".into()));
        }
        let Some(relative_start) = relative_start else {
            push_part(
                &mut parts,
                TemplatePart::Literal(source[cursor..].to_owned()),
            )?;
            break;
        };
        let start = cursor + relative_start;
        if start > cursor {
            push_part(
                &mut parts,
                TemplatePart::Literal(source[cursor..start].to_owned()),
            )?;
        }
        let end = source[start + 2..]
            .find("}}")
            .ok_or_else(|| TemplateError::Syntax("unclosed `{{`".into()))?
            + start
            + 2;
        let inner = &source[start + 2..end];
        let (path, filter) = parse_expression(inner)?;
        push_part(
            &mut parts,
            TemplatePart::Expression(TemplateExpression {
                path,
                filter,
                span: None,
            }),
        )?;
        cursor = end + 2;
    }
    if source.is_empty() {
        push_part(&mut parts, TemplatePart::Literal(String::new()))?;
    }
    Ok(Template {
        source: source.to_owned(),
        parts,
    })
}

fn push_part(parts: &mut Vec<TemplatePart>, part: TemplatePart) -> Result<(), TemplateError> {
    if parts.len() >= MAX_TEMPLATE_PARTS {
        return Err(TemplateError::Limit(format!(
            "template has more than {} parts",
            MAX_TEMPLATE_PARTS
        )));
    }
    parts.push(part);
    Ok(())
}

pub fn parse_template(source: &str) -> Result<Template, TemplateError> {
    parse(source)
}

pub fn parse_expression(source: &str) -> Result<(Vec<PathPart>, Filter), TemplateError> {
    let mut input = source.trim();
    if input.is_empty() {
        return Err(TemplateError::Syntax("empty template".into()));
    }
    let mut pieces = input.split('|').map(str::trim);
    let path_text = pieces.next().unwrap();
    let filters: Vec<_> = pieces.collect();
    if filters.len() > 1 || filters.first().is_some_and(|value| value.is_empty()) {
        return Err(TemplateError::Syntax(
            "template must contain at most one filter".into(),
        ));
    }
    let filter = filters
        .first()
        .and_then(|value| Filter::parse(value))
        .unwrap_or(Filter::String);
    if !filters.is_empty() && Filter::parse(filters[0]).is_none() {
        return Err(TemplateError::Syntax(format!(
            "unknown filter `{}`",
            filters[0]
        )));
    }
    let mut chars = path_text.chars().peekable();
    let mut path = Vec::new();
    let first = parse_identifier(&mut chars)?;
    path.push(PathPart::Key(first));
    if path.len() > MAX_TEMPLATE_EXPANSION_DEPTH {
        return Err(TemplateError::Limit(format!(
            "template expansion depth exceeds {}",
            MAX_TEMPLATE_EXPANSION_DEPTH
        )));
    }
    loop {
        match chars.peek().copied() {
            Some('.') => {
                chars.next();
                path.push(PathPart::Key(parse_identifier(&mut chars)?));
            }
            Some('[') => {
                chars.next();
                let mut digits = String::new();
                while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                    digits.push(chars.next().unwrap());
                }
                if digits.is_empty() || chars.next() != Some(']') {
                    return Err(TemplateError::Syntax(
                        "array index must be a non-negative integer".into(),
                    ));
                }
                path.push(PathPart::Index(digits.parse().map_err(|_| {
                    TemplateError::Syntax("array index is too large".into())
                })?));
            }
            Some(_) => {
                return Err(TemplateError::Syntax(
                    "unexpected character in variable path".into(),
                ));
            }
            None => break,
        }
        if path.len() > MAX_TEMPLATE_EXPANSION_DEPTH {
            return Err(TemplateError::Limit(format!(
                "template expansion depth exceeds {}",
                MAX_TEMPLATE_EXPANSION_DEPTH
            )));
        }
    }
    input = input.trim();
    let _ = input;
    Ok((path, filter))
}

fn parse_identifier<I>(chars: &mut std::iter::Peekable<I>) -> Result<String, TemplateError>
where
    I: Iterator<Item = char>,
{
    let mut value = String::new();
    match chars.peek().copied() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => value.push(chars.next().unwrap()),
        _ => {
            return Err(TemplateError::Syntax(
                "path must start with an identifier".into(),
            ));
        }
    }
    while chars
        .peek()
        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
    {
        value.push(chars.next().unwrap());
    }
    Ok(value)
}

pub fn lookup<'a>(values: &'a ValueMap, path: &[PathPart]) -> Result<&'a Value, TemplateError> {
    let first = match path.first() {
        Some(PathPart::Key(key)) => key,
        _ => return Err(TemplateError::Syntax("path must start with a key".into())),
    };
    let mut value = values
        .get(first)
        .ok_or_else(|| TemplateError::MissingVariable(first.clone()))?;
    for part in &path[1..] {
        value = match part {
            PathPart::Key(key) => value
                .get(key)
                .ok_or_else(|| TemplateError::MissingVariable(key.clone()))?,
            PathPart::Index(index) => value
                .get(*index)
                .ok_or_else(|| TemplateError::MissingVariable(format!("[{index}]")))?,
        };
    }
    Ok(value)
}

pub fn render_expression(
    expression: &TemplateExpression,
    values: &ValueMap,
) -> Result<String, TemplateError> {
    render_expression_with_limit(expression, values, MAX_TEMPLATE_RENDERED_BYTES)
}

pub fn render_expression_with_limit(
    expression: &TemplateExpression,
    values: &ValueMap,
    max_bytes: usize,
) -> Result<String, TemplateError> {
    let value = lookup(values, &expression.path)?;
    match expression.filter {
        Filter::Json => {
            let mut writer = LimitedWriter::new(max_bytes);
            if let Err(error) = serde_json::to_writer(&mut writer, value) {
                return if writer.exceeded {
                    Err(TemplateError::Limit(format!(
                        "rendered value exceeds {max_bytes} bytes"
                    )))
                } else {
                    Err(TemplateError::Type(error.to_string()))
                };
            }
            String::from_utf8(writer.bytes).map_err(|error| TemplateError::Type(error.to_string()))
        }
        Filter::Base64 => {
            let byte_len = match value {
                Value::String(value) => value.len(),
                Value::Array(values)
                    if values
                        .iter()
                        .all(|value| value.as_u64().is_some_and(|n| n <= 255)) =>
                {
                    values.len()
                }
                _ => {
                    return Err(TemplateError::Type(
                        "base64 expects a string or byte array".into(),
                    ));
                }
            };
            let encoded_len = byte_len
                .checked_add(2)
                .and_then(|length| length.checked_div(3))
                .and_then(|groups| groups.checked_mul(4))
                .ok_or_else(|| TemplateError::Limit("base64 output size overflowed".into()))?;
            if encoded_len > max_bytes {
                return Err(TemplateError::Limit(format!(
                    "rendered value exceeds {max_bytes} bytes"
                )));
            }
            match value {
                Value::String(value) => {
                    Ok(base64::engine::general_purpose::STANDARD.encode(value.as_bytes()))
                }
                Value::Array(values) => {
                    let bytes = values
                        .iter()
                        .map(|value| value.as_u64().expect("validated byte array") as u8)
                        .collect::<Vec<_>>();
                    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
                }
                _ => unreachable!("base64 value was validated above"),
            }
        }
        Filter::Url => {
            let value = scalar_limited(value, max_bytes)?;
            if value
                .len()
                .checked_mul(3)
                .is_none_or(|length| length > max_bytes)
            {
                return Err(TemplateError::Limit(format!(
                    "rendered URL value exceeds {max_bytes} bytes"
                )));
            }
            Ok(percent_encoding::percent_encode(value.as_bytes(), RFC3986_COMPONENT).to_string())
        }
        Filter::Header => {
            let value = scalar_limited(value, max_bytes)?;
            if value.contains(['\r', '\n']) {
                Err(TemplateError::UnsafeHeader)
            } else {
                Ok(value.to_owned())
            }
        }
        Filter::String | Filter::Raw => scalar_limited(value, max_bytes),
    }
}

fn scalar_limited(value: &Value, max_bytes: usize) -> Result<String, TemplateError> {
    let rendered = match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => (if *value { "true" } else { "false" }).to_owned(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Array(_) | Value::Object(_) => {
            return Err(TemplateError::Type("filter expects a scalar value".into()));
        }
    };
    if rendered.len() > max_bytes {
        return Err(TemplateError::Limit(format!(
            "rendered value exceeds {max_bytes} bytes"
        )));
    }
    Ok(rendered)
}

fn append_limited(output: &mut String, value: &str, max_bytes: usize) -> Result<(), TemplateError> {
    let total = output
        .len()
        .checked_add(value.len())
        .ok_or_else(|| TemplateError::Limit("rendered template size overflowed".into()))?;
    if total > max_bytes {
        return Err(TemplateError::Limit(format!(
            "rendered template exceeds {max_bytes} bytes"
        )));
    }
    output.push_str(value);
    Ok(())
}

struct LimitedWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
    exceeded: bool,
}

impl LimitedWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(max_bytes.min(4096)),
            max_bytes,
            exceeded: false,
        }
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining = self.max_bytes.saturating_sub(self.bytes.len());
        if bytes.len() > remaining {
            self.exceeded = true;
            if remaining > 0 {
                self.bytes.extend_from_slice(&bytes[..remaining]);
            }
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "template output limit exceeded",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn render(source: &str, values: &ValueMap) -> Result<String, TemplateError> {
    parse(source)?.render(values)
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn values() -> ValueMap {
        [("user".into(), json!({"name": "A/B", "tags": ["x y"]}))]
            .into_iter()
            .collect()
    }

    #[test]
    fn parses_nested_paths_and_renders_inside_literal_text() {
        assert_eq!(
            render("/{{user.name}}/{{user.tags[0]|url}}", &values()).unwrap(),
            "/A/B/x%20y"
        );
    }

    #[test]
    fn filters_are_typed_and_header_is_safe() {
        assert_eq!(render("{{user.name|base64}}", &values()).unwrap(), "QS9C");
        let values = [("value".into(), json!("ok\nInjected: yes"))]
            .into_iter()
            .collect();
        assert_eq!(
            render("{{value|header}}", &values).unwrap_err().code(),
            "MDOK-E403"
        );
    }

    #[test]
    fn rejects_bad_templates() {
        assert_eq!(parse("{{missing").unwrap_err().code(), "MDOK-E400");
        assert_eq!(parse("{{value|wat}}").unwrap_err().code(), "MDOK-E400");
    }

    #[test]
    fn bounds_expansion_depth_and_rendered_bytes() {
        let deep_path = format!(
            "{{{{root{}}}}}",
            ".child".repeat(MAX_TEMPLATE_EXPANSION_DEPTH)
        );
        assert_eq!(parse(&deep_path).unwrap_err().code(), "MDOK-E404");

        let values = [(
            "value".into(),
            json!("x".repeat(MAX_TEMPLATE_RENDERED_BYTES + 1)),
        )]
        .into_iter()
        .collect();
        assert_eq!(
            render("{{value}}", &values).unwrap_err().code(),
            "MDOK-E404"
        );
    }
}
