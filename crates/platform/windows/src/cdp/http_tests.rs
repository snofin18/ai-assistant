//! Loopback contract tests for bounded CDP HTTP response reads.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use super::discover_pages;

#[test]
fn test_discovery_reads_content_length_without_waiting_for_eof()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let server = thread::spawn(
        move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let (mut stream, _) = listener.accept()?;
            read_headers(&mut stream)?;
            let body = format!(
                "[{{\"type\":\"page\",\"url\":\"http://127.0.0.1:8765/page-1.html\",\
             \"title\":\"Keep Alive\",\"webSocketDebuggerUrl\":\
             \"ws://127.0.0.1:{port}/devtools/page/keep-alive\"}}]"
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: keep-alive\r\n\r\n{body}",
                body.len()
            )?;
            stream.flush()?;
            thread::sleep(Duration::from_millis(500));
            Ok(())
        },
    );

    let pages = discover_pages("127.0.0.1", port, Duration::from_millis(200))?;
    assert_eq!(pages.len(), 1);
    let page = pages
        .first()
        .ok_or_else(|| io::Error::other("keep-alive fixture returned no page"))?;
    assert_eq!(page.title, "Keep Alive");
    server
        .join()
        .map_err(|_| io::Error::other("fixture thread panicked"))??;
    Ok(())
}

#[test]
fn test_discovery_keeps_eof_compatibility_without_content_length()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let server = thread::spawn(
        move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let (mut stream, _) = listener.accept()?;
            read_headers(&mut stream)?;
            let body = format!(
                "[{{\"type\":\"page\",\"url\":\"http://127.0.0.1:8765/page-1.html\",\
             \"title\":\"Close Delimited\",\"webSocketDebuggerUrl\":\
             \"ws://127.0.0.1:{port}/devtools/page/close-delimited\"}}]"
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
             Connection: close\r\n\r\n{body}"
            )?;
            Ok(())
        },
    );

    let pages = discover_pages("127.0.0.1", port, Duration::from_secs(1))?;
    assert_eq!(pages.len(), 1);
    let page = pages
        .first()
        .ok_or_else(|| io::Error::other("close-delimited fixture returned no page"))?;
    assert_eq!(page.title, "Close Delimited");
    server
        .join()
        .map_err(|_| io::Error::other("fixture thread panicked"))??;
    Ok(())
}

#[test]
fn test_discovery_accepts_empty_about_blank_title()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let server = thread::spawn(
        move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let (mut stream, _) = listener.accept()?;
            read_headers(&mut stream)?;
            let body = format!(
                "[{{\"type\":\"page\",\"url\":\"about:blank\",\"title\":\"\",\
                 \"webSocketDebuggerUrl\":\
                 \"ws://127.0.0.1:{port}/devtools/page/about-blank\"}}]"
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: keep-alive\r\n\r\n{body}",
                body.len()
            )?;
            stream.flush()?;
            Ok(())
        },
    );

    let pages = discover_pages("127.0.0.1", port, Duration::from_secs(1))?;
    assert_eq!(pages.len(), 1);
    let page = pages
        .first()
        .ok_or_else(|| io::Error::other("about:blank fixture returned no page"))?;
    assert_eq!(page.url, "about:blank");
    assert_eq!(page.title, "");
    server
        .join()
        .map_err(|_| io::Error::other("fixture thread panicked"))??;
    Ok(())
}

fn read_headers(stream: &mut TcpStream) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    while bytes.len() < 16 * 1024 {
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return String::from_utf8(bytes)
                .map_err(|error| io::Error::other(format!("fixture header UTF-8: {error}")));
        }
    }
    Err(io::Error::other("fixture header exceeded 16 KiB"))
}
