use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let (mut command, resource) = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let resource = output.join("scoretab.res");
        let mut command = Command::new("rc.exe");
        command
            .args(["/nologo", "/fo"])
            .arg(&resource)
            .arg("icon.rc");
        (command, resource)
    } else {
        let resource = output.join("scoretab.o");
        let mut command = Command::new(env::var_os("WINDRES").unwrap_or_else(|| "windres".into()));
        command
            .args(["-i", "icon.rc", "-O", "coff", "-o"])
            .arg(&resource);
        (command, resource)
    };
    let status = command
        .current_dir("assets")
        .status()
        .expect("Windows resource compiler");
    assert!(status.success(), "could not embed the ScoreTab icon");
    println!(
        "cargo:rustc-link-arg-bin=scoretab-cs2={}",
        resource.display()
    );
}
