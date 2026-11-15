use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug)]
pub struct LocalHttpServer {
    port: u16,
    task: JoinHandle<io::Result<Vec<u8>>>,
}

fn read_request(stream: &mut std::net::TcpStream) -> io::Result<Vec<u8>> {
    let mut request = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(request);
        }
        request.extend_from_slice(buffer.get(..count).ok_or(io::ErrorKind::InvalidData)?);
        if let Some(index) = String::from_utf8_lossy(&request).find("\r\n\r\n") {
            let header =
                String::from_utf8_lossy(request.get(..index).ok_or(io::ErrorKind::InvalidData)?);
            let length = header
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                .map_or(Ok(0), |(_, value)| {
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_error| io::Error::from(io::ErrorKind::InvalidData))
                })?;
            if request.len() >= index.saturating_add(4).saturating_add(length) {
                return Ok(request);
            }
        }
    }
}

impl LocalHttpServer {
    pub fn start(response: Vec<u8>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let task = thread::spawn(move || {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let request = read_request(&mut stream)?;
            stream.write_all(&response)?;
            Ok(request)
        });
        Ok(Self { port, task })
    }
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }
    pub fn finish(self) -> io::Result<Vec<u8>> {
        self.task
            .join()
            .map_err(|_error| io::Error::from(io::ErrorKind::Other))?
    }
}

pub fn proxy_probe(executable: &std::path::Path, test: &str) -> io::Result<std::process::Output> {
    std::process::Command::new(executable)
        .args(["--exact", test])
        .env("HTTP_PROXY", "http://192.0.2.1:9")
        .env("HTTPS_PROXY", "http://192.0.2.2:9")
        .env("ALL_PROXY", "http://192.0.2.3:9")
        .env("http_proxy", "http://192.0.2.4:9")
        .env("https_proxy", "http://192.0.2.5:9")
        .env("all_proxy", "http://192.0.2.6:9")
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .output()
}

pub fn check_consumer(
    manifest: &std::path::Path,
    target: &std::path::Path,
) -> io::Result<std::process::Output> {
    std::process::Command::new("cargo")
        .args(["check", "--offline", "--quiet", "--manifest-path"])
        .arg(manifest)
        .arg("--target-dir")
        .arg(target)
        .output()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionObservation {
    NoConnection,
    Connected,
}

#[derive(Debug)]
pub struct ConnectionProbe(TcpListener);
impl ConnectionProbe {
    pub fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        Ok(Self(listener))
    }
    pub fn port(&self) -> io::Result<u16> {
        Ok(self.0.local_addr()?.port())
    }
    pub fn observe(&self) -> io::Result<ConnectionObservation> {
        match self.0.accept() {
            Ok((_stream, _address)) => Ok(ConnectionObservation::Connected),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Ok(ConnectionObservation::NoConnection)
            }
            Err(error) => Err(error),
        }
    }
}
