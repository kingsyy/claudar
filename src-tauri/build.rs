use std::process::Command;

fn main() {
    // Capture the short git commit hash at build time for the About panel.
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string();
    println!("cargo:rustc-env=GIT_HASH={git_hash}");
    println!("cargo:rerun-if-changed=../.git/HEAD");

    tauri_build::build()
}
