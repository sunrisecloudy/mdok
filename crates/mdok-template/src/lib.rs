#![forbid(unsafe_code)]

use base64::Engine;
use mdok_core::{SourceSpan, ValueMap};
use percent_encoding::{AsciiSet, CONTROLS};
use serde_json::Value;
use std::fmt;

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
}

impl TemplateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Syntax(_) => "MDOK-E400",
            Self::MissingVariable(_) => "MDOK-E401",
            Self::Type(_) => "MDOK-E402",
            Self::UnsafeHeader => "MDOK-E403",
        }
    }
}

impl Template {
    pub fn parse(source: &str) -> Result<Self, TemplateError> {
        parse(source)
    }

    pub fn render(&self, values: &ValueMap) -> Result<String, TemplateError> {
        let mut output = String::new();
        for part in &self.parts {
            match part {
                TemplatePart::Literal(value) => output.push_str(value),
                TemplatePart::Expression(expression) => {
                    output.push_str(&render_expression(expression, values)?)
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
    let mut parts = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let relative_start = source[cursor..].find("{{");
        let relative_end = source[cursor..].find("}}");
        if relative_end.is_some_and(|end| relative_start.is_none_or(|start| end < start)) {
            return Err(TemplateError::Syntax("unmatched `}}`".into()));
        }
        let Some(relative_start) = relative_start else {
            parts.push(TemplatePart::Literal(source[cursor..].to_owned()));
            break;
        };
        let start = cursor + relative_start;
        if start > cursor {
            parts.push(TemplatePart::Literal(source[cursor..start].to_owned()));
        }
        let end = source[start + 2..]
            .find("}}")
            .ok_or_else(|| TemplateError::Syntax("unclosed `{{`".into()))?
            + start
            + 2;
        let inner = &source[start + 2..end];
        let (path, filter) = parse_expression(inner)?;
        parts.push(TemplatePart::Expression(TemplateExpression {
            path,
            filter,
            span: None,
        }));
        cursor = end + 2;
    }
    if source.is_empty() {
        parts.push(TemplatePart::Literal(String::new()));
    }
    Ok(Template {
        source: source.to_owned(),
        parts,
    })
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
    let value = lookup(values, &expression.path)?;
    match expression.filter {
        Filter::Json => {
            serde_json::to_string(value).map_err(|error| TemplateError::Type(error.to_string()))
        }
        Filter::Base64 => {
            let bytes = match value {
                Value::String(value) => value.as_bytes().to_vec(),
                Value::Array(values)
                    if values
                        .iter()
                        .all(|value| value.as_u64().is_some_and(|n| n <= 255)) =>
                {
                    values
                        .iter()
                        .map(|value| value.as_u64().unwrap() as u8)
                        .collect()
                }
                _ => {
                    return Err(TemplateError::Type(
                        "base64 expects a string or byte array".into(),
                    ));
                }
            };
            Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
        }
        Filter::Url => {
            let value = scalar(value)?;
            Ok(percent_encoding::percent_encode(value.as_bytes(), RFC3986_COMPONENT).to_string())
        }
        Filter::Header => {
            let value = scalar(value)?;
            if value.contains(['\r', '\n']) {
                Err(TemplateError::UnsafeHeader)
            } else {
                Ok(value.to_owned())
            }
        }
        Filter::String | Filter::Raw => scalar(value),
    }
}

fn scalar(value: &Value) -> Result<String, TemplateError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Bool(value) => Ok(if *value { "true" } else { "false" }.to_owned()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Null => Ok("null".to_owned()),
        Value::Array(_) | Value::Object(_) => {
            Err(TemplateError::Type("filter expects a scalar value".into()))
        }
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
}
