use testkit::ScriptedHttpServer;

use super::{PublicIpAddress, PublicIpClient, PublicIpEndpoint, PublicIpError};

fn http(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn lookup(response: Vec<u8>) -> (Result<PublicIpAddress, PublicIpError>, String) {
    let server = ScriptedHttpServer::start(vec![response]).expect("server");
    let client =
        PublicIpClient::new(PublicIpEndpoint::local(server.port(), "/ip").expect("endpoint"))
            .expect("client");
    let result = runtime().block_on(client.lookup());
    let sent = String::from_utf8(server.finish().expect("sent")).expect("utf8");
    (result, sent)
}

#[test]
fn ipv4_body_with_trailing_newline_is_the_public_address_and_one_plain_get_is_sent() {
    let (result, sent) = lookup(http("200 OK", "203.0.114.7\n"));
    assert_eq!(
        result.map(PublicIpAddress::to_text),
        Ok(String::from("203.0.114.7"))
    );
    assert!(sent.starts_with("GET /ip HTTP/1.1"));
    assert!(
        sent.to_ascii_lowercase()
            .contains("user-agent: stock-copilot")
    );
    assert!(!sent.to_ascii_lowercase().contains("cookie"));
}

#[test]
fn ipv6_body_is_accepted() {
    let (result, _sent) = lookup(http("200 OK", "2606:4700:4700::1111"));
    assert_eq!(
        result.map(PublicIpAddress::to_text),
        Ok(String::from("2606:4700:4700::1111"))
    );
}

#[test]
fn redirects_errors_large_bodies_and_non_public_addresses_are_rejected() {
    for (response, expected) in [
        (
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            PublicIpError::RedirectBlocked,
        ),
        (http("500 Internal Server Error", "203.0.114.7"), PublicIpError::Status),
        (http("200 OK", &"1".repeat(65)), PublicIpError::ResponseTooLarge),
        (http("200 OK", "<html>not an ip</html>"), PublicIpError::InvalidAddress),
        (http("200 OK", "10.0.0.1"), PublicIpError::InvalidAddress),
        (http("200 OK", "127.0.0.1"), PublicIpError::InvalidAddress),
        (http("200 OK", "fe80::1"), PublicIpError::InvalidAddress),
    ] {
        assert_eq!(lookup(response).0, Err(expected));
    }
}

#[test]
fn service_endpoint_accepts_only_plain_https_urls() {
    let _endpoint = PublicIpEndpoint::service("https://ip.example.test").expect("https endpoint");
    for raw in [
        "",
        "http://ip.example.test",
        "https://ip.example.test?format=text",
        "https://ip.example.test#x",
        "https://user@ip.example.test",
        "https://ip.example.test:8443",
        "https://ip.example.test.",
        "ftp://ip.example.test",
    ] {
        assert_eq!(
            PublicIpEndpoint::service(raw),
            Err(PublicIpError::InvalidEndpoint),
            "{raw}"
        );
    }
    assert_eq!(
        PublicIpEndpoint::local(0, "/"),
        Err(PublicIpError::InvalidEndpoint)
    );
    assert_eq!(
        PublicIpEndpoint::local(8080, "ip"),
        Err(PublicIpError::InvalidEndpoint)
    );
}
