//! A one-shot HTTP server for the download tests.
//!
//! It speaks just enough HTTP/1.1 to return a fixed status and body, so the
//! orchestration (status mapping, size cap, checksum, part-file cleanup) is
//! tested end to end without a new dependency or a real network.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// Starts a server that answers exactly one request and returns its URL.
pub(crate) fn serve_once(status: &'static str, body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the test server");
    let address = listener.local_addr().expect("test server address");
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/octet-stream\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
        let _ = stream.flush();
    });
    format!("http://{address}/asset")
}
