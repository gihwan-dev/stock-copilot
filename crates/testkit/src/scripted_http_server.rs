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

fn serve_holding(
    listener: &TcpListener,
    responses: Vec<Vec<u8>>,
    hold: HoldPosition,
    seen: &std::sync::mpsc::Sender<()>,
    released: &std::sync::mpsc::Receiver<()>,
) -> io::Result<Vec<u8>> {
    let mut requests = Vec::new();
    let mut deferred = None;
    for (position, response) in responses.into_iter().enumerate() {
        let (mut stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        requests.extend(read_request(&mut stream)?);
        if position == hold.0 {
            deferred = Some((stream, response));
            seen.send(())
                .map_err(|_error| io::Error::from(io::ErrorKind::BrokenPipe))?;
        } else {
            stream.write_all(&response)?;
        }
    }
    released
        .recv()
        .map_err(|_error| io::Error::from(io::ErrorKind::BrokenPipe))?;
    if let Some((mut stream, response)) = deferred {
        stream.write_all(&response)?;
    }
    Ok(requests)
}

impl ScriptedHttpServer {
    pub fn start_holding(
        responses: Vec<Vec<u8>>,
        hold: HoldPosition,
    ) -> io::Result<(Self, ResponseGate)> {
        if hold.0 >= responses.len() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let (seen, observed) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel();
        let task =
            thread::spawn(move || serve_holding(&listener, responses, hold, &seen, &released));
        Ok((Self { port, task }, ResponseGate { observed, release }))
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldPosition(usize);
impl HoldPosition {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateObservation {
    Waiting,
    Held,
    Closed,
}
#[derive(Debug)]
pub struct ResponseGate {
    observed: std::sync::mpsc::Receiver<()>,
    release: std::sync::mpsc::Sender<()>,
}
impl ResponseGate {
    #[must_use]
    pub fn observe(&self) -> GateObservation {
        match self.observed.try_recv() {
            Ok(()) => GateObservation::Held,
            Err(std::sync::mpsc::TryRecvError::Empty) => GateObservation::Waiting,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => GateObservation::Closed,
        }
    }
    pub fn release(self) -> io::Result<()> {
        self.release
            .send(())
            .map_err(|_error| io::ErrorKind::BrokenPipe.into())
    }
}
