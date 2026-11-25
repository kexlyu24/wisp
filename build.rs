use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/wisp.rc");
    println!("cargo:rerun-if-changed=assets/icons/wisp.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let obj = out_dir.join("wisp-icon.o");
    let rc = PathBuf::from("assets/icons/wisp.rc");

    let candidates = [
        "windres",
        "x86_64-w64-mingw32-windres",
        r"C:\msys64\mingw64\bin\windres.exe",
        r"C:\ProgramData\mingw64\mingw64\bin\windres.exe",
        r"C:\mingw64\bin\windres.exe",
    ];

    for tool in candidates {
        let status = Command::new(tool)
            .args(["-i"])
            .arg(&rc)
            .args(["-o"])
            .arg(&obj)
            .args(["-O", "coff"])
            .status();
        if let Ok(s) = status {
            if s.success() {
                println!("cargo:rustc-link-arg-bin=wisp={}", obj.display());
                return;
            }
        }
    }

    println!("cargo:warning=windres not found; wisp.exe built without embedded .ico resource");
}
