//! Loopback HTTP fixture for provider contract tests. Never contacts a paid service.

use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

pub async fn http_fixture(
    status: u16,
    response: &str,
) -> (String, tokio::task::JoinHandle<CapturedRequest>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let response = response.to_owned();
    let handle = tokio::spawn(async move {
        tokio::time::timeout(std::time::Duration::from_secs(10), async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut received = Vec::new();
            let header_end = loop {
                let mut chunk = [0_u8; 8192];
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0, "request ended before headers");
                received.extend_from_slice(&chunk[..n]);
                if let Some(pos) = received.windows(4).position(|w| w == b"\r\n\r\n") { break pos + 4; }
                assert!(received.len() < 65_536, "oversized test headers");
            };
            let header_text = String::from_utf8_lossy(&received[..header_end]).to_string();
            let mut lines = header_text.lines();
            let mut request_line = lines.next().unwrap().split_whitespace();
            let method = request_line.next().unwrap().to_string();
            let path = request_line.next().unwrap().to_string();
            let headers: HashMap<String, String> = lines.filter_map(|line| {
                line.split_once(':').map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
            }).collect();
            let length: usize = headers.get("content-length").expect("known request length").parse().unwrap();
            assert!(length < 2_000_000, "oversized test body");
            while received.len() < header_end + length {
                let mut chunk = [0_u8; 8192];
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0, "request ended before body");
                received.extend_from_slice(&chunk[..n]);
            }
            let reply = format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len());
            socket.write_all(reply.as_bytes()).await.unwrap();
            CapturedRequest { method, path, headers, body: received[header_end..header_end + length].to_vec() }
        }).await.expect("loopback request timed out")
    });
    (url, handle)
}

pub fn audio_request(model: &str) -> super::TranscriptionRequest {
    super::TranscriptionRequest {
        audio_path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/english-speech.wav"),
        model: model.into(),
        language: Some("es".into()),
        prompt: Some("Málaga, Acme Corp; Málaga".into()),
    }
}
