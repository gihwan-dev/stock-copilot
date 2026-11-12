use std::process::{Command, Output};

use crate::source_check::cargo_config::{
    SANDBOX_PROFILE, SANDBOX_SCRIPT, SANDBOX_SHELL, SANDBOX_WRAPPER,
};

const OTHER_PROFILE_SUFFIX: &str =
    "(deny file-write* (literal \"/private/var/empty/stock-copilot-nested\"))";

fn wrapped(profile: &str, program: &[&str], outer_sandbox: Option<&str>) -> Output {
    let mut command = Command::new(SANDBOX_SHELL);
    command
        .args(["-c", SANDBOX_SCRIPT, SANDBOX_WRAPPER, profile])
        .args(program)
        .env_remove("SANDBOX_RUNTIME");
    if let Some(value) = outer_sandbox {
        command.env("SANDBOX_RUNTIME", value);
    }
    command.output().expect("run the network-sandbox wrapper")
}

#[test]
fn wrapper_refuses_to_run_unsandboxed_when_it_cannot_nest() {
    let profile = format!("{SANDBOX_PROFILE}{OTHER_PROFILE_SUFFIX}");
    let refused = wrapped(&profile, &["/bin/echo", "ran"], None);
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert_eq!(refused.status.code(), Some(1_i32), "{stderr}");
    assert!(stderr.contains("network-sandbox:"), "{stderr}");
    assert!(refused.stdout.is_empty(), "the program ran unsandboxed");
    let other = wrapped(&profile, &["/bin/echo", "ran"], Some("0"));
    assert_eq!(
        other.status.code(),
        Some(1_i32),
        "SANDBOX_RUNTIME=0 must not count"
    );
}

#[test]
fn wrapper_leaves_the_program_to_the_outer_sandbox_when_it_cannot_nest() {
    let profile = format!("{SANDBOX_PROFILE}{OTHER_PROFILE_SUFFIX}");
    let inside = wrapped(&profile, &["/bin/echo", "ran"], Some("1"));
    assert!(inside.status.success(), "{inside:?}");
    assert_eq!(String::from_utf8_lossy(&inside.stdout), "ran\n");
}

#[test]
fn wrapped_programs_cannot_reach_the_network_in_either_branch() {
    let probe = ["/usr/bin/nc", "-v", "-z", "-G", "5", "1.1.1.1", "443"];
    let refused_or_blocked = wrapped(SANDBOX_PROFILE, &probe, None);
    let stderr = String::from_utf8_lossy(&refused_or_blocked.stderr);
    assert!(
        !refused_or_blocked.status.success(),
        "nc connected: {stderr}"
    );
    assert!(
        stderr.contains("Operation not permitted") || stderr.contains("network-sandbox:"),
        "nc failed for another reason: {stderr}"
    );
    let inside = wrapped(SANDBOX_PROFILE, &probe, Some("1"));
    let inside_stderr = String::from_utf8_lossy(&inside.stderr);
    assert!(!inside.status.success(), "nc connected: {inside_stderr}");
    assert!(
        inside_stderr.contains("Operation not permitted"),
        "nc failed for another reason: {inside_stderr}"
    );
}
