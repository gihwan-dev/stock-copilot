use std::io::{ErrorKind, Read, Write};
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
