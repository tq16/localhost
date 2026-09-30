# Localhost HTTP Server

A configurable HTTP server built from scratch in **Rust**.

The project implements core HTTP server functionality including request parsing, routing, static file serving, file uploads, sessions, CGI execution, redirects, directory listing, and custom error handling.

## Features

- HTTP request parsing
- `GET`, `POST`, and `DELETE` methods
- Configurable routes
- Static file serving
- File uploads
- Multipart request handling
- Directory listing
- HTTP redirects
- Sessions and cookies
- CGI execution with Python
- Chunked transfer encoding
- `Content-Length` handling
- Maximum request body size
- Custom error pages
- Multiple server configurations
- Configurable ports and server names

## Tech Stack

- **Rust**
- **TCP / HTTP**
- **Python** for CGI scripts
- **HTML** for static pages
- **Cargo**

## Project Structure

```text
localhost/
├── assets/        # Static assets
├── cgi/           # CGI scripts
├── errors/        # Custom HTTP error pages
├── src/           # Rust server source code
├── tests/         # Automated tests
├── uploads/       # Uploaded files
├── www/           # Website files
├── Cargo.toml
├── Cargo.lock
└── server.conf    # Server configuration
```

## Configuration

The server is configured through `server.conf`.

The configuration supports options such as:

- Host and port
- Server names
- Root directories
- Index files
- Maximum body size
- Allowed HTTP methods
- Directory listing
- Redirects
- CGI handlers
- Custom error pages

Example:

```text
server {
    host 127.0.0.1
    port 8080

    root www
    index index.html
    max_body_size 1048576

    route / {
        methods GET POST DELETE
        root www
        index index.html
        directory_listing off
    }
}
```

## Running the Server

Make sure Rust and Cargo are installed.

Clone the repository:

```bash
git clone https://github.com/tq16/localhost.git
cd localhost
```

Build and run:

```bash
cargo run
```

The default configuration runs the server locally on:

```text
http://127.0.0.1:8080
```

The configuration also contains an additional listener on port `8081`.

## Example Requests

Get the home page:

```bash
curl http://127.0.0.1:8080/
```

View the uploads directory:

```bash
curl http://127.0.0.1:8080/uploads/
```

Run the configured CGI route:

```bash
curl http://127.0.0.1:8080/cgi/
```

## Testing

Run the automated test suite with:

```bash
cargo test
```

The project was also tested under concurrent HTTP load to verify server stability and request handling.

## What I Learned

This project gave me practical experience with:

- HTTP protocol fundamentals
- TCP networking
- Request and response parsing
- Server routing
- File and stream handling
- CGI
- Sessions and cookies
- Error handling
- Configuration parsing
- Systems programming in Rust

## Author

**Taqi Sarhan**

Software Developer | Programming Student at Bahrain Polytechnic | Reboot01 Developer
