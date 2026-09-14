use std::collections::HashMap;
use std::str;

#[derive(Debug, PartialEq)]
pub enum Method {
    Get,
    Post,
    Delete,
}

#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

pub fn parse_request(raw: &[u8]) -> Result<Request, String> {
    let header_end = find_header_end(raw).ok_or("Request headers are incomplete")?;

    let header_text = str::from_utf8(&raw[..header_end - 4])
        .map_err(|_| "Headers are not valid UTF-8")?;

    let mut lines = header_text.split("\r\n");

    let first_line = lines.next().ok_or("Missing request line")?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() != 3 {
        return Err("Invalid request line".to_string());
    }

    let method = match parts[0] {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "DELETE" => Method::Delete,
        _ => return Err("Unsupported HTTP method".to_string()),
    };

    if !parts[1].starts_with('/') {
        return Err("Request path must start with '/'".to_string());
    }

    if parts[2] != "HTTP/1.1" {
        return Err("Only HTTP/1.1 is supported".to_string());
    }

    let mut headers = HashMap::new();

    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or("Invalid header")?;

        let name = name.trim().to_lowercase();
        let value = value.trim().to_string();

        if name.is_empty() {
            return Err("Header name cannot be empty".to_string());
        }

        headers.insert(name, value);
    }

    if !headers.contains_key("host") {
        return Err("HTTP/1.1 request needs a Host header".to_string());
    }

    let raw_body = &raw[header_end..];

    let body = if headers
        .get("transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"))
    {
        decode_chunked_body(raw_body)?
    } else if let Some(content_length) = headers.get("content-length") {
        let expected_size = content_length
            .parse::<usize>()
            .map_err(|_| "Invalid Content-Length")?;

        if raw_body.len() < expected_size {
            return Err("Request body is incomplete".to_string());
        }

        raw_body[..expected_size].to_vec()
    } else {
        Vec::new()
    };

    Ok(Request {
        method,
        path: parts[1].to_string(),
        version: parts[2].to_string(),
        headers,
        body,
    })
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn find_crlf(data: &[u8], start: usize) -> Option<usize> {
    data[start..]
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|position| start + position)
}

fn decode_chunked_body(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut position = 0;

    loop {
        let line_end = find_crlf(raw, position)
            .ok_or("Chunk size line is incomplete")?;

        let size_text = str::from_utf8(&raw[position..line_end])
            .map_err(|_| "Chunk size is invalid")?;

        let size_text = size_text.split(';').next().unwrap_or("").trim();

        let chunk_size = usize::from_str_radix(size_text, 16)
            .map_err(|_| "Chunk size is not valid hexadecimal")?;

        position = line_end + 2;

        if chunk_size == 0 {
            if raw.len() < position + 2 {
                return Err("Chunked body is incomplete".to_string());
            }

            if &raw[position..position + 2] == b"\r\n" {
                return Ok(body);
            }

            let trailer_end = find_header_end(&raw[position..])
                .ok_or("Chunk trailers are incomplete")?;

            let _ = trailer_end;
            return Ok(body);
        }

        if raw.len() < position + chunk_size + 2 {
            return Err("Chunk data is incomplete".to_string());
        }

        body.extend_from_slice(&raw[position..position + chunk_size]);
        position += chunk_size;

        if &raw[position..position + 2] != b"\r\n" {
            return Err("Chunk must end with CRLF".to_string());
        }

        position += 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_get_request() {
        let raw = b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n";

        let request = parse_request(raw).unwrap();

        assert_eq!(request.method, Method::Get);
        assert_eq!(request.path, "/hello");
        assert_eq!(request.headers.get("host").unwrap(), "localhost");
    }

    #[test]
    fn rejects_an_invalid_method() {
        let raw = b"PUT /hello HTTP/1.1\r\nHost: localhost\r\n\r\n";

        assert!(parse_request(raw).is_err());
    }

    #[test]
    fn parses_a_post_body_using_content_length() {
        let raw = b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 11\r\n\r\nhello world";

        let request = parse_request(raw).unwrap();

        assert_eq!(request.method, Method::Post);
        assert_eq!(request.body, b"hello world");
    }

    #[test]
    fn parses_a_chunked_body() {
        let raw = b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";

        let request = parse_request(raw).unwrap();

        assert_eq!(request.body, b"Wikipedia");
    }

    #[test]
    fn rejects_an_incomplete_body() {
        let raw = b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\nshort";

        assert!(parse_request(raw).is_err());
    }
}