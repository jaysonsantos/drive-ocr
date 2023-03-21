use std::process::Command;

use chrono::Utc;

fn main() {
    // Get the current git commit and branch
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git command");
    let commit = String::from_utf8(output.stdout).unwrap();
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .expect("Failed to execute git command");
    let branch = String::from_utf8(output.stdout).unwrap();
    let now = Utc::now();

    // Set the environment variables for cargo
    println!("cargo:rustc-env=GIT_COMMIT={}", commit.trim());
    println!("cargo:rustc-env=GIT_BRANCH={}", branch.trim());
    println!("cargo:rustc-env=BUILD_TIME={now}");
}
