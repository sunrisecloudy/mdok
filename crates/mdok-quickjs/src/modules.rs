//! Pinned `require()` module registry.
//!
//! Only pure-JS modules that run on QuickJS are vendored here. `lodash`
//! 4.17.21 is checked in as a UMD bundle
//! (`src/modules/lodash.js`) and evaluated with a `module`/`exports` shim.
//! Anything else is refused with `MDOK-PM-REQUIRE` — the registry never
//! fetches code from the network at run time.

use sha2::{Digest, Sha256};

/// The vendored lodash bundle, pinned to 4.17.21.
const LODASH_SOURCE: &str = include_str!("modules/lodash.js");
/// Vendored moment 2.29.4 UMD bundle.
const MOMENT_SOURCE: &str = include_str!("modules/moment.js");
/// Vendored ajv 6.12.6 browser bundle (self-contained UMD).
const AJV_SOURCE: &str = include_str!("modules/ajv.js");
/// uuid capability shim (callable v4 + v1/v3/v4/v5/NIL properties).
const UUID_SOURCE: &str = include_str!("modules/uuid.js");
/// querystring capability shim (Node-compatible parse/stringify subset).
const QUERYSTRING_SOURCE: &str = include_str!("modules/querystring.js");
/// Vendored crypto-js 4.2.0 single-file bundle (UMD).
const CRYPTO_JS_SOURCE: &str = include_str!("modules/crypto-js.js");

/// Registered module names (in declaration order).
const MODULES: &[&str] = &["lodash", "moment", "ajv", "uuid", "querystring", "crypto-js"];

/// Return the source of a pinned module, or `None` when unknown.
pub fn module_source(name: &str) -> Option<&'static str> {
    match name {
        "lodash" => Some(LODASH_SOURCE),
        "moment" => Some(MOMENT_SOURCE),
        "ajv" => Some(AJV_SOURCE),
        "uuid" => Some(UUID_SOURCE),
        "querystring" => Some(QUERYSTRING_SOURCE),
        "crypto-js" => Some(CRYPTO_JS_SOURCE),
        _ => None,
    }
}

/// All registered module names.
pub fn module_names() -> Vec<String> {
    MODULES.iter().map(|s| s.to_string()).collect()
}

/// SHA-256 of a module's vendored source (content-addressing for the pinned
/// registry). `None` for unknown modules.
pub fn module_sha256(name: &str) -> Option<String> {
    module_source(name).map(|src| {
        let digest = Sha256::digest(src.as_bytes());
        let mut out = String::with_capacity(64);
        for b in digest {
            out.push_str(&format!("{b:02x}"));
        }
        out
    })
}

#[cfg(test)]
mod tests {
    /// Hard 30-second watchdog for every test (see tests/integration.rs).
    fn run_bounded<F>(f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let _ = tx.send(catch_unwind(AssertUnwindSafe(f)));
        });
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => {}
            Ok(Err(payload)) => std::panic::resume_unwind(payload),
            Err(_) => {
                drop(handle);
                panic!("test exceeded the 30s hard timeout");
            }
        }
    }

    use super::*;

    #[test]
    fn lodash_is_pinned_and_addressable() {
        run_bounded(|| {
            let digest = module_sha256("lodash").expect("lodash vendored");
            assert_eq!(digest.len(), 64);
            assert!(module_source("lodash").unwrap().contains("Lodash"));
            assert_eq!(module_source("nope"), None);
        });
    }

    #[test]
    fn vendored_modules_are_addressable() {
        run_bounded(|| {
            for name in ["lodash", "moment", "ajv", "uuid", "querystring", "crypto-js"] {
                let digest = module_sha256(name).expect("module vendored");
                assert_eq!(digest.len(), 64);
                assert!(!module_source(name).unwrap().is_empty());
            }
        });
    }
}
