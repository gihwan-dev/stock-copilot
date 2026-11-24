use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::thread;

use zeroize::Zeroizing;

pub const TOKEN_ENV_NAME: &str = "STOCK_MCP_TOKEN";
pub const SOCKET_FLAG: &str = "--socket";
pub const TOKEN_HEX_BYTES: usize = 64_usize;
pub const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103_usize;

const AUTH_PREFIX: &[u8] = b"AUTH ";
const CHUNK_SIZE: usize = 8192_usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    InvalidArguments,
    InvalidSocketPath,
    SocketNotFound,
    SocketSymlinkRejected,
    SocketNotUnixDomain,
    MissingToken,
    InvalidToken,
    ConnectFailed,
    AuthWriteFailed,
    ServerClosedBeforeResponse,
    StreamIoFailed,
}

impl BridgeError {
    #[must_use]
    pub const fn fixed_message(self) -> &'static str {
        match self {
            Self::InvalidArguments => "expected exact arguments: --socket <absolute-path>",
            Self::InvalidSocketPath => "invalid unix socket path",
            Self::SocketNotFound => "unix socket path does not exist",
            Self::SocketSymlinkRejected => "unix socket path must not be a symlink",
            Self::SocketNotUnixDomain => "target path is not a unix domain socket",
            Self::MissingToken => "required auth token environment variable is missing",
            Self::InvalidToken => "auth token format is invalid",
            Self::ConnectFailed => "failed to connect to unix domain socket",
            Self::AuthWriteFailed => "failed to send auth handshake line",
            Self::ServerClosedBeforeResponse => "server closed connection before sending any data",
            Self::StreamIoFailed => "bridge stream io failed",
        }
    }

    #[must_use]
    pub fn exit_code(self) -> ExitCode {
        let code = match self {
            Self::InvalidArguments => 2_u8,
            Self::InvalidSocketPath => 3_u8,
            Self::SocketNotFound => 4_u8,
            Self::SocketSymlinkRejected => 5_u8,
            Self::SocketNotUnixDomain => 6_u8,
            Self::MissingToken => 7_u8,
            Self::InvalidToken => 8_u8,
            Self::ConnectFailed => 9_u8,
            Self::AuthWriteFailed => 10_u8,
            Self::ServerClosedBeforeResponse => 11_u8,
            Self::StreamIoFailed => 12_u8,
        };
        ExitCode::from(code)
    }
}

pub struct BridgeToken(Zeroizing<Vec<u8>>);

impl fmt::Debug for BridgeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BridgeToken(<redacted>)")
    }
}

impl BridgeToken {
    pub fn try_new(raw: String) -> Result<Self, BridgeError> {
        let bytes = Zeroizing::new(raw.into_bytes());
        if bytes.len() != TOKEN_HEX_BYTES {
            return Err(BridgeError::InvalidToken);
        }
        if !bytes.iter().all(|byte| matches_lowercase_hex(*byte)) {
            return Err(BridgeError::InvalidToken);
        }
        Ok(Self(bytes))
    }

    fn build_auth_line(&self) -> Zeroizing<Vec<u8>> {
        let mut line = Zeroizing::new(Vec::with_capacity(
            AUTH_PREFIX
                .len()
                .saturating_add(TOKEN_HEX_BYTES)
                .saturating_add(1_usize),
        ));
        line.extend_from_slice(AUTH_PREFIX);
        line.extend_from_slice(self.0.as_slice());
        line.push(b'\n');
        line
    }
}

const fn matches_lowercase_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (byte >= b'a' && byte <= b'f')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketTarget(PathBuf);

impl SocketTarget {
    pub fn try_new(raw: OsString) -> Result<Self, BridgeError> {
        let path = PathBuf::from(raw);
        let encoded = path.as_os_str().as_encoded_bytes();
        if encoded.is_empty() || encoded.len() > MAX_UNIX_SOCKET_PATH_BYTES {
            return Err(BridgeError::InvalidSocketPath);
        }
        if encoded.contains(&0_u8) || !path.is_absolute() || path.parent().is_none() {
            return Err(BridgeError::InvalidSocketPath);
        }
        if encoded
            .split(|&byte| byte == b'/')
            .any(|segment| segment == b"." || segment == b"..")
        {
            return Err(BridgeError::InvalidSocketPath);
        }
        for component in path.components() {
            match component {
                Component::RootDir | Component::Normal(_) => {}
                Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                    return Err(BridgeError::InvalidSocketPath);
                }
            }
        }
        Ok(Self(path))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn verify_existing_socket(&self) -> Result<(), BridgeError> {
        let meta = fs::symlink_metadata(&self.0).map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                BridgeError::SocketNotFound
            } else {
                BridgeError::InvalidSocketPath
            }
        })?;
        if meta.file_type().is_symlink() {
            return Err(BridgeError::SocketSymlinkRejected);
        }
        if !meta.file_type().is_socket() {
            return Err(BridgeError::SocketNotUnixDomain);
        }
        Ok(())
    }
}

pub fn parse_socket_target(args: &[OsString]) -> Result<SocketTarget, BridgeError> {
    let [_program, flag, socket_raw] = args else {
        return Err(BridgeError::InvalidArguments);
    };
    if flag.as_os_str() != SOCKET_FLAG {
        return Err(BridgeError::InvalidArguments);
    }
    SocketTarget::try_new(socket_raw.clone())
}

