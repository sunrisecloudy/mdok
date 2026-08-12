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
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
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
    let deadline = started
        .checked_add(policy.timeout)
        .unwrap_or_else(Instant::now);

    let mut timed_out = false;
    let mut output_limit_exceeded = false;
    let status = loop {
        if Instant::now() >= deadline {
            timed_out = true;
            break reap_group_with_deadline(child, None, deadline)?;
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| CommandError::new(E_START, format!("could not poll child: {error}")))?
        {
            break reap_group_with_deadline(child, Some(status), deadline)?;
        }
        if output_state.truncated.load(Ordering::Acquire) {
            output_limit_exceeded = true;
            break reap_group_with_deadline(child, None, deadline)?;
        }
        thread::sleep(Duration::from_millis(2));
    };
    let (stdout, stderr) = join_readers_with_deadline(stdout_thread, stderr_thread, deadline)?;
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

/// Kill the complete process group and reap it without allowing cleanup to
/// extend the command's original wall-clock deadline. A separate reaper is
/// used because a descendant can keep the group wait open after the leader has
/// already exited.
fn reap_group_with_deadline(
    mut child: command_group::GroupChild,
    leader_status: Option<ExitStatus>,
    deadline: Instant,
) -> Result<ExitStatus, CommandError> {
    let _ = child.kill();
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(child.wait());
    });
    let remaining = deadline.saturating_duration_since(Instant::now());
    match receiver.recv_timeout(remaining) {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(error)) => Err(CommandError::new(
            E_START,
            format!("could not reap command group: {error}"),
        )),
        Err(RecvTimeoutError::Timeout) => Err(CommandError::new(
            E_TIMEOUT,
            "command group cleanup exceeded the command deadline",
        )),
        Err(RecvTimeoutError::Disconnected) => {
            if let Some(status) = leader_status {
                Ok(status)
            } else {
                Err(CommandError::new(
                    E_START,
                    "command group reaper exited unexpectedly",
                ))
            }
        }
    }
}

fn join_readers_with_deadline(
    stdout_thread: thread::JoinHandle<Vec<u8>>,
    stderr_thread: thread::JoinHandle<Vec<u8>>,
    deadline: Instant,
) -> Result<(Vec<u8>, Vec<u8>), CommandError> {
    while !stdout_thread.is_finished() || !stderr_thread.is_finished() {
        if Instant::now() >= deadline {
            return Err(CommandError::new(
                E_TIMEOUT,
                "stdout/stderr cleanup exceeded the command deadline",
            ));
        }
        thread::sleep(Duration::from_millis(1));
    }
    let stdout = stdout_thread
        .join()
        .map_err(|_| CommandError::new(E_START, "stdout reader panicked"))?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| CommandError::new(E_START, "stderr reader panicked"))?;
    Ok((stdout, stderr))
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
    let canonical = std::fs::canonicalize(&profile.program).map_err(|error| {
        CommandError::new(
            E_ENVIRONMENT,
            format!(
                "cannot canonicalize command profile `{}`: {error}",
                profile.program.display()
            ),
        )
    })?;
    if canonical != profile.program {
        return Err(CommandError::new(
            E_ENVIRONMENT,
            format!(
                "command profile `{}` must be canonicalized",
                profile.program.display()
            ),
        ));
    }
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
        // F8: the basename-only `is_shell_interpreter` check above can be
        // defeated by a profile whose program is a non-shell-named script
        // (e.g. validate.sh, or an extensionless file) with a `#!/bin/sh`
        // shebang — the kernel would exec it under a shell. Read the first line
        // of the canonical program file and reject any shebang that names a
        // blocked interpreter (directly or via `#!/usr/bin/env <interp>`).
        if let Some(interpreter) = read_shebang_interpreter(&profile.program) {
            if is_shell_interpreter(&interpreter) {
                return Err(CommandError::new(
                    E_POLICY,
                    format!(
                        "command profile `{}` runs under a shell interpreter (`#!{interpreter}`), which is not allowed",
                        profile.program.display()
                    ),
                ));
            }
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
        if name.is_empty()
            || name.contains('=')
            || name.contains('\0')
            || is_dangerous_environment_name(name)
        {
            return Err(CommandError::new(
                E_ENVIRONMENT,
                format!("invalid command environment variable `{name}`"),
            ));
        }
    }
    Ok(())
}

fn is_dangerous_environment_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    // F12: cover the full set of interpreter/library injection env vars and
    // treat LD_*/DYLD_* as prefixes (e.g. LD_PRELOAD_ALT, DYLD_FALLBACK_*).
    upper.starts_with("LD_")
        || upper.starts_with("DYLD_")
        || upper.starts_with("PYTHONPATH")
        || upper.starts_with("PYTHON")
        || matches!(
            upper.as_str(),
            "NODE_OPTIONS"
                | "NODE_PATH"
                | "RUBYOPT"
                | "RUBYLIB"
                | "PERL5OPT"
                | "PERL5LIB"
                | "PERLLIB"
                | "BASH_ENV"
                | "ENV"
                | "JAVA_TOOL_OPTIONS"
                | "_JAVA_OPTIONS"
                | "JAVA_OPTIONS"
                | "CLASSPATH"
                | "GCONV_PATH"
                | "LOCPATH"
                | "GLIBC_TUNABLES"
        )
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

