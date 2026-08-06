//! Canonical serialization for transient direct-argv invocations.
//!
//! MDOK command fences use a restricted shell-word grammar. This module emits
//! every argv element as one double-quoted word, escaping the characters that
//! the grammar treats specially. Values therefore remain data when the
//! generated Markdown is parsed again.

use std::collections::BTreeMap;
use std::fmt;

/// The command-fence language to generate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandKind {
    /// A curl request fence.
    Curl,
    /// A trusted direct-argv exec fence.
    Exec,
}

impl CommandKind {
    /// The Markdown language token for this command kind.
    pub const fn fence_language(self) -> &'static str {
        match self {
            Self::Curl => "curl",
            Self::Exec => "exec",
        }
    }
}

/// The fixed step name for a one-command transient document.
pub const TRANSIENT_STEP_NAME: &str = "call";
/// Existing restricted shell/command argv limit.
pub const MAX_TRANSIENT_ARGUMENTS: usize = 64;
/// Existing per-argument byte limit.
pub const MAX_TRANSIENT_ARGUMENT_BYTES: usize = 64 * 1024;
/// Existing aggregate argv byte limit.
pub const MAX_TRANSIENT_ARGV_BYTES: usize = 1024 * 1024;
/// Existing CLI executable-fence body limit.
pub const MAX_TRANSIENT_FENCE_BODY_BYTES: usize = 512 * 1024;

/// An argv value that cannot be represented safely by the transient format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalCommandError {
    /// No argv was supplied.
    EmptyArgv,
    /// An exec command has no executable name.
    EmptyProgram,
    /// Curl fences must start with the literal `curl`.
    InvalidCurlProgram { actual: String },
    /// NUL cannot cross the Rust/C argv boundary.
    NulByte { argument: usize },
    /// Markdown line parsing can strip CR at a line boundary.
    CarriageReturn { argument: usize },
    /// Too many argv elements were supplied.
    TooManyArguments { limit: usize, observed: usize },
    /// One argv element is too large.
    ArgumentTooLarge {
        argument: usize,
        limit: usize,
        observed: usize,
    },
    /// The decoded argv is too large in aggregate.
    ArgvTooLarge { limit: usize, observed: usize },
    /// Quoting overhead makes the generated fence too large.
    FenceTooLarge { limit: usize, observed: usize },
}

impl CanonicalCommandError {
    /// The stable diagnostic code family for this conversion error.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TooManyArguments { .. }
            | Self::ArgumentTooLarge { .. }
            | Self::ArgvTooLarge { .. }
            | Self::FenceTooLarge { .. } => "MDOK-E405",
            Self::EmptyArgv
            | Self::EmptyProgram
            | Self::InvalidCurlProgram { .. }
            | Self::NulByte { .. }
            | Self::CarriageReturn { .. } => "MDOK-E202",
        }
    }
}

impl fmt::Display for CanonicalCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArgv => formatter.write_str("direct command argv is empty"),
            Self::EmptyProgram => formatter.write_str("direct command executable is empty"),
            Self::InvalidCurlProgram { actual } => {
                write!(
                    formatter,
                    "curl command must start with `curl`, got `{actual}`"
                )
            }
            Self::NulByte { argument } => {
                write!(formatter, "argument {argument} contains a NUL byte")
            }
            Self::CarriageReturn { argument } => write!(
                formatter,
                "argument {argument} contains a carriage return that cannot be preserved"
            ),
            Self::TooManyArguments { limit, observed } => write!(
                formatter,
                "direct command has {observed} arguments; the maximum is {limit}"
            ),
            Self::ArgumentTooLarge {
                argument,
                limit,
                observed,
            } => write!(
                formatter,
                "argument {argument} is {observed} bytes; the maximum is {limit}"
            ),
            Self::ArgvTooLarge { limit, observed } => write!(
                formatter,
                "direct command argv is {observed} bytes; the maximum is {limit}"
            ),
            Self::FenceTooLarge { limit, observed } => write!(
                formatter,
                "generated command fence is {observed} bytes; the maximum is {limit}"
            ),
        }
    }
}

impl std::error::Error for CanonicalCommandError {}

