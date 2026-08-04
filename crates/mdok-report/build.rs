use std::{fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("mdok-report must remain in the workspace crates directory");
    let version_path = root.join("vendor/curl.version");
    let version = fs::read_to_string(&version_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", version_path.display()))
        .trim()
        .to_owned();
    let mut parts = version.split('.');
    let major = parts.next().unwrap_or_default();
    let minor = parts.next().unwrap_or_default();
    assert!(
        !major.is_empty() && !minor.is_empty(),
        "invalid curl version: {version}"
    );
    println!("cargo:rustc-env=MDOK_CURL_SOURCE_VERSION={version}");
    println!("cargo:rustc-env=MDOK_CURL_COMPAT_VERSION={major}.{minor}");
    println!("cargo:rerun-if-changed={}", version_path.display());
}
