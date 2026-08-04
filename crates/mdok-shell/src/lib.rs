#![forbid(unsafe_code)]

use mdok_core::{SourceSpan, ValueMap};
use mdok_template::{TemplateExpression, TemplatePart, parse as parse_template, render_expression};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WordSegment {
    Literal(String),
    Template(TemplateExpression),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub segments: Vec<WordSegment>,
    pub span: SourceSpan,
}

impl Argument {
    pub fn is_literal(&self, value: &str) -> bool {
        let mut joined = String::new();
        for segment in &self.segments {
            let WordSegment::Literal(actual) = segment else {
                return false;
            };
            joined.push_str(actual);
        }
        joined == value
    }

    pub fn render(&self, values: &ValueMap) -> Result<String, ShellError> {
        let mut output = String::new();
        for segment in &self.segments {
            match segment {
                WordSegment::Literal(value) => output.push_str(value),
                WordSegment::Template(expression) => {
                    output.push_str(&render_expression(expression, values).map_err(|error| {
                        ShellError::template(self.span.clone(), error.to_string())
                    })?)
                }
            }
        }
        Ok(output)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgvPlan {
    pub source: String,
    pub path: PathBuf,
    pub arguments: Vec<Argument>,
}

impl ArgvPlan {
    pub fn evaluate(&self, values: &ValueMap) -> Result<Vec<String>, ShellError> {
        let argv: Vec<String> = self
            .arguments
            .iter()
            .map(|argument| argument.render(values))
            .collect::<Result<_, _>>()?;
        if argv.first().is_none_or(|value| value != "curl") {
            return Err(ShellError::new(
                "MDOK-E202",
                "command must start with `curl`",
                None,
            ));
        }
        Ok(argv)
    }

    pub fn templates(&self) -> impl Iterator<Item = &TemplateExpression> {
        self.arguments.iter().flat_map(|argument| {
            argument
                .segments
                .iter()
                .filter_map(|segment| match segment {
                    WordSegment::Template(expression) => Some(expression),
                    WordSegment::Literal(_) => None,
                })
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ShellError {
    pub code: &'static str,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl ShellError {
    pub fn new(code: &'static str, message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            code,
            message: message.into(),
            span,
        }
    }
    fn template(span: SourceSpan, message: String) -> Self {
        Self::new("MDOK-E402", message, Some(span))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quote {
    Unquoted,
    Single,
    Double,
}

pub fn parse(source: &str) -> Result<ArgvPlan, ShellError> {
    parse_with_path(source, PathBuf::from("<curl>"))
}

pub fn parse_curl_source(source: &str) -> Result<ArgvPlan, ShellError> {
    parse(source)
}

pub fn evaluate_argv(plan: &ArgvPlan, values: &ValueMap) -> Result<Vec<String>, ShellError> {
    plan.evaluate(values)
}

pub fn parse_with_path(source: &str, path: impl Into<PathBuf>) -> Result<ArgvPlan, ShellError> {
    let path = path.into();
    let mut arguments = Vec::new();
    let mut current = Vec::new();
    let mut quote = Quote::Unquoted;
    let mut word_start = None;
    let mut index = 0;
    let bytes = source.as_bytes();
    while index < bytes.len() {
        let byte = bytes[index];
        if source[index..].starts_with("{{") {
            let end = source[index + 2..].find("}}").ok_or_else(|| {
                error(
                    "MDOK-E400",
                    "unclosed template",
                    &path,
                    word_start.unwrap_or(index),
                    index + 2,
                )
            })? + index
                + 2;
            let template_source = &source[index..end + 2];
            let template = parse_template(template_source).map_err(|error| {
                ShellError::new(
                    error.code(),
                    error.to_string(),
                    Some(span(&path, index, end + 2, source)),
                )
            })?;
            for part in template.parts {
                match part {
                    TemplatePart::Literal(value) => push_literal(&mut current, value),
                    TemplatePart::Expression(mut expression) => {
                        expression.span = Some(span(&path, index, end + 2, source));
                        current.push(WordSegment::Template(expression))
                    }
                }
            }
            word_start.get_or_insert(index);
            index = end + 2;
            continue;
        }
        match quote {
            Quote::Unquoted => match byte {
                b' ' | b'\t' | b'\r' => {
                    finish_word(
                        &mut arguments,
                        &mut current,
                        &mut word_start,
                        &path,
                        index,
                        source,
                    );
                    index += 1;
                }
                b'\n' => {
                    return Err(error(
                        "MDOK-E201",
                        "unescaped newline would terminate the curl command",
                        &path,
                        index,
                        index + 1,
                    ));
                }
                b'#' if current.is_empty() => {
                    while index < bytes.len() && bytes[index] != b'\n' {
                        index += 1;
                    }
                    if index < bytes.len() {
                        return Err(error(
                            "MDOK-E201",
                            "a comment cannot be followed by another command",
                            &path,
                            index,
                            index + 1,
                        ));
                    }
                }
                b'\'' => {
                    word_start.get_or_insert(index);
                    if current.is_empty() {
                        current.push(WordSegment::Literal(String::new()));
                    }
                    quote = Quote::Single;
                    index += 1;
                }
                b'"' => {
                    word_start.get_or_insert(index);
                    if current.is_empty() {
                        current.push(WordSegment::Literal(String::new()));
                    }
                    quote = Quote::Double;
                    index += 1;
                }
                b'\\' => {
                    word_start.get_or_insert(index);
                    index += 1;
                    if index >= bytes.len() {
                        return Err(error(
                            "MDOK-E200",
                            "trailing backslash",
                            &path,
                            index - 1,
                            index,
                        ));
                    }
                    if bytes[index] == b'\n' {
                        index += 1;
                    } else {
                        push_literal(&mut current, next_char(source, &mut index));
                    }
                }
                b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')' | b'`' | b'$' | b'{' | b'}'
                | b'*' | b'?' => {
                    return Err(error(
                        "MDOK-E201",
                        "forbidden shell construct",
                        &path,
                        index,
                        index + 1,
                    ));
                }
                _ => {
                    word_start.get_or_insert(index);
                    if byte == b'=' && current.is_empty() {
                        return Err(error(
                            "MDOK-E201",
                            "shell assignments are not allowed",
                            &path,
                            index,
                            index + 1,
                        ));
                    }
                    push_literal(&mut current, next_char(source, &mut index));
                }
            },
            Quote::Single => match byte {
                b'\'' => {
                    quote = Quote::Unquoted;
                    index += 1;
                }
                _ => {
                    word_start.get_or_insert(index);
                    push_literal(&mut current, next_char(source, &mut index));
                }
            },
            Quote::Double => match byte {
                b'"' => {
                    quote = Quote::Unquoted;
                    index += 1;
                }
                b'\\' => {
                    word_start.get_or_insert(index);
                    index += 1;
                    if index >= bytes.len() {
                        return Err(error(
                            "MDOK-E200",
                            "trailing backslash in quoted word",
                            &path,
                            index - 1,
                            index,
                        ));
                    }
                    if matches!(bytes[index], b'"' | b'\\' | b'$' | b'`' | b'\n') {
                        if bytes[index] != b'\n' {
                            push_literal(&mut current, next_char(source, &mut index));
                        } else {
                            index += 1;
                        }
                    } else {
                        push_literal(&mut current, "\\".into());
                    }
                }
                b'$' | b'`' => {
                    return Err(error(
                        "MDOK-E201",
                        "shell expansion is not allowed",
                        &path,
                        index,
                        index + 1,
                    ));
                }
                _ => {
                    word_start.get_or_insert(index);
                    push_literal(&mut current, next_char(source, &mut index));
                }
            },
        }
    }
    if quote != Quote::Unquoted {
        return Err(error(
            "MDOK-E200",
            "unterminated shell quote",
            &path,
            word_start.unwrap_or(0),
            source.len(),
        ));
    }
    finish_word(
        &mut arguments,
        &mut current,
        &mut word_start,
        &path,
        source.len(),
        source,
    );
    if arguments.is_empty() {
        return Err(ShellError::new("MDOK-E202", "curl fence is empty", None));
    }
    if !arguments[0].is_literal("curl") {
        return Err(ShellError::new(
            "MDOK-E202",
            "first word must be the literal `curl`",
            Some(arguments[0].span.clone()),
        ));
    }
    Ok(ArgvPlan {
        source: source.to_owned(),
        path,
        arguments,
    })
}

fn finish_word(
    arguments: &mut Vec<Argument>,
    current: &mut Vec<WordSegment>,
    start: &mut Option<usize>,
    path: &Path,
    end: usize,
    source: &str,
) {
    if current.is_empty() {
        return;
    }
    arguments.push(Argument {
        segments: std::mem::take(current),
        span: span(path, start.take().unwrap_or(end), end, source),
    });
}

fn push_literal(segments: &mut Vec<WordSegment>, value: String) {
    if value.is_empty() {
        return;
    }
    if let Some(WordSegment::Literal(previous)) = segments.last_mut() {
        previous.push_str(&value);
    } else {
        segments.push(WordSegment::Literal(value));
    }
}

fn next_char(source: &str, index: &mut usize) -> String {
    let character = source[*index..].chars().next().unwrap();
    *index += character.len_utf8();
    character.to_string()
}

fn span(path: &Path, start: usize, end: usize, source: &str) -> SourceSpan {
    let before = &source[..start.min(source.len())];
    SourceSpan::new(
        path.to_path_buf(),
        start,
        end,
        before.bytes().filter(|byte| *byte == b'\n').count() as u32 + 1,
        before
            .rsplit('\n')
            .next()
            .map_or(1, |line| line.chars().count() as u32 + 1),
    )
}

fn error(code: &'static str, message: &str, path: &Path, start: usize, end: usize) -> ShellError {
    ShellError::new(
        code,
        message,
        Some(span(path, start, end, &" ".repeat(end.max(start)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_quoted_words_and_templates_without_retokenizing_values() {
        let plan = parse(r#"curl --header "X-Name: {{name|header}}" "{{base}}/me""#).unwrap();
        let values = [
            ("name".into(), json!("W \"Admin\"")),
            ("base".into(), json!("https://example.test")),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            plan.evaluate(&values).unwrap(),
            [
                "curl",
                "--header",
                "X-Name: W \"Admin\"",
                "https://example.test/me"
            ]
        );
    }

    #[test]
    fn rejects_shell_operators_expansions_and_multiple_commands() {
        for source in [
            "curl x | jq .",
            "curl $(touch /tmp/x)",
            "curl x; echo bad",
            "curl x > out",
            "CURL=x curl y",
        ] {
            assert!(parse(source).is_err(), "accepted {source}");
        }
    }

    #[test]
    fn supports_backslash_continuations() {
        let plan = parse("curl --url https://example.test/\\\nusers").unwrap();
        assert_eq!(
            plan.evaluate(&ValueMap::new()).unwrap()[2],
            "https://example.test/users"
        );
    }
}
