//! Runtime setup, budgets, deadlines, eval and the async-bridge driver.
//!
//! Mirrors the Terrane boundary shape
//! (`terrane/rust/crates/terrane-cap-js-runtime/src/sandbox.rs`): a fresh
//! `rquickjs::Runtime` with max stack 512KB, memory limit 64MB, an interrupt
//! handler checking an injected deadline, `Context::full`, first-error
//! capture and post-settle promise-job draining.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rquickjs::{CatchResultExt, CaughtError, Context, Ctx, Error as JsError, Function, Runtime};

use crate::effect::{ChildRequestExecutor, offline_executor};
use crate::pm::{HostState, PRELUDE, install_host};
use crate::transcript::Transcript;
use crate::{Outcome, ProbeInput, ProbeOutput};

/// Run one probe case with a caller-provided child-request executor.
///
/// The executor is pumped synchronously: pending `pm.sendRequest` effects are
/// performed, their Promises resolved/rejected, and QuickJS promise jobs are
/// drained until quiescent or the injected deadline fires (spec section 3/5).
pub fn run_script_with_executor(
    input: &ProbeInput,
    executor: &mut ChildRequestExecutor,
) -> ProbeOutput {
    let started = Instant::now();
    let profile = input.profile.clone();
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return harness_error(format!("runtime setup failed: {e}")),
    };
    rt.set_max_stack_size(profile.max_stack_bytes);
    rt.set_memory_limit(profile.max_memory_bytes);
    let deadline = started + Duration::from_millis(profile.script_timeout_ms.max(1));
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    // NOTE: the Eval intrinsic must remain enabled because host-side loading
    // (prelude, user script, and module source via __mdok_eval_module) all go
    // through Rust `ctx.eval`, which calls JS_Eval -> JS_EvalInternal and
    // throws "eval is not supported" when the intrinsic is absent. F1's
    // exploitable host-side eval sink (__mdok_eval_module) is closed by
    // token-gating it (pm.rs). The prototype-chain constructor bypass
    // (`(function(){}).constructor('...')()`) cannot be defeated from JS in
    // QuickJS (it resolves to an internal native constructor), but it runs only
    // inside the sandbox — no FS/process access, policy-gated network (F4), and
    // post-run redaction of encoded secret forms (F2). It is not a host escape.
    let ctx = match Context::full(&rt) {
        Ok(ctx) => ctx,
        Err(e) => return harness_error(format!("context setup failed: {e}")),
    };

    ctx.with(|ctx| {
        let taint = crate::secrets::taint_from_input(input);
        let state = Rc::new(RefCell::new(HostState::new(input, taint.clone())));
        if let Err(e) = install_host(&ctx, state.clone(), input) {
            return harness_error(format!("facade installation failed: {e}"));
        }
        // Facade prelude (our code; failure is a harness error).
        if let Err(e) = ctx.eval::<(), _>(PRELUDE.as_bytes()).catch(&ctx) {
            return harness_error(format!("facade prelude failed: {}", caught_message(&e)));
        }
        // User script.
        let script_result: rquickjs::Result<()> = ctx.eval::<(), _>(input.script.as_bytes());
        if let Err(e) = script_result.catch(&ctx) {
            record_script_error(&state, &e);
        }
        // Async bridge: process child requests, drain promise jobs, and pump
        // timers until quiescent or the injected deadline.
        let drain_timers: rquickjs::Result<Function> = ctx.globals().get("__mdok_drain_timers");
        loop {
            if Instant::now() >= deadline {
                state.borrow_mut().timed_out = true;
                break;
            }
            let batch: Vec<_> = state.borrow_mut().pending.drain(..).collect();
            if !batch.is_empty() {
                let mut interrupted = false;
                for pending in batch {
                    // Late completion from an older generation is ignored.
                    if pending.request.generation != state.borrow().generation {
                        continue;
                    }
                    let result = executor(&pending.request);
                    if !result.ok {
                        let mut state = state.borrow_mut();
                        if !state.offline_diagnostic_emitted
                            && result
                                .error
                                .as_deref()
                                .map(|e| e.contains("MDOK-PM-NETWORK-OFFLINE"))
                                .unwrap_or(false)
                        {
                            state.offline_diagnostic_emitted = true;
                            state.push_diagnostic(
                                "MDOK-PM-NETWORK-OFFLINE",
                                "pm.sendRequest".to_string(),
                                "pm.sendRequest is disabled in offline mode; use --network fetch to enable child requests".to_string(),
                            );
                        }
                    }
                    state.borrow_mut().record_child(&pending.request, &result);
                    if result.ok {
                        let json = crate::pm::child_response_json(&result);
                        if let Err(e) = pending.resolve.call::<_, ()>((json,)).catch(&ctx) {
                            record_script_error(&state, &e);
                        }
                    } else {
                        let message = result
                            .error
                            .clone()
                            .unwrap_or_else(|| "child request failed".to_string());
                        if let Err(e) = pending.reject.call::<_, ()>((message,)).catch(&ctx) {
                            record_script_error(&state, &e);
                        }
                    }
                    if drain_jobs(&ctx, &state, deadline) {
                        interrupted = true;
                        break;
                    }
                    if state.borrow().timed_out {
                        interrupted = true;
                        break;
                    }
                }
                if interrupted {
                    break;
                }
                continue;
            }
            // No pending child requests: drain any remaining jobs.
            if drain_jobs(&ctx, &state, deadline) {
                break;
            }
            if state.borrow().timed_out {
                break;
            }
            // Timers: fire due callbacks and learn how long until the next one
            // (-1 when the queue is empty). Timer callbacks may enqueue child
            // requests or promise jobs, so the pump continues either way.
            let next_timer_ms: i64 = match &drain_timers {
                Ok(fire) => match fire.call::<_, i64>(()) {
                    Ok(ms) => ms,
                    Err(e) => {
                        record_script_error(&state, &CaughtError::Error(e));
                        break;
                    }
                },
                Err(_) => -1,
            };
            if next_timer_ms < 0 {
                // No timers left, but timer callbacks may have enqueued child
                // requests (pm.sendRequest) or promise jobs; only stop when
                // the queue is truly quiescent.
                if !state.borrow().pending.is_empty() {
                    continue;
                }
                if drain_jobs(&ctx, &state, deadline) {
                    break;
                }
                if state.borrow().timed_out {
                    break;
                }
                if state.borrow().pending.is_empty() {
                    break;
                }
                continue;
            }
            let wait = Duration::from_millis(next_timer_ms as u64)
                .min(deadline.saturating_duration_since(Instant::now()));
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
        }

        let mut state = state.borrow_mut();
        let timed_out = state.timed_out;
        if timed_out {
            state.push_diagnostic(
                "MDOK-PM-TIMEOUT",
                String::new(),
                format!(
                    "script exceeded the {}ms timeout budget",
                    profile.script_timeout_ms
                ),
            );
        }
        let outcome = if timed_out {
            Outcome::Timeout
        } else if !state.errors.is_empty() {
            Outcome::Error
        } else if state.tests.iter().any(|t| !t.passed) {
            Outcome::Failed
        } else {
            Outcome::Passed
        };
        let used_api = state.fold_used_api();
        let transcript = state.fold_transcript();
        let diagnostics = state.diagnostics();
        drop(state);

        let mut output = ProbeOutput {
            ok: true,
            outcome,
            duration_ms: started.elapsed().as_millis() as u64,
            used_api,
            diagnostics,
            transcript,
        };
        // Belt-and-suspenders: sweep the whole output for tainted strings.
        redact_output_json(&mut output, &taint);
        // Transcript strings truncated at the profile cap.
        truncate_output(&mut output, profile.max_transcript_bytes);
        output
    })
}

