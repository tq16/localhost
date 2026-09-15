use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::http::Request;

pub struct CgiOutput {
    pub status: String,
    pub content_type: String,
    pub body: Vec<u8>,
}

pub fn resolve_script(
    root: &str,
    index: &str,
    request_path: &str,
    extension: &str,
) -> Option<(PathBuf, String)> {
    let requested = if request_path == "/" {
        index
    } else {
        request_path.trim_start_matches('/')
    };

    let extension_position = requested.find(extension)?;
    let script_end = extension_position + extension.len();
    let path_info = &requested[script_end..];

    if !path_info.is_empty() && !path_info.starts_with('/') {
        return None;
    }

    let script = Path::new(root).join(&requested[..script_end]);

    Some((script, path_info.to_string()))
}

pub fn run_cgi(
    interpreter: &str,
    script: &Path,
    request: &Request,
    path_info: &str,
) -> Result<CgiOutput, String> {
    let script_name = script
        .file_name()
        .ok_or("CGI script has no file name")?;

    let mut command = Command::new(interpreter);

    command
        .arg(script_name)
        .env("REQUEST_METHOD", match &request.method {
            crate::http::Method::Get => "GET",
            crate::http::Method::Post => "POST",
            crate::http::Method::Delete => "DELETE",
        })
        .env("PATH_INFO", path_info)
        .env("CONTENT_LENGTH", request.body.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());

    if let Some(directory) = script.parent() {
        command.current_dir(directory);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start CGI: {error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&request.body)
            .map_err(|error| format!("Could not send CGI body: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Could not read CGI output: {error}"))?;

    if !output.status.success() {
        return Err(format!("CGI exited with status {}", output.status));
    }

    parse_output(&output.stdout)
}

fn parse_output(output: &[u8]) -> Result<CgiOutput, String> {
    let separator = if let Some(position) = find_bytes(output, b"\r\n\r\n") {
        (position, 4)
    } else if let Some(position) = find_bytes(output, b"\n\n") {
        (position, 2)
    } else {
        return Ok(CgiOutput {
            status: "200 OK".to_string(),
            content_type: "text/html; charset=utf-8".to_string(),
            body: output.to_vec(),
        });
    };

    let headers = std::str::from_utf8(&output[..separator.0])
        .map_err(|_| "CGI headers are invalid")?;

    let mut status = "200 OK".to_string();
    let mut content_type = "text/html; charset=utf-8".to_string();

    for line in headers.lines() {
        let line = line.trim_end_matches('\r');

        if let Some(value) = line.strip_prefix("Status:") {
            status = value.trim().to_string();
        }

        if let Some(value) = line.strip_prefix("Content-Type:") {
            content_type = value.trim().to_string();
        }
    }

    Ok(CgiOutput {
        status,
        content_type,
        body: output[separator.0 + separator.1..].to_vec(),
    })
}

fn find_bytes(data: &[u8], needle: &[u8]) -> Option<usize> {
    data.windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cgi_headers_and_body() {
        let output = b"Status: 201 Created\r\nContent-Type: text/plain\r\n\r\nHello";

        let result = parse_output(output).unwrap();

        assert_eq!(result.status, "201 Created");
        assert_eq!(result.content_type, "text/plain");
        assert_eq!(result.body, b"Hello");
    }
}