/// Convert a direct argv into a canonical one-command Markdown document.
///
/// The executable name remains a literal command token. Every remaining
/// argument is stored in a TOML variable and inserted with the `raw` filter.
/// This is important because direct data may itself contain `{{...}}`; putting
/// it directly in a command fence would make the template parser reinterpret
/// the data as a template expression.
pub fn argv_to_markdown(
    kind: CommandKind,
    argv: &[String],
) -> Result<String, CanonicalCommandError> {
    validate_argv(kind, argv)?;

    let variables = argv
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, argument)| (format!("mdok_arg_{index}"), argument.clone()))
        .collect::<BTreeMap<_, _>>();
    let variables_block =
        toml::to_string(&variables).map_err(|error| CanonicalCommandError::FenceTooLarge {
            limit: MAX_TRANSIENT_FENCE_BODY_BYTES,
            observed: error.to_string().len(),
        })?;
    let command_prefix = format!(
        "```{} mdok name={}\n",
        kind.fence_language(),
        TRANSIENT_STEP_NAME
    );
    let command_suffix = "\n```\n";
    let mut command = command_prefix;
    append_quoted_argument(&mut command, &argv[0]);
    for index in 1..argv.len() {
        command.push(' ');
        command.push_str(&format!("{{{{mdok_arg_{index}|raw}}}}"));
    }
    command.push_str(command_suffix);
    let document =
        format!("# Transient MDOK call\n\n```toml mdok vars\n{variables_block}```\n\n{command}");
    if command.len() > MAX_TRANSIENT_FENCE_BODY_BYTES
        || document.len() > MAX_TRANSIENT_FENCE_BODY_BYTES
    {
        return Err(CanonicalCommandError::FenceTooLarge {
            limit: MAX_TRANSIENT_FENCE_BODY_BYTES,
            observed: document.len(),
        });
    }
    Ok(document)
}

/// Compatibility entry point for the future CLI: infer curl from argv[0],
/// otherwise generate an exec fence.
pub fn canonical_command_markdown(argv: &[String]) -> Result<String, String> {
    let result = match argv.first().map(String::as_str) {
        Some("curl") => curl_argv_to_markdown(argv),
        Some(_) => exec_argv_to_markdown(argv),
        None => return Err(CanonicalCommandError::EmptyArgv.to_string()),
    };
    result.map_err(|error| format!("{}: {error}", error.code()))
}

/// Convert a direct curl argv into a canonical curl fence.
pub fn curl_argv_to_markdown(argv: &[String]) -> Result<String, CanonicalCommandError> {
    argv_to_markdown(CommandKind::Curl, argv)
}

/// Convert a trusted direct-command argv into a canonical exec fence.
pub fn exec_argv_to_markdown(argv: &[String]) -> Result<String, CanonicalCommandError> {
    argv_to_markdown(CommandKind::Exec, argv)
}

fn validate_argv(kind: CommandKind, argv: &[String]) -> Result<(), CanonicalCommandError> {
    if argv.is_empty() {
        return Err(CanonicalCommandError::EmptyArgv);
    }
    if argv.len() > MAX_TRANSIENT_ARGUMENTS {
        return Err(CanonicalCommandError::TooManyArguments {
            limit: MAX_TRANSIENT_ARGUMENTS,
            observed: argv.len(),
        });
    }
    match kind {
        CommandKind::Curl if argv[0] != "curl" => {
            return Err(CanonicalCommandError::InvalidCurlProgram {
                actual: argv[0].clone(),
            });
        }
        CommandKind::Exec if argv[0].is_empty() => {
            return Err(CanonicalCommandError::EmptyProgram);
        }
        CommandKind::Curl | CommandKind::Exec => {}
    }

    let mut total_bytes = 0usize;
    for (index, argument) in argv.iter().enumerate() {
        if argument.len() > MAX_TRANSIENT_ARGUMENT_BYTES {
            return Err(CanonicalCommandError::ArgumentTooLarge {
                argument: index,
                limit: MAX_TRANSIENT_ARGUMENT_BYTES,
                observed: argument.len(),
            });
        }
        if argument.contains('\0') {
            return Err(CanonicalCommandError::NulByte { argument: index });
        }
        if argument.contains('\r') {
            return Err(CanonicalCommandError::CarriageReturn { argument: index });
        }
        total_bytes =
            total_bytes
                .checked_add(argument.len())
                .ok_or(CanonicalCommandError::ArgvTooLarge {
                    limit: MAX_TRANSIENT_ARGV_BYTES,
                    observed: usize::MAX,
                })?;
    }
    if total_bytes > MAX_TRANSIENT_ARGV_BYTES {
        return Err(CanonicalCommandError::ArgvTooLarge {
            limit: MAX_TRANSIENT_ARGV_BYTES,
            observed: total_bytes,
        });
    }
    Ok(())
}