/// Read the interpreter from a program file's `#!shebang` line, if any.
///
/// Handles direct (`#!/bin/sh`), `/usr/bin/env` (`#!/usr/bin/env bash`), and
/// `/usr/bin/env -S` (`#!/usr/bin/env -S bash`) forms. For the `env` forms, it
/// scans past option flags (tokens starting with `-`, e.g. `-S`, `-i`, `-C`) to
/// find the actual interpreter token, so `#!/usr/bin/env -S bash` resolves to
/// `bash` rather than `-S`. Returns `None` if the file cannot be read or has no
/// shebang. Basename matching is done by the caller via [`is_shell_interpreter`].
/// See security findings F8 and F8-V1.
#[cfg(unix)]
fn read_shebang_interpreter(program: &std::path::Path) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(program).ok()?;
    let mut buf = [0u8; 128];
    let n = file.read(&mut buf).ok()?;
    let first_line = buf[..n].split(|&b| b == b'\n').next()?;
    let line = std::str::from_utf8(first_line).ok()?.trim_start();
    let after_bang = line.strip_prefix("#!")?;
    let mut parts = after_bang.split_whitespace();
    let interpreter = parts.next()?;
    let is_env = std::path::Path::new(interpreter)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "env")
        .unwrap_or(false);
    if is_env {
        // `/usr/bin/env [-flags] <interp> [interp-args]`: skip option flags
        // (tokens starting with `-`, such as `-S`/`-i`/`-C`/`-v`) to reach the
        // real interpreter. Without this, `#!/usr/bin/env -S bash` would yield
        // `-S` (which is not in the denylist) and the script would be allowed
        // even though the kernel runs bash. See finding F8-V1.
        for real in parts {
            if real.starts_with('-') {
                continue;
            }
            return Some(real.to_string());
        }
        // `env` with only flags and no interpreter: nothing to match.
        return None;
    }
    Some(interpreter.to_string())
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

    /// F8 regression: a profile whose program is a script with a `#!/bin/sh`
    /// shebang must be rejected (the basename-only check otherwise misses it
    /// and the kernel would exec it under a shell).
    #[cfg(unix)]
    #[test]
    fn shebang_shell_interpreter_is_rejected() {
        let dir = std::env::temp_dir();
        let script_path = dir.join("mdok_f8_shebang_probe.sh");
        std::fs::write(&script_path, "#!/bin/sh\necho hi\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let canonical = std::fs::canonicalize(&script_path).unwrap();
        let policy = CommandPolicy {
            profiles: [(
                "shscript".to_string(),
                CommandProfile {
                    program: canonical,
                    ..CommandProfile::default()
                },
            )]
            .into_iter()
            .collect(),
            ..CommandPolicy::default()
        };
        let argv = vec!["shscript".to_string()];
        let result = validate_argv(&argv, &policy);
        let err = result.expect_err("shebang shell should be rejected");
        assert_eq!(err.code, E_POLICY);
        let _ = std::fs::remove_file(&script_path);
    }

    /// F8-V1 regression: a shebang of the form `#!/usr/bin/env -S bash` must
    /// also be rejected. The earlier F8 fix took the token after `/usr/bin/env`
    /// verbatim, which yielded `-S` (an option flag, not in the denylist) and
    /// allowed the script even though the kernel runs bash. The fix scans past
    /// option flags to find the real interpreter.
    #[cfg(unix)]
    #[test]
    fn shebang_env_s_shell_interpreter_is_rejected() {
        let dir = std::env::temp_dir();
        for shebang in [
            "#!/usr/bin/env -S bash\necho hi\n",
            "#!/usr/bin/env -S sh\necho hi\n",
            "#!/usr/bin/env bash\necho hi\n",
        ] {
            let script_path = dir.join("mdok_f8v1_probe.sh");
            std::fs::write(&script_path, shebang).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
            let canonical = std::fs::canonicalize(&script_path).unwrap();
            let policy = CommandPolicy {
                profiles: [(
                    "shscript".to_string(),
                    CommandProfile {
                        program: canonical,
                        ..CommandProfile::default()
                    },
                )]
                .into_iter()
                .collect(),
                ..CommandPolicy::default()
            };
            let argv = vec!["shscript".to_string()];
            let result = validate_argv(&argv, &policy);
            let err = result.expect_err(&format!("shebang `{shebang}` should be rejected"));
            assert_eq!(err.code, E_POLICY);
            let _ = std::fs::remove_file(&script_path);
        }
    }

    /// F8-V1 sanity: a shebang pointing at a non-shell interpreter must still be
    /// allowed (the fix must not over-block legitimate scripts).
    #[cfg(unix)]
    #[test]
    fn shebang_non_shell_interpreter_is_allowed() {
        let dir = std::env::temp_dir();
        let script_path = dir.join("mdok_f8v1_ok_probe.sh");
        // `cat` is not a shell interpreter and is harmless; use it as the stand-in.
        std::fs::write(&script_path, "#!/bin/cat\nhello\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let canonical = std::fs::canonicalize(&script_path).unwrap();
        let policy = CommandPolicy {
            profiles: [(
                "catscript".to_string(),
                CommandProfile {
                    program: canonical,
                    ..CommandProfile::default()
                },
            )]
            .into_iter()
            .collect(),
            ..CommandPolicy::default()
        };
        let argv = vec!["catscript".to_string()];
        validate_argv(&argv, &policy).expect("non-shell shebang should be allowed");
        let _ = std::fs::remove_file(&script_path);
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

    #[test]
    fn checks_deadline_before_accepting_a_completed_leader() {
        let mut command = policy();
        command.timeout = Duration::from_nanos(1);
        let error = run(&["fixture".into(), "--list".into()], &command).unwrap_err();
        assert_eq!(error.code, E_TIMEOUT);
    }

    #[test]
    fn rejects_dangerous_profile_environment() {
        let mut command = policy();
        command
            .profiles
            .get_mut("fixture")
            .expect("fixture profile")
            .env
            .insert("LD_PRELOAD".into(), OsString::from("unsafe"));
        assert_eq!(
            run(&["fixture".into()], &command).unwrap_err().code,
            E_ENVIRONMENT
        );
    }
}
