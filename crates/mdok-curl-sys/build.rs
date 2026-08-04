fn main() {
    let dst = cmake::Config::new("../../native").build();
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=mdok_curl_bridge");
    println!("cargo:rustc-link-lib=static=curl");
    let link_file = dst.join("mdok-curl-link-files.txt");
    if let Ok(contents) = std::fs::read_to_string(&link_file) {
        for raw in contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if let Some(framework) = raw.strip_prefix("-framework ") {
                println!("cargo:rustc-link-lib=framework={framework}");
                continue;
            }
            if let Some(path) = raw.strip_prefix("-l") {
                println!("cargo:rustc-link-lib={path}");
                continue;
            }
            if raw.contains("::") {
                continue;
            }
            let path = std::path::Path::new(raw);
            if path.is_absolute() {
                if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name()) {
                    println!("cargo:rustc-link-search=native={}", parent.display());
                    let name = file_name.to_string_lossy();
                    let name = name.strip_prefix("lib").unwrap_or(&name);
                    let name = name.split_once('.').map(|(name, _)| name).unwrap_or(name);
                    println!("cargo:rustc-link-lib=dylib={name}");
                }
            } else {
                println!("cargo:rustc-link-lib={raw}");
            }
        }
    }
    println!("cargo:rerun-if-changed=../../native");
    println!("cargo:rerun-if-changed=../../vendor/curl");
    println!("cargo:rerun-if-changed=../../vendor/curl.version");
}
