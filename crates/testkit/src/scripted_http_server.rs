use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug)]
pub struct ScriptedHttpServer {
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

impl ScriptedHttpServer {
    pub fn start(responses: Vec<Vec<u8>>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let task = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept()?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                requests.extend(read_request(&mut stream)?);
                stream.write_all(&response)?;
            }
            Ok(requests)
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