/// Run one probe case with the offline executor.
pub fn run_script(input: &ProbeInput) -> ProbeOutput {
    let mut executor = offline_executor;
    run_script_with_executor(input, &mut executor)
}

fn record_script_error<'js>(state: &Rc<RefCell<HostState<'js>>>, e: &CaughtError) {
    if is_allocation_error(e) {
        let mut state = state.borrow_mut();
        state.push_diagnostic(
            "MDOK-PM-LIMIT",
            String::new(),
            "script exceeded the sandbox memory limit".to_string(),
        );
        let (msg, _) = state.redact_owned(&caught_message(e));
        state.errors.push(msg);
    } else if is_interrupted(e) {
        state.borrow_mut().timed_out = true;
    } else {
        let mut state = state.borrow_mut();
        let (msg, _) = state.redact_owned(&caught_message(e));
        state.errors.push(msg);
    }
}

/// Drain pending jobs, catching job exceptions and interrupts. Returns true
/// when execution stopped early (timeout or job exception recorded).
fn drain_jobs<'js>(ctx: &Ctx<'js>, state: &Rc<RefCell<HostState<'js>>>, deadline: Instant) -> bool {
    loop {
        if Instant::now() >= deadline {
            state.borrow_mut().timed_out = true;
            return true;
        }
        if !ctx.execute_pending_job() {
            return false;
        }
        let pending_exception = ctx.catch();
        if pending_exception.is_exception() || pending_exception.is_uncatchable_error() {
            let message = if let Some(s) = pending_exception
                .as_string()
                .and_then(|s| s.to_string().ok())
            {
                s
            } else {
                format!("{pending_exception:?}")
            };
            let mut state = state.borrow_mut();
            if pending_exception.is_uncatchable_error()
                || message.to_ascii_lowercase().contains("interrupted")
            {
                state.timed_out = true;
            } else {
                let (message, _) = state.redact_owned(&message);
                state.errors.push(message);
            }
            return true;
        }
    }
}

