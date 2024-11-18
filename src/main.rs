use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::fs;
use std::collections::HashMap;
use tracing::{info, warn, error}; // Structured logging

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt::init();
    info!("Starting server on http://127.0.0.1:8080");

    // Create a TCP listener to handle incoming connections
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("Server is running and ready to accept connections");

    loop {
        // Accept an incoming client connection
        let (mut socket, addr) = listener.accept().await?;
        info!("New connection established from: {}", addr);

        // Spawn a new task to handle the connection asynchronously
        tokio::spawn(async move {
            let mut buffer = vec![0; 4096]; // Allocate a buffer to read incoming data
            let bytes_read = match socket.read(&mut buffer).await {
                Ok(0) => {
                    warn!("Connection closed by client");
                    return; // Connection was closed
                }
                Ok(n) => n, // Successfully read n bytes
                Err(e) => {
                    error!("Failed to read request: {}", e);
                    return; // Log and exit on error
                }
            };

            // Convert the buffer into a request string
            let request = String::from_utf8_lossy(&buffer[..bytes_read]);

            // Parse the HTTP request (method, path, headers, and body)
            let (method, path, headers, body) = parse_request_with_body(&request);

            // Route the request and generate a response
            let response = route_request(method, path, headers, body).await;

            // Send the response back to the client
            if let Err(e) = socket.write_all(response.as_bytes()).await {
                error!("Failed to send response: {}", e);
            } else {
                info!("Response successfully sent to client");
            }
        });
    }
}

/// Parses the HTTP request and extracts the method, path, headers, and body.
///
/// # Arguments
/// * `request` - A string slice of the raw HTTP request.
///
/// # Returns
/// A tuple containing:
/// - `method`: HTTP method (e.g., "GET", "POST")
/// - `path`: Request path (e.g., "/static/index.html")
/// - `headers`: Parsed headers as a HashMap
/// - `body`: The request body (if any)
fn parse_request_with_body(request: &str) -> (&str, &str, HashMap<String, String>, String) {
    let mut lines = request.lines(); // Split the request into lines
    let first_line = lines.next().unwrap_or_default(); // First line contains method, path, and protocol
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or(""); // Extract method (e.g., "GET")
    let path = parts.next().unwrap_or(""); // Extract path (e.g., "/static/file.txt")

    // Parse headers into a HashMap
    let mut headers = HashMap::new();
    let mut body = String::new(); // Initialize an empty String for the body

    for line in &mut lines {
        if line.is_empty() {
            // Stop parsing headers when we encounter an empty line
            body = lines.collect::<Vec<&str>>().join("\n"); // Collect the remaining lines as the body
            break;
        }
        if let Some((key, value)) = line.split_once(": ") {
            headers.insert(key.to_string(), value.to_string()); // Insert header key-value pairs
        }
    }

    (method, path, headers, body)
}

/// Routes the HTTP request based on the method and path.
///
/// # Arguments
/// * `method` - HTTP method (e.g., "GET", "POST").
/// * `path` - The request path.
/// * `headers` - Request headers.
/// * `body` - Request body.
///
/// # Returns
/// A string containing the HTTP response.
async fn route_request(
    method: &str,
    path: &str,
    headers: HashMap<String, String>,
    body: String,
) -> String {
    match method {
        "GET" => match path {
            "/" => {
                let body = "Welcome to the homepage!";
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
            }
            path if path.starts_with("/static/") => serve_file(&path[1..]).await,
            _ => {
                let body = "404 Not Found";
                format!(
                    "HTTP/1.1 404 NOT FOUND\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
            }
        },
        "POST" => handle_post(path, headers, body).await,
        _ => {
            let body = "405 Method Not Allowed";
            format!(
                "HTTP/1.1 405 METHOD NOT ALLOWED\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
        }
    }
}

/// Serves a file, dynamically setting the Content-Type header based on file extension.
///
/// # Arguments
/// * `filepath` - The file path (relative to the project root).
///
/// # Returns
/// A string containing the HTTP response with the file's content or a 404 error.
async fn serve_file(filepath: &str) -> String {
    match fs::read_to_string(filepath).await {
        Ok(content) => {
            let mime_type = get_mime_type(filepath); // Determine MIME type based on file extension
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
                mime_type,
                content.len(),
                content
            )
        }
        Err(_) => {
            let body = "404 File Not Found";
            format!(
                "HTTP/1.1 404 NOT FOUND\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
        }
    }
}

/// Handles POST requests, processing the body.
///
/// # Arguments
/// * `path` - The request path.
/// * `headers` - Parsed headers.
/// * `body` - The request body.
///
/// # Returns
/// A string containing the HTTP response.
async fn handle_post(
    path: &str,
    headers: HashMap<String, String>,
    body: String,
) -> String {
    match path {
        "/submit" => {
            info!("Processing POST request to /submit with body: {}", body);

            // Check if the Content-Type header is JSON or form-encoded
            if let Some(content_type) = headers.get("Content-Type") {
                if content_type == "application/json" {
                    info!("Received JSON payload");
                    return format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        format!("Received JSON: {}", body)
                    );
                } else if content_type == "application/x-www-form-urlencoded" {
                    info!("Received form-encoded payload");
                    return format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        format!("Received form data: {}", body)
                    );
                }
            }

            // Default fallback for unsupported Content-Type
            warn!("Unsupported Content-Type: {:?}", headers.get("Content-Type"));
            let response_body = "Unsupported Content-Type";
            format!(
                "HTTP/1.1 415 UNSUPPORTED MEDIA TYPE\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            )
        }
        _ => {
            let body = "404 Not Found";
            format!(
                "HTTP/1.1 404 NOT FOUND\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
        }
    }
}

/// Determines the MIME type based on the file extension.
///
/// # Arguments
/// * `filepath` - The file path.
///
/// # Returns
/// The MIME type as a string.
fn get_mime_type(filepath: &str) -> &str {
    match filepath.rsplit('.').next() {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("txt") => "text/plain",
        _ => "application/octet-stream", // Default binary type
    }
}