fn append_quoted_argument(output: &mut String, argument: &str) {
    output.push('"');
    for character in argument.chars() {
        if matches!(character, '"' | '\\' | '$' | '`') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_curl_and_exec_fences() {
        let curl = curl_argv_to_markdown(&["curl".into(), "--fail".into()]).unwrap();
        assert!(curl.contains("```toml mdok vars\nmdok_arg_1 = \"--fail\"\n```"));
        assert!(curl.contains("```curl mdok name=call\n\"curl\" {{mdok_arg_1|raw}}\n```"));

        let exec = exec_argv_to_markdown(&["/usr/bin/tool".into()]).unwrap();
        assert!(exec.contains("```exec mdok name=call\n\"/usr/bin/tool\"\n```"));
    }

    #[test]
    fn round_trips_spaces_empty_quotes_unicode_backslashes_newlines_and_operators() {
        let argv = vec![
            "curl".to_owned(),
            String::new(),
            "has spaces".to_owned(),
            "quotes: \"double\" and 'single'".to_owned(),
            "Unicode: 日本語 🚀".to_owned(),
            r"C:\tmp\file".to_owned(),
            "line one\nline two".to_owned(),
            "literal operators: | ; & < > $ ( ) { } * ? [ ]".to_owned(),
            "line\n```\nthat must not close the Markdown fence".to_owned(),
        ];
        let document = curl_argv_to_markdown(&argv).unwrap();
        let variables = generated_variables(&document);
        for (index, argument) in argv.iter().enumerate().skip(1) {
            assert_eq!(variables.get(&format!("mdok_arg_{index}")), Some(argument));
        }
        assert!(document.contains("{{mdok_arg_1|raw}}"));
        assert!(document.contains("{{mdok_arg_8|raw}}"));
    }

    #[test]
    fn compatibility_entry_point_selects_exec_for_non_curl_programs() {
        let source = canonical_command_markdown(&["tool".into(), "a b".into()]).unwrap();
        assert!(source.contains("```exec mdok name=call\n\"tool\" {{mdok_arg_1|raw}}\n```"));
        assert_eq!(generated_variables(&source)["mdok_arg_1"], "a b");
    }

    #[test]
    fn rejects_unrepresentable_values_and_resource_overruns() {
        assert_eq!(
            curl_argv_to_markdown(&[]).unwrap_err(),
            CanonicalCommandError::EmptyArgv
        );
        assert_eq!(
            curl_argv_to_markdown(&["wget".into()]).unwrap_err(),
            CanonicalCommandError::InvalidCurlProgram {
                actual: "wget".into()
            }
        );
        assert_eq!(
            exec_argv_to_markdown(&[String::new()]).unwrap_err(),
            CanonicalCommandError::EmptyProgram
        );
        assert_eq!(
            exec_argv_to_markdown(&["tool".into(), "bad\0value".into()]).unwrap_err(),
            CanonicalCommandError::NulByte { argument: 1 }
        );
        assert_eq!(
            exec_argv_to_markdown(&["tool".into(), "bad\rvalue".into()]).unwrap_err(),
            CanonicalCommandError::CarriageReturn { argument: 1 }
        );

        let too_many = std::iter::once("tool".to_owned())
            .chain((0..MAX_TRANSIENT_ARGUMENTS).map(|_| "x".to_owned()))
            .collect::<Vec<_>>();
        assert!(matches!(
            exec_argv_to_markdown(&too_many),
            Err(CanonicalCommandError::TooManyArguments { .. })
        ));
        assert!(matches!(
            exec_argv_to_markdown(&[
                "tool".to_owned(),
                "x".repeat(MAX_TRANSIENT_ARGUMENT_BYTES + 1)
            ]),
            Err(CanonicalCommandError::ArgumentTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_fences_that_exceed_the_body_budget_after_quoting() {
        let arguments = std::iter::once("tool".to_owned())
            .chain((0..MAX_TRANSIENT_ARGUMENTS - 1).map(|_| "x".repeat(9_000)))
            .collect::<Vec<_>>();
        assert!(matches!(
            exec_argv_to_markdown(&arguments),
            Err(CanonicalCommandError::FenceTooLarge { .. })
        ));
    }

    fn generated_variables(document: &str) -> BTreeMap<String, String> {
        let body = document
            .strip_prefix("# Transient MDOK call\n\n```toml mdok vars\n")
            .and_then(|value| value.split_once("```\n\n"))
            .map(|(body, _)| body)
            .expect("canonical variables fence");
        toml::from_str(body).expect("canonical variables should be valid TOML")
    }
}
