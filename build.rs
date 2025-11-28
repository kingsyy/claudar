use std::process::Command;

fn main() {
    // On macOS, link required frameworks for notifications
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }

    // Capture git commit hash at build time
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

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    // Rerun if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
}
