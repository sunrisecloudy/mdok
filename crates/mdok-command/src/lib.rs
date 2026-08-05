#![forbid(unsafe_code)]

//! Bounded direct-process execution for non-curl MDOK command steps.
//!
//! The runner accepts only an already-tokenized argv and a trusted command
//! profile. It never invokes a shell, expands variables, inherits stdin, or
//! forwards the caller's environment wholesale. Process groups are used so a
//! timeout or output limit also terminates descendants.

use command_group::CommandGroup;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub const E_POLICY: &str = "MDOK-E306";
pub const E_INVALID_ARGV: &str = "MDOK-E307";
pub const E_START: &str = "MDOK-E308";
pub const E_EXIT: &str = "MDOK-E309";
pub const E_TIMEOUT: &str = "MDOK-E310";
pub const E_LIMIT: &str = "MDOK-E311";
pub const E_ENVIRONMENT: &str = "MDOK-E312";

#[derive(Clone, Debug, Default)]
pub struct CommandProfile {
    /// Canonical absolute executable path selected by trusted configuration.
    pub program: PathBuf,
    /// Fixed non-secret environment values.
    pub env: BTreeMap<String, OsString>,
    /// Explicitly declared secret environment values.
    pub secret_env: BTreeMap<String, OsString>,
    /// Optional working directory selected by trusted configuration.
    pub working_directory: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct CommandPolicy {
    /// Profile key to trusted executable mapping. The key must equal argv[0].
    pub profiles: BTreeMap<String, CommandProfile>,
    /// Maximum wall-clock time before the process group is killed and reaped.
    pub timeout: Duration,
    /// Maximum combined bytes retained from stdout and stderr.
    pub max_output_bytes: usize,
    /// Maximum number of argv elements, including argv[0].
    pub max_args: usize,
    /// Maximum bytes in any one argv element.
    pub max_arg_bytes: usize,
    /// Maximum bytes across all argv elements.
    pub max_argv_bytes: usize,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        Self {
            profiles: BTreeMap::new(),
            timeout: Duration::from_secs(30),
            max_output_bytes: 1024 * 1024,
            max_args: 64,
            max_arg_bytes: 64 * 1024,
            max_argv_bytes: 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcessOutput {
    /// Logical argv supplied by the Markdown step. The executable path is not
    /// substituted here, so reports can use the configured profile identity.
    pub argv: Vec<String>,
    pub program: PathBuf,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub success: bool,
    pub timed_out: bool,
    pub output_limit_exceeded: bool,
    pub output_truncated: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub secret_env_used: bool,
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
    let profile = validate_argv(argv, policy)?;
    let started = Instant::now();
    let mut command = Command::new(&profile.program);
    command
        .args(&argv[1..])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(directory) = &profile.working_directory {
        command.current_dir(directory);
    }
    for (name, value) in profile.env.iter().chain(profile.secret_env.iter()) {
        command.env(name, value);
    }

    let mut child = command.group_spawn().map_err(|error| {
        CommandError::new(
            E_START,
            format!("could not start profile `{}`: {error}", argv[0]),
        )
    })?;
    let Some(stdout) = child.inner().stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(CommandError::new(
            E_START,
            "child stdout pipe was unavailable",
        ));
    };
    let Some(stderr) = child.inner().stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(CommandError::new(
            E_START,
            "child stderr pipe was unavailable",
        ));
    };
    let output_state = Arc::new(OutputState::new());
    let stdout_state = Arc::clone(&output_state);
    let stderr_state = Arc::clone(&output_state);
    let max_output = policy.max_output_bytes;
    let stdout_thread = thread::spawn(move || read_limited(stdout, max_output, stdout_state));
    let stderr_thread = thread::spawn(move || read_limited(stderr, max_output, stderr_state));

    let mut timed_out = false;
    let mut output_limit_exceeded = false;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| CommandError::new(E_START, format!("could not poll child: {error}")))?
        {
            break status;
        }
        if output_state.truncated.load(Ordering::Acquire) {
            output_limit_exceeded = true;
            let _ = child.kill();
            break child.wait().map_err(|error| {
                CommandError::new(
                    E_START,
                    format!("could not stop output-limited child: {error}"),
                )
            })?;
        }
        if started.elapsed() >= policy.timeout {
            timed_out = true;
            let _ = child.kill();
            break child.wait().map_err(|error| {
                CommandError::new(E_START, format!("could not stop timed-out child: {error}"))
            })?;
        }
        thread::sleep(Duration::from_millis(2));
    };
    let stdout = stdout_thread
        .join()
        .map_err(|_| CommandError::new(E_START, "stdout reader panicked"))?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| CommandError::new(E_START, "stderr reader panicked"))?;
    output_limit_exceeded |= output_state.truncated.load(Ordering::Acquire);
    let output_truncated = output_limit_exceeded;
    let signal = exit_signal(&status);
    let success = status.success() && !timed_out && !output_limit_exceeded;
    Ok(ProcessOutput {
        argv: argv.to_vec(),
        program: profile.program.clone(),
        exit_code: status.code(),
        signal,
        success,
        timed_out,
        output_limit_exceeded,
        output_truncated,
        stdout_bytes: stdout.len(),
        stderr_bytes: stderr.len(),
        secret_env_used: !profile.secret_env.is_empty(),
        stdout,
        stderr,
        duration: started.elapsed(),
    })
}

