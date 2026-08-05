#![forbid(unsafe_code)]

//! Bounded direct-process execution for non-curl MDOK command steps.
//!
//! The runner deliberately accepts only an already-tokenized argv. It never
//! invokes a shell, expands variables, inherits stdin, or forwards the
//! caller's environment wholesale.

use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub const E_POLICY: &str = "MDOK-E306";
pub const E_LIMIT: &str = "MDOK-E700";
pub const E_EXECUTION: &str = "MDOK-E607";

#[derive(Clone, Debug)]
pub struct CommandPolicy {
    /// Exact executable names or paths permitted by the runner.
    pub allowed_commands: Vec<String>,
    /// Maximum wall-clock time before the child is killed and reaped.
    pub timeout: Duration,
    /// Maximum bytes retained from either stdout or stderr.
    pub max_output_bytes: usize,
    /// Optional PATH supplied after the child's inherited environment is cleared.
    pub path: Option<OsString>,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        Self {
            allowed_commands: Vec::new(),
            timeout: Duration::from_secs(30),
            max_output_bytes: 1024 * 1024,
            path: std::env::var_os("PATH"),
        }
    }
}

impl CommandPolicy {
    /// Adds a bare executable name to the exact allowlist.
    pub fn allow_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.allowed_commands.push(name.into());
        self
    }

    /// Adds an executable path to the exact allowlist.
    pub fn allow_path(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.allowed_commands
            .push(path.as_ref().to_string_lossy().into_owned());
        self
    }
}

