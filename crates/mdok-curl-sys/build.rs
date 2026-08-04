fn main() {
    let dst = cmake::Config::new("../../native").build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=mdok_curl_bridge");
    // The bridge is a static archive, so CMake's PRIVATE curl dependency is
    // not carried into rustc's final link step. Keep the dependency explicit
    // here instead of changing the existing C ABI or requiring callers to
    // know how the archive was built.
    println!("cargo:rustc-link-lib=dylib=curl");
    println!("cargo:rerun-if-changed=../../native");
    println!("cargo:rerun-if-changed=../../vendor/curl.version");
}