fn validate_argv<'a>(
    argv: &[String],
    policy: &'a CommandPolicy,
) -> Result<&'a CommandProfile, CommandError> {
    if policy.timeout.is_zero() {
        return Err(CommandError::new(
            E_LIMIT,
            "external command timeout must be greater than zero",
        ));
    }
    if policy.max_output_bytes == 0
        || policy.max_args == 0
        || policy.max_arg_bytes == 0
        || policy.max_argv_bytes == 0
    {
        return Err(CommandError::new(
            E_LIMIT,
            "external command resource limits must be greater than zero",
        ));
    }
    let Some(executable) = argv.first() else {
        return Err(CommandError::new(
            E_INVALID_ARGV,
            "external command argv is empty",
        ));
    };
    if executable.is_empty() || argv.iter().any(|argument| argument.contains('\0')) {
        return Err(CommandError::new(
            E_INVALID_ARGV,
            "external command argv is empty or contains NUL",
        ));
    }
    if argv.len() > policy.max_args {
        return Err(CommandError::new(
            E_LIMIT,
            format!(
                "external command has more than {} argv elements",
                policy.max_args
            ),
        ));
    }
    let argv_bytes = argv.iter().try_fold(0usize, |total, argument| {
        if argument.len() > policy.max_arg_bytes {
            return Err(CommandError::new(
                E_LIMIT,
                format!(
                    "external command argument exceeds {} bytes",
                    policy.max_arg_bytes
                ),
            ));
        }
        total.checked_add(argument.len()).ok_or_else(|| {
            CommandError::new(E_LIMIT, "external command argv byte count overflowed")
        })
    })?;
    if argv_bytes > policy.max_argv_bytes {
        return Err(CommandError::new(
            E_LIMIT,
            format!(
                "external command argv exceeds {} total bytes",
                policy.max_argv_bytes
            ),
        ));
    }
    if is_shell_interpreter(executable) {
        return Err(CommandError::new(
            E_POLICY,
            "shell interpreters are not allowed in exec fences",
        ));
    }
    let profile = policy.profiles.get(executable).ok_or_else(|| {
        CommandError::new(
            E_POLICY,
            format!("external command profile `{executable}` is not configured"),
        )
    })?;
    if profile.program.as_os_str().is_empty() || !profile.program.is_absolute() {
        return Err(CommandError::new(
            E_POLICY,
            format!("command profile `{executable}` must use an absolute program path"),
        ));
    }
    if is_shell_interpreter(
        profile
            .program
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default(),
    ) {
        return Err(CommandError::new(
            E_POLICY,
            "shell interpreter programs are not allowed in exec profiles",
        ));
    }
    validate_profile_environment(profile)?;
    let metadata = std::fs::metadata(&profile.program).map_err(|error| {
        CommandError::new(
            E_ENVIRONMENT,
            format!(
                "cannot inspect command profile `{}`: {error}",
                profile.program.display()
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(CommandError::new(
            E_ENVIRONMENT,
            format!(
                "command profile `{}` is not a regular file",
                profile.program.display()
            ),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(CommandError::new(
                E_ENVIRONMENT,
                format!(
                    "command profile `{}` is not executable",
                    profile.program.display()
                ),
            ));
        }
    }
    if let Some(directory) = &profile.working_directory
        && !directory.is_absolute()
    {
        return Err(CommandError::new(
            E_ENVIRONMENT,
            "command working directory must be absolute",
        ));
    }
    Ok(profile)
}

fn validate_profile_environment(profile: &CommandProfile) -> Result<(), CommandError> {
    for name in profile.env.keys().chain(profile.secret_env.keys()) {
        if name.is_empty() || name.contains('=') || name.contains('\0') {
            return Err(CommandError::new(
                E_ENVIRONMENT,
                format!("invalid command environment variable `{name}`"),
            ));
        }
    }
    Ok(())
}

fn is_shell_interpreter(executable: &str) -> bool {
    let basename = Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();
    matches!(
        basename.as_str(),
        "sh" | "bash"
            | "dash"
            | "zsh"
            | "fish"
            | "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
    )
}

struct OutputState {
    reserved: AtomicUsize,
    truncated: AtomicBool,
}

impl OutputState {
    fn new() -> Self {
        Self {
            reserved: AtomicUsize::new(0),
            truncated: AtomicBool::new(false),
        }
    }

    fn reserve(&self, requested: usize, limit: usize) -> usize {
        loop {
            let current = self.reserved.load(Ordering::Acquire);
            if current >= limit {
                self.truncated.store(true, Ordering::Release);
                return 0;
            }
            let available = requested.min(limit - current);
            if self
                .reserved
                .compare_exchange(
                    current,
                    current + available,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                if available < requested {
                    self.truncated.store(true, Ordering::Release);
                }
                return available;
            }
        }
    }
}

fn read_limited<R: Read>(mut reader: R, limit: usize, state: Arc<OutputState>) -> Vec<u8> {
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return output,
            Ok(count) => {
                let retained = state.reserve(count, limit);
                output.extend_from_slice(&buffer[..retained]);
                if retained < count {
                    return output;
                }
            }
            Err(_) => return output,
        }
    }
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_profile() -> (String, CommandProfile) {
        let executable = std::env::current_exe().expect("test executable path");
        (
            "fixture".into(),
            CommandProfile {
                program: executable,
                ..CommandProfile::default()
            },
        )
    }

    fn policy() -> CommandPolicy {
        let (name, profile) = fixture_profile();
        CommandPolicy {
            profiles: [(name, profile)].into_iter().collect(),
            timeout: Duration::from_secs(2),
            max_output_bytes: 4096,
            ..CommandPolicy::default()
        }
    }

    #[test]
    fn rejects_commands_outside_trusted_profiles() {
        let error = run(&["echo".into(), "ok".into()], &policy()).unwrap_err();
        assert_eq!(error.code, E_POLICY);
    }

    #[test]
    fn rejects_shell_interpreters_even_when_profiled() {
        let executable = std::env::current_exe().expect("test executable path");
        let mut command_policy = policy();
        command_policy.profiles.insert(
            "PowerShell.EXE".into(),
            CommandProfile {
                program: executable,
                ..CommandProfile::default()
            },
        );
        let error = run(&["PowerShell.EXE".into()], &command_policy).unwrap_err();
        assert_eq!(error.code, E_POLICY);
    }

    #[test]
    fn rejects_empty_nul_and_oversized_argv() {
        let command = policy();
        assert_eq!(run(&Vec::new(), &command).unwrap_err().code, E_INVALID_ARGV);
        assert_eq!(
            run(&["fixture".into(), "bad\0arg".into()], &command)
                .unwrap_err()
                .code,
            E_INVALID_ARGV
        );
        let mut limited = command;
        limited.max_arg_bytes = 2;
        assert_eq!(
            run(&["fixture".into(), "long".into()], &limited)
                .unwrap_err()
                .code,
            E_LIMIT
        );
    }

    #[test]
    fn captures_success_without_shell_expansion() {
        let output = run(&["fixture".into(), "--list".into()], &policy()).unwrap();
        assert!(output.success);
        assert!(!output.stdout.is_empty());
    }

    #[test]
    fn reports_nonzero_exit() {
        let mut output = run(
            &["fixture".into(), "--definitely-invalid".into()],
            &policy(),
        )
        .unwrap();
        assert!(!output.success);
        assert!(output.exit_code.is_some());
        output.stdout.clear();
    }

    #[test]
    fn enforces_combined_output_limit() {
        let mut command = policy();
        command.max_output_bytes = 1;
        let output = run(&["fixture".into(), "--list".into()], &command).unwrap();
        assert!(output.output_limit_exceeded);
        assert!(output.output_truncated);
        assert!(output.stdout.len() + output.stderr.len() <= 1);
    }

    #[test]
    fn rejects_zero_limits() {
        let mut timeout = policy();
        timeout.timeout = Duration::ZERO;
        assert_eq!(
            run(&["fixture".into()], &timeout).unwrap_err().code,
            E_LIMIT
        );

        let mut output = policy();
        output.max_output_bytes = 0;
        assert_eq!(run(&["fixture".into()], &output).unwrap_err().code, E_LIMIT);
    }
}
