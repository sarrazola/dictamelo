fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_macos_compiler_runtime();
    }
    tauri_build::build()
}

/// Native Metal uses availability checks when targeting older macOS releases.
/// Rust links with `-nodefaultlibs`, so supply the Apple compiler runtime that
/// Clang would normally add for `__isPlatformVersionAtLeast` and related helpers.
fn link_macos_compiler_runtime() {
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
    println!("cargo:rerun-if-env-changed=SDKROOT");
    let output = std::process::Command::new("xcrun")
        .args(["--sdk", "macosx", "clang", "-print-file-name=libclang_rt.osx.a"])
        .output()
        .expect("macOS builds require the Apple toolchain selected by xcrun");
    assert!(output.status.success(), "Could not locate the macOS compiler runtime through xcrun");
    let archive = std::path::PathBuf::from(
        String::from_utf8(output.stdout).expect("Apple compiler runtime path must be UTF-8").trim(),
    );
    assert!(
        archive.is_absolute() && archive.is_file(),
        "The selected Apple toolchain does not provide libclang_rt.osx.a; check Xcode / Command Line Tools installation"
    );
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rustc-link-search=native={}", archive.parent().expect("compiler runtime directory").display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
