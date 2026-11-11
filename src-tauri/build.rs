fn main() {
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=src/mac_audio.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        use std::{path::PathBuf, process::Command};
        let output = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let sdk = Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]).output().expect("Xcode Command Line Tools richiesti");
        assert!(sdk.status.success(), "Impossibile trovare macOS SDK");
        let sdk = String::from_utf8(sdk.stdout).unwrap();
        let arch = if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") { "arm64" } else { "x86_64" };
        let status = Command::new("xcrun").args(["--sdk", "macosx", "clang", "-arch", arch, "-fobjc-arc", "-fblocks", "-mmacosx-version-min=13.0", "-isysroot", sdk.trim(), "-c", "src/mac_audio.m", "-o"])
            .arg(output.join("mac_audio.o")).status().expect("Compilazione audio Mac");
        assert!(status.success(), "Compilazione cattura Mac fallita");
        assert!(Command::new("ar").arg("crs").arg(output.join("libparlo_audio.a")).arg(output.join("mac_audio.o")).status().unwrap().success());
        println!("cargo:rustc-link-search=native={}", output.display());
        println!("cargo:rustc-link-lib=static=parlo_audio");
        for framework in ["Foundation", "AppKit", "AVFoundation", "ScreenCaptureKit", "CoreMedia", "CoreAudio", "AudioToolbox", "CoreGraphics", "UserNotifications"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    tauri_build::build()
}
