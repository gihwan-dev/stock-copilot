use std::io::Write;

pub(crate) fn place(host: &str, verb: &str, path: &str) {
    let mut stream = std::net::TcpStream::connect(host);
    stream.write_all(format!("{verb} {path} HTTP/1.1\r\n\r\n").as_bytes());
}
