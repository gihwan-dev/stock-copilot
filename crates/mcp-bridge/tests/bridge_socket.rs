#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs::{self, DirBuilder};
    use std::io::{BufRead, BufReader, Cursor, Write};
    use std::os::unix::fs::{DirBuilderExt, symlink};
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};
    use std::thread;

    use mcp_bridge::bridge::{BridgeError, BridgeToken, SocketTarget, run_bridge_with_io};

    fn ignore_io_result(result: std::io::Result<()>) {
        match result {
            Ok(()) => {}
            Err(_error) => {}
        }
    }

    struct TempSocketDir {
        path: PathBuf,
    }

    impl TempSocketDir {
        fn new(tag: &str) -> Self {
            let root = fs::canonicalize(std::env::temp_dir()).expect("임시 폴더 정규화");
            let pid = std::process::id();
            let path = root.join(format!("mb-{tag}-{pid}"));
            if path.exists() {
                ignore_io_result(fs::remove_dir_all(&path));
            }
            DirBuilder::new()
                .mode(0o700_u32)
                .create(&path)
                .expect("소켓 테스트 폴더");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempSocketDir {
        fn drop(&mut self) {
            if self.path.exists() {
                ignore_io_result(fs::remove_dir_all(&self.path));
            }
        }
    }

    fn sample_token_hex() -> String {
        "a1b2c3d4e5f60718".repeat(4_usize)
    }

    #[test]
    fn bridge_sends_auth_first_line_and_forwards_bidirectional_bytes() {
        let dir = TempSocketDir::new("ok");
        let sock_path = dir.path().join("m.sock");
        let listener = UnixListener::bind(&sock_path).expect("테스트 UDS 리스너");
        let expected_token = sample_token_hex();
        let expected_auth = format!("AUTH {expected_token}\n");

        let server_thread = thread::spawn(move || {
            let (stream, _addr) = listener.accept().expect("접속 수락");
            let mut writer = stream.try_clone().expect("스트림 복제");
            let mut reader = BufReader::new(stream);
            let mut first_line = String::new();
            let first_len = reader.read_line(&mut first_line).expect("인증 줄 읽기");
            assert!(first_len > 0_usize, "인증 줄 수신");
            assert_eq!(first_line, expected_auth, "첫 줄은 정확히 AUTH <token>\\n");

            let mut rpc_line = String::new();
            let rpc_len = reader.read_line(&mut rpc_line).expect("JSON-RPC 줄 읽기");
            assert!(rpc_len > 0_usize, "JSON-RPC 줄 수신");
            assert_eq!(
                rpc_line,
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n"
            );

            writer
                .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n")
                .expect("응답 전송");
            writer.flush().expect("응답 플러시");
        });

        let target = SocketTarget::try_new(OsString::from(&sock_path)).expect("소켓 대상");
        let token = BridgeToken::try_new(sample_token_hex()).expect("토큰");
        let stdin =
            Cursor::new(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n".to_vec());
        let mut stdout = Vec::new();
        run_bridge_with_io(&target, &token, stdin, &mut stdout).expect("브리지 전달 성공");
        server_thread.join().expect("서버 스레드 종료");

        assert_eq!(
            stdout, b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n",
            "서버 응답 바이트가 stdout으로 그대로 전달됐다"
        );
    }

    #[test]
    fn bridge_fails_closed_when_server_rejects_and_closes_without_output() {
        let dir = TempSocketDir::new("rej");
        let sock_path = dir.path().join("m.sock");
        let listener = UnixListener::bind(&sock_path).expect("테스트 리스너");

        let server_thread = thread::spawn(move || {
            let (stream, _addr) = listener.accept().expect("접속 수락");
            let mut reader = BufReader::new(stream);
            let mut auth_line = String::new();
            match reader.read_line(&mut auth_line) {
                Ok(_) | Err(_) => {}
            }
        });

        let target = SocketTarget::try_new(OsString::from(&sock_path)).expect("소켓 대상");
        let token = BridgeToken::try_new(sample_token_hex()).expect("토큰");
        let stdin = Cursor::new(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n".to_vec());
        let mut stdout = Vec::new();
        let outcome = run_bridge_with_io(&target, &token, stdin, &mut stdout);
        server_thread.join().expect("서버 스레드 종료");

        assert_eq!(
            outcome,
            Err(BridgeError::ServerClosedBeforeResponse),
            "서버가 응답 없이 연결을 닫으면 비정상 종료 오류를 돌려준다"
        );
        assert!(stdout.is_empty(), "거부 시 stdout 출력 0바이트");
    }

    #[test]
    fn bridge_rejects_missing_regular_file_and_symlink_socket_targets() {
        let dir = TempSocketDir::new("bad");
        let missing =
            SocketTarget::try_new(OsString::from(dir.path().join("absent.sock"))).expect("경로");
        assert_eq!(
            missing.verify_existing_socket(),
            Err(BridgeError::SocketNotFound),
            "존재하지 않는 소켓 경로 거부"
        );

        let regular_path = dir.path().join("plain.txt");
        fs::write(&regular_path, b"not-a-socket").expect("일반 파일 생성");
        let regular = SocketTarget::try_new(OsString::from(&regular_path)).expect("경로");
        assert_eq!(
            regular.verify_existing_socket(),
            Err(BridgeError::SocketNotUnixDomain),
            "일반 파일 대상 거부"
        );

        let real_sock = dir.path().join("real.sock");
        let _listener = UnixListener::bind(&real_sock).expect("실제 소켓");
        let link_sock = dir.path().join("link.sock");
        symlink(&real_sock, &link_sock).expect("소켓 심볼릭 링크");
        let link_target = SocketTarget::try_new(OsString::from(&link_sock)).expect("경로");
        assert_eq!(
            link_target.verify_existing_socket(),
            Err(BridgeError::SocketSymlinkRejected),
            "심볼릭 링크 소켓 거부"
        );
    }
}
