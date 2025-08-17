// whambam - A high-performance HTTP load testing tool
//
// Copyright (c) 2025 Stephen Harrison
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::sleep;

struct ServerState {
    request_count: AtomicUsize,
    headers: Mutex<HashMap<String, Vec<String>>>,
    status_code: AtomicUsize,
    delay_ms: AtomicUsize,
}

impl ServerState {
    fn new() -> Self {
        ServerState {
            request_count: AtomicUsize::new(0),
            headers: Mutex::new(HashMap::new()),
            status_code: AtomicUsize::new(200),
            delay_ms: AtomicUsize::new(0),
        }
    }
}

pub struct MockServer {
    port: u16,
    state: Arc<ServerState>,
    server_task: Option<tokio::task::JoinHandle<()>>,
}

impl MockServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(ServerState::new());

        let state_clone = state.clone();
        let server_task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = state_clone.clone();
                tokio::spawn(async move {
                    handle_connection(stream, state).await;
                });
            }
        });

        MockServer {
            port,
            state,
            server_task: Some(server_task),
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn request_count(&self) -> usize {
        self.state.request_count.load(Ordering::SeqCst)
    }

    pub fn set_response_status(&self, status: u16) {
        self.state
            .status_code
            .store(status as usize, Ordering::SeqCst);
    }

    pub fn set_response_delay(&self, delay_ms: u64) {
        self.state
            .delay_ms
            .store(delay_ms as usize, Ordering::SeqCst);
    }

    pub fn get_received_headers(&self) -> HashMap<String, Vec<String>> {
        self.state.headers.lock().unwrap().clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(task) = self.server_task.take() {
            task.abort();
        }
    }
}

async fn handle_connection(mut stream: TcpStream, state: Arc<ServerState>) {
    let mut buffer = [0; 1024];

    // Read the request
    let mut headers = Vec::new();

    // Simple HTTP request parsing
    let mut bytes_read = 0;
    while bytes_read < buffer.len() {
        match stream.read(&mut buffer[bytes_read..bytes_read + 1]).await {
            Ok(0) => break, // End of stream
            Ok(n) => {
                bytes_read += n;

                // Check if we have a complete line
                if buffer[bytes_read - 1] == b'\n'
                    && bytes_read >= 2
                    && buffer[bytes_read - 2] == b'\r'
                {
                    // We have a complete line
                    let line = String::from_utf8_lossy(&buffer[..bytes_read - 2]);
                    headers.push(line.to_string());

                    // If we got an empty line, we're done with headers
                    if line.is_empty() {
                        break;
                    }

                    // Reset for next line
                    buffer = [0; 1024];
                    bytes_read = 0;
                }
            }
            Err(_) => break,
        }
    }

    // Process headers - Do this inside a block to ensure the mutex is dropped before the await
    {
        let mut header_map = state.headers.lock().unwrap();

        for line in headers.iter().skip(1) {
            // Skip the request line
            if line.is_empty() {
                break;
            }

            if let Some(idx) = line.find(':') {
                let (name, value) = line.split_at(idx);
                let name = name.trim().to_lowercase();
                let value = value[1..].trim().to_string();

                header_map.entry(name).or_default().push(value);
            }
        }
    }

    // Increment request counter
    state.request_count.fetch_add(1, Ordering::SeqCst);

    // Apply delay if configured
    let delay_ms = state.delay_ms.load(Ordering::SeqCst);
    if delay_ms > 0 {
        sleep(Duration::from_millis(delay_ms as u64)).await;
    }

    // Send response
    let status = state.status_code.load(Ordering::SeqCst) as u16;
    let status_text = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: text/plain\r\n\
         Connection: close\r\n\
         Content-Length: 13\r\n\
         \r\n\
         Hello, World!"
    );

    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;

    #[tokio::test]
    async fn test_mock_server() {
        let server = MockServer::start().await;
        let client = Client::new();

        // Test basic request
        let resp = client.get(server.url()).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(server.request_count(), 1);

        // Test with custom status code
        server.set_response_status(404);
        let resp = client.get(server.url()).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 404);
        assert_eq!(server.request_count(), 2);

        // Test with custom headers
        let _resp = client
            .get(server.url())
            .header("X-Test", "test-value")
            .header("User-Agent", "mock-client")
            .send()
            .await
            .unwrap();

        let headers = server.get_received_headers();
        assert!(headers.contains_key("x-test"));
        assert_eq!(headers.get("x-test").unwrap()[0], "test-value");
        assert_eq!(server.request_count(), 3);
    }
}