/// Extract the JS exception message (redacted later by the caller).
fn caught_message(e: &CaughtError) -> String {
    match e {
        CaughtError::Exception(ex) => ex
            .message()
            .unwrap_or_else(|| "JavaScript exception".into()),
        CaughtError::Value(v) => {
            if let Some(s) = v.as_string().and_then(|s| s.to_string().ok()) {
                s
            } else {
                format!("exception generated by quickjs: {v:?}")
            }
        }
        CaughtError::Error(err) => err.to_string(),
    }
}

/// True when a caught error was produced by the interrupt handler.
fn is_interrupted(e: &CaughtError) -> bool {
    match e {
        CaughtError::Exception(ex) => ex
            .message()
            .map(|m| m.to_ascii_lowercase().contains("interrupted"))
            .unwrap_or(false),
        CaughtError::Value(v) => v.is_uncatchable_error(),
        CaughtError::Error(_) => false,
    }
}

fn is_allocation_error(e: &CaughtError) -> bool {
    matches!(e, CaughtError::Error(JsError::Allocation))
}

fn harness_error(message: String) -> ProbeOutput {
    ProbeOutput {
        ok: false,
        outcome: Outcome::Error,
        duration_ms: 0,
        used_api: Vec::new(),
        diagnostics: Vec::new(),
        transcript: Transcript {
            errors: vec![message],
            ..Transcript::default()
        },
    }
}

/// Post-run taint sweep: every string in the output JSON is redacted against
/// the full taint set.
fn redact_output_json(output: &mut ProbeOutput, taint: &[String]) {
    let mut value = serde_json::to_value(&*output).unwrap_or(serde_json::Value::Null);
    redact_value(&mut value, taint);
    if let Ok(patched) = serde_json::from_value::<ProbeOutput>(value) {
        *output = patched;
    }
}

fn redact_value(value: &mut serde_json::Value, taint: &[String]) {
    match value {
        serde_json::Value::String(s) => {
            let (redacted_s, _) = crate::secrets::redact_with(taint, s);
            *s = redacted_s;
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_value(item, taint);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values_mut() {
                redact_value(item, taint);
            }
        }
        _ => {}
    }
}

/// Truncate every transcript string at the profile cap.
fn truncate_output(output: &mut ProbeOutput, max: usize) {
    let mut value = serde_json::to_value(&*output).unwrap_or(serde_json::Value::Null);
    truncate_value(&mut value, max);
    if let Ok(patched) = serde_json::from_value::<ProbeOutput>(value) {
        *output = patched;
    }
}

/// Truncate a string to a byte limit, landing on a UTF-8 char boundary.
///
/// `String::truncate` panics when the limit falls inside a multi-byte sequence;
/// attacker-controlled JS/JSON text (e.g. `console.log('日'.repeat(20000))`)
/// can force a non-char-boundary cut and, under `panic = "abort"`, abort the
/// long-lived MCP server — a one-line DoS. This helper walks back to the
/// nearest char boundary so truncation never panics. See finding F10.
pub(crate) fn truncate_bytes(s: &mut String, max: usize) {
    if s.len() <= max {
        return;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
}

fn truncate_value(value: &mut serde_json::Value, max: usize) {
    match value {
        serde_json::Value::String(s) => {
            truncate_bytes(s, max);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                truncate_value(item, max);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values_mut() {
                truncate_value(item, max);
            }
        }
        _ => {}
    }
}
