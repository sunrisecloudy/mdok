fn main() {
    let dst = cmake::Config::new("../../native").build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=mdok_curl_bridge");
    println!("cargo:rerun-if-changed=../../native");
    println!("cargo:rerun-if-changed=../../vendor/curl.version");
}
