use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::ToSocketAddrs;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::harness_workspace_tests::{fresh_directory, repository_root};

const EXTERNAL_ADDRESSES: [&str; 3] = [
    "1.1.1.1:443",
    "93.184.215.14:80",
    "[2606:4700:4700::1111]:443",
];

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

const GREETING: &[u8] = b"loopback fake server";

fn serve_once(listener: TcpListener) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept loopback client");
        stream.write_all(GREETING).expect("write greeting");
    })
}

#[test]
fn outbound_connections_fail_with_permission_denied() {
    for text in EXTERNAL_ADDRESSES {
        let address: SocketAddr = text.parse().expect("socket address");
        let outcome = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT);
        assert_eq!(
            outcome.as_ref().map_err(std::io::Error::kind).err(),
            Some(ErrorKind::PermissionDenied),
            "{text}: {outcome:?}"
        );
    }
}

#[test]
fn child_processes_inherit_the_network_block() {
    let output = Command::new("/usr/bin/nc")
        .args(["-v", "-z", "-G", "5", "1.1.1.1", "443"])
        .output()
        .expect("run nc");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "nc connected: {stderr}");
    assert!(
        stderr.contains("Operation not permitted"),
        "nc failed for another reason: {stderr}"
    );
}

#[test]
fn loopback_tcp_fake_server_answers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
    let address = listener.local_addr().expect("listener address");
    let server = serve_once(listener);
    let mut client = TcpStream::connect(address).expect("connect to loopback fake server");
    let mut received = Vec::new();
    client
        .read_to_end(&mut received)
        .expect("read loopback greeting");
    server.join().expect("loopback server thread");
    assert_eq!(received, GREETING);
}

#[test]
fn unix_socket_fake_server_answers() {
    let directory = fresh_directory(&repository_root(), "net-unix");
    let path = directory.join("s");
    let listener = UnixListener::bind(&path).expect("bind unix socket");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept unix client");
        stream.write_all(GREETING).expect("write greeting");
    });
    let mut client = UnixStream::connect(&path).expect("connect to unix fake server");
    let mut received = Vec::new();
    client
        .read_to_end(&mut received)
        .expect("read unix greeting");
    server.join().expect("unix server thread");
    assert_eq!(received, GREETING);
}

const FIXED_NEXTEST_VARIABLES: [(&str, &str); 8] = [
    ("NEXTEST_PROFILE", "default-miri"),
    ("NEXTEST_RETRIES", "5"),
    ("NEXTEST_FLAKY_RESULT", "pass"),
    ("NEXTEST_NO_TESTS", "pass"),
    ("NEXTEST_USER_CONFIG_FILE", "/dev/null"),
    ("NEXTEST_TEST_THREADS", "1"),
    ("CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER", "/usr/bin/env"),
    ("CARGO_BUILD_TARGET", "x86_64-apple-darwin"),
];

#[test]
fn name_resolution_is_blocked_but_localhost_resolves() {
    let external = ("example.com", 443).to_socket_addrs();
    assert!(external.is_err(), "example.com resolved: {external:?}");
    let mut local = ("localhost", 443)
        .to_socket_addrs()
        .expect("resolve localhost");
    assert!(local.next().is_some(), "localhost has no address");
}

#[test]
fn cargo_run_does_not_use_the_sandbox() {
    let root = repository_root();
    let output = Command::new("cargo")
        .current_dir(&root)
        .args(["run", "--locked", "--package", "toss-smoke"])
        .output()
        .expect("cargo run toss-smoke");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let running: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("Running"))
        .collect();
    assert_eq!(running.len(), 1, "{stderr}");
    assert!(
        running.iter().all(|line| !line.contains("sandbox-exec")),
        "{running:?}"
    );
    assert!(
        !root.join(".cargo/config.toml").exists(),
        "cargo runner config is back"
    );
}

#[test]
fn justfile_clears_nextest_and_runner_variables() {
    let root = repository_root();
    let directory = fresh_directory(&root, "nextest-variables");
    let probe: Vec<String> = FIXED_NEXTEST_VARIABLES
        .iter()
        .map(|(name, _)| format!("    printf '%s\\n' \"${{{name}:-unset}}\""))
        .collect();
    let recipe = format!("probe:\n{}\n", probe.join("\n"));
    let fixed = directory.join("fixed.just");
    fs::write(
        &fixed,
        format!("import '{}'\n\n{recipe}", root.join("justfile").display()),
    )
    .expect("write importing justfile");
    let plain = directory.join("plain.just");
    fs::write(&plain, &recipe).expect("write plain justfile");
    let run = |justfile: &std::path::Path| {
        let output = Command::new("just")
            .arg("--justfile")
            .arg(justfile)
            .arg("probe")
            .envs(FIXED_NEXTEST_VARIABLES)
            .output()
            .expect("run just");
        assert!(output.status.success(), "just probe failed");
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    let unset: String = FIXED_NEXTEST_VARIABLES.iter().map(|_| "unset\n").collect();
    let values: Vec<&str> = FIXED_NEXTEST_VARIABLES
        .iter()
        .map(|(_, value)| *value)
        .collect();
    let hostile = format!("{}\n", values.join("\n"));
    assert_eq!(run(&fixed), unset, "a variable reached just recipes");
    assert_eq!(run(&plain), hostile, "probe does not see the environment");
}