#[derive(Clone, Debug)]
pub struct ProcessOutput {
    pub argv: Vec<String>,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub timed_out: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

pub fn run(argv: &[String], policy: &CommandPolicy) -> Result<ProcessOutput, CommandError> {
    validate_argv(argv, policy)?;
    let started = Instant::now();
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(path) = &policy.path {
        command.env("PATH", path);
    }
    let mut child = command.spawn().map_err(|error| {
        CommandError::new(
            E_EXECUTION,
            format!("could not start `{}`: {error}", argv[0]),
        )
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CommandError::new(E_EXECUTION, "child stdout pipe was unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CommandError::new(E_EXECUTION, "child stderr pipe was unavailable"))?;
    let output_limit_hit = Arc::new(AtomicBool::new(false));
    let stdout_limit = Arc::clone(&output_limit_hit);
    let stderr_limit = Arc::clone(&output_limit_hit);
    let max_output = policy.max_output_bytes;
    let stdout_thread = thread::spawn(move || read_limited(stdout, max_output, stdout_limit));
    let stderr_thread = thread::spawn(move || read_limited(stderr, max_output, stderr_limit));

    let mut timed_out = false;
    let status = loop {
        if output_limit_hit.load(Ordering::Acquire) {
            let _ = child.kill();
            break child.wait().map_err(|error| {
                CommandError::new(E_EXECUTION, format!("could not stop child: {error}"))
            })?;
        }
        if started.elapsed() >= policy.timeout {
            timed_out = true;
            let _ = child.kill();
            break child.wait().map_err(|error| {
                CommandError::new(
                    E_EXECUTION,
                    format!("could not stop timed-out child: {error}"),
                )
            })?;
        }
        if let Some(status) = child.try_wait().map_err(|error| {
            CommandError::new(E_EXECUTION, format!("could not poll child: {error}"))
        })? {
            break status;
        }
        thread::sleep(Duration::from_millis(5));
    };
    let stdout = stdout_thread
        .join()
        .map_err(|_| CommandError::new(E_EXECUTION, "stdout reader panicked"))?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| CommandError::new(E_EXECUTION, "stderr reader panicked"))?;
    if output_limit_hit.load(Ordering::Acquire) {
        return Err(CommandError::new(
            E_LIMIT,
            format!("command output exceeded {} bytes", policy.max_output_bytes),
        ));
    }
    Ok(ProcessOutput {
        argv: argv.to_vec(),
        exit_code: status.code(),
        success: status.success() && !timed_out,
        timed_out,
        stdout,
        stderr,
        duration: started.elapsed(),
    })
}

fn validate_argv(argv: &[String], policy: &CommandPolicy) -> Result<(), CommandError> {
    let Some(executable) = argv.first() else {
        return Err(CommandError::new(
            E_POLICY,
            "external command argv is empty",
        ));
    };
    if executable.is_empty() || executable.contains('\0') {
        return Err(CommandError::new(
            E_POLICY,
            "external command executable is empty or contains NUL",
        ));
    }
    let basename = Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable);
    if matches!(
        basename,
        "sh" | "bash" | "dash" | "zsh" | "fish" | "cmd" | "cmd.exe" | "powershell" | "pwsh"
    ) {
        return Err(CommandError::new(
            E_POLICY,
            "shell interpreters are not allowed in exec fences",
        ));
    }
    if !policy
        .allowed_commands
        .iter()
        .any(|allowed| allowed == executable)
    {
        return Err(CommandError::new(
            E_POLICY,
            format!("external command `{executable}` is not allowed"),
        ));
    }
    if argv.iter().any(|argument| argument.contains('\0')) {
        return Err(CommandError::new(
            E_POLICY,
            "external command argv contains NUL",
        ));
    }
    Ok(())
}

fn read_limited<R: Read>(mut reader: R, limit: usize, limit_hit: Arc<AtomicBool>) -> Vec<u8> {
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return output,
            Ok(count) => {
                if output.len().saturating_add(count) > limit {
                    limit_hit.store(true, Ordering::Release);
                    return output;
                }
                output.extend_from_slice(&buffer[..count]);
            }
            Err(_) => return output,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(command: &str) -> CommandPolicy {
        CommandPolicy {
            allowed_commands: vec![command.to_owned()],
            timeout: Duration::from_secs(2),
            max_output_bytes: 4096,
            ..CommandPolicy::default()
        }
    }

    #[test]
    fn rejects_commands_outside_allowlist() {
        let error = run(&["echo".into(), "ok".into()], &policy("printf")).unwrap_err();
        assert_eq!(error.code, E_POLICY);
    }

    #[test]
    fn rejects_empty_argv_and_nul_bytes() {
        let command = policy("printf");
        let empty: Vec<String> = Vec::new();
        let empty_error = run(&empty, &command).unwrap_err();
        assert_eq!(empty_error.code, E_POLICY);

        let nul_error = run(&["printf".into(), "bad\0arg".into()], &command).unwrap_err();
        assert_eq!(nul_error.code, E_POLICY);
    }

    #[test]
    fn allows_an_exact_executable_path() {
        let executable = std::env::current_exe().expect("test executable path");
        let mut command = CommandPolicy::default();
        command.allow_path(&executable);
        let output = run(
            &[executable.to_string_lossy().into_owned(), "--help".into()],
            &command,
        )
        .expect("the exact test executable path should be allowed");
        assert!(output.success);
    }

    #[test]
    fn captures_successful_output_without_shell_expansion() {
        let output = run(
            &["printf".into(), "%s".into(), "$(not-a-shell)".into()],
            &policy("printf"),
        )
        .unwrap();
        assert!(output.success);
        assert_eq!(output.stdout, b"$(not-a-shell)");
    }

    #[test]
    fn reports_nonzero_exit() {
        let output = run(&["false".into()], &policy("false")).unwrap();
        assert!(!output.success);
        assert!(output.exit_code.is_some_and(|code| code != 0));
    }

    #[test]
    fn rejects_output_over_limit() {
        let mut command = policy("printf");
        command.max_output_bytes = 3;
        let error = run(&["printf".into(), "1234".into()], &command).unwrap_err();
        assert_eq!(error.code, E_LIMIT);
    }

    #[test]
    fn kills_timed_out_child() {
        let mut command = policy("sleep");
        command.timeout = Duration::from_millis(20);
        let output = run(&["sleep".into(), "1".into()], &command).unwrap();
        assert!(output.timed_out);
        assert!(!output.success);
    }
}
