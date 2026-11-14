fn main() {
    println!("cargo:rerun-if-env-changed=LAZYPROJECT_BUILD_SHA");
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let full_version = match std::env::var("LAZYPROJECT_BUILD_SHA") {
        Ok(sha) if !sha.trim().is_empty() => format!("{pkg_version}+{}", sha.trim()),
        _ => pkg_version,
    };
    println!("cargo:rustc-env=LAZYPROJECT_VERSION={full_version}");
}