fn pump_flushed<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<u64> {
    let mut buffer = [0_u8; CHUNK_SIZE];
    let mut total = 0_u64;
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0_usize) => return Ok(total),
            Ok(read_len) => read_len,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        let Some(slice) = buffer.get(..count) else {
            return Err(io::Error::other("read length exceeded buffer size"));
        };
        writer.write_all(slice)?;
        writer.flush()?;
        let chunk_len =
            u64::try_from(count).map_err(|_overflow| io::Error::other("chunk count overflow"))?;
        total = total.saturating_add(chunk_len);
    }
}

fn shutdown_stream(stream: &UnixStream, how: Shutdown) {
    match stream.shutdown(how) {
        Ok(()) | Err(_) => {}
    }
}

pub fn run_bridge_with_io<R, W>(
    socket_target: &SocketTarget,
    token: &BridgeToken,
    mut stdin: R,
    mut stdout: W,
) -> Result<(), BridgeError>
where
    R: Read + Send + 'static,
    W: Write,
{
    socket_target.verify_existing_socket()?;
    let mut writer = UnixStream::connect(socket_target.as_path())
        .map_err(|_error| BridgeError::ConnectFailed)?;
    let auth_line = token.build_auth_line();
    writer
        .write_all(auth_line.as_slice())
        .and_then(|()| writer.flush())
        .map_err(|_error| BridgeError::AuthWriteFailed)?;
    let mut reader = writer
        .try_clone()
        .map_err(|_error| BridgeError::StreamIoFailed)?;
    let shutdown_handle = writer
        .try_clone()
        .map_err(|_error| BridgeError::StreamIoFailed)?;

    let upload_handle = thread::spawn(move || {
        let result = pump_flushed(&mut stdin, &mut writer);
        shutdown_stream(&writer, Shutdown::Write);
        result
    });

    let download_result = pump_flushed(&mut reader, &mut stdout);
    shutdown_stream(&shutdown_handle, Shutdown::Both);

    if upload_handle.is_finished() {
        match upload_handle.join() {
            Ok(Ok(_bytes)) => {}
            Ok(Err(error)) if error.kind() == ErrorKind::BrokenPipe => {}
            Ok(Err(_)) | Err(_) => return Err(BridgeError::StreamIoFailed),
        }
    }

    let received_bytes = download_result.map_err(|_error| BridgeError::StreamIoFailed)?;
    if received_bytes == 0_u64 {
        return Err(BridgeError::ServerClosedBeforeResponse);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::{
        BridgeError, BridgeToken, MAX_UNIX_SOCKET_PATH_BYTES, SOCKET_FLAG, SocketTarget,
        parse_socket_target,
    };

    #[test]
    fn bridge_token_validates_64_lowercase_hex_and_redacts_debug() {
        let valid_raw = "0123456789abcdef".repeat(4_usize);
        let token = BridgeToken::try_new(valid_raw.clone()).expect("유효 토큰");
        let debug_text = format!("{token:?}");
        assert_eq!(debug_text, "BridgeToken(<redacted>)", "Debug 가림 표식");
        assert!(
            !debug_text.contains(&valid_raw),
            "Debug에 원문 토큰이 노출되면 안 된다"
        );
        let line = token.build_auth_line();
        assert_eq!(
            line.as_slice(),
            format!("AUTH {valid_raw}\n").as_bytes(),
            "첫 줄 AUTH 형식 정확 일치"
        );

        let uppercase = "0123456789ABCDEF".repeat(4_usize);
        let with_newline = format!("{}\n", "a".repeat(63_usize));
        for bad in [
            String::new(),
            "a".repeat(63_usize),
            "a".repeat(65_usize),
            uppercase,
            with_newline,
            format!("{}g", "a".repeat(63_usize)),
        ] {
            assert_eq!(
                BridgeToken::try_new(bad).err(),
                Some(BridgeError::InvalidToken),
                "잘못된 토큰 형식 거부"
            );
        }
    }

    #[test]
    fn cli_args_accept_only_exact_socket_flag_and_safe_path() {
        let valid_args = [
            OsString::from("mcp-bridge"),
            OsString::from(SOCKET_FLAG),
            OsString::from("/private/tmp/r1/m.sock"),
        ];
        let parsed = parse_socket_target(&valid_args).expect("유효 인자");
        assert_eq!(
            parsed.as_path(),
            Path::new("/private/tmp/r1/m.sock"),
            "소켓 경로 보존"
        );

        let bad_arg_lists = [
            vec![OsString::from("mcp-bridge")],
            vec![
                OsString::from("mcp-bridge"),
                OsString::from("/private/tmp/r1/m.sock"),
            ],
            vec![
                OsString::from("mcp-bridge"),
                OsString::from("--token"),
                OsString::from("abc"),
            ],
            vec![
                OsString::from("mcp-bridge"),
                OsString::from(SOCKET_FLAG),
                OsString::from("/private/tmp/r1/m.sock"),
                OsString::from("--token"),
            ],
        ];
        for args in bad_arg_lists {
            assert_eq!(
                parse_socket_target(&args),
                Err(BridgeError::InvalidArguments),
                "잘못된 인자 목록 거부"
            );
        }

        let too_long = format!("/{}", "a".repeat(MAX_UNIX_SOCKET_PATH_BYTES));
        for bad_path in [
            "",
            "relative/m.sock",
            "/",
            "/private/tmp/../m.sock",
            "/private/tmp/./m.sock",
            too_long.as_str(),
        ] {
            assert_eq!(
                SocketTarget::try_new(OsString::from(bad_path)),
                Err(BridgeError::InvalidSocketPath),
                "잘못된 소켓 경로 거부: {bad_path}"
            );
        }
    }
}
