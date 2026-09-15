mod cgi;
mod config;
mod epoll;
mod event_loop;
mod http;
mod multipart;
mod router;
mod sessions;
mod static_files;

use cgi::{resolve_script, run_cgi};
use config::{ServerConfig, load_config};
use http::{Method, Request};
use multipart::parse_multipart;
use router::match_route;
use sessions::SessionStore;
use static_files::{FileResult, UploadResult, delete_file, get_file, save_file};
use std::fs;

pub(crate) fn make_response(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let headers = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );

    let mut response = headers.into_bytes();
    response.extend_from_slice(body);
    response
}

pub(crate) fn make_error_response(server: &ServerConfig, code: u16, status: &str) -> Vec<u8> {
    if let Some(path) = server.error_pages.get(&code) {
        if let Ok(body) = fs::read(path) {
            return make_response(status, "text/html; charset=utf-8", &body);
        }
    }

    let fallback = format!("<h1>{code} - {status}</h1>");

    make_response(status, "text/html; charset=utf-8", fallback.as_bytes())
}

fn make_response_with_cookie(
    status: &str,
    content_type: &str,
    body: &[u8],
    session_id: &str,
) -> Vec<u8> {
    let headers = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Set-Cookie: session_id={session_id}; Path=/; HttpOnly; SameSite=Lax\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );

    let mut response = headers.into_bytes();
    response.extend_from_slice(body);
    response
}

fn make_redirect(location: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 302 Found\r\n\
         Location: {location}\r\n\
         Content-Length: 0\r\n\
         Connection: close\r\n\
         \r\n"
    )
    .into_bytes()
}

fn make_method_not_allowed(server: &ServerConfig, allowed_methods: &[String]) -> Vec<u8> {
    let allowed = allowed_methods.join(", ");
    let body = server
        .error_pages
        .get(&405)
        .and_then(|path| fs::read(path).ok())
        .unwrap_or_else(|| b"<h1>405 - 405 Method Not Allowed</h1>".to_vec());

    let headers = format!(
        "HTTP/1.1 405 Method Not Allowed\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Allow: {allowed}\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );

    let mut response = headers.into_bytes();
    response.extend_from_slice(&body);
    response
}

fn method_name(method: &Method) -> &'static str {
    match method {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Delete => "DELETE",
    }
}

fn route_path<'a>(route_path: &str, request_path: &'a str) -> &'a str {
    if route_path == "/" {
        request_path
    } else {
        let remaining = &request_path[route_path.len()..];

        if remaining.is_empty() { "/" } else { remaining }
    }
}

fn upload_response(server: &ServerConfig, result: UploadResult) -> Vec<u8> {
    match result {
        UploadResult::Saved => make_response(
            "201 Created",
            "text/html",
            b"<h1>File uploaded successfully</h1>",
        ),

        UploadResult::Forbidden => make_error_response(server, 403, "403 Forbidden"),

        UploadResult::NotFound | UploadResult::ServerError => {
            make_error_response(server, 500, "500 Internal Server Error")
        }
    }
}

pub(crate) fn is_incomplete_request(error: &str) -> bool {
    return matches!(
        error,
        "Request headers are incomplete"
            | "Request body is incomplete"
            | "Chunk size line is incomplete"
            | "Chunked body is incomplete"
            | "Chunk trailers are incomplete"
            | "Chunk data is incomplete"
    );
}

pub(crate) fn handle_request(
    request: Request,
    server: &ServerConfig,
    sessions: &mut SessionStore,
) -> Vec<u8> {
    println!(
        "{} {} {}",
        method_name(&request.method),
        request.path,
        request.version
    );

    if request.path == "/session" && matches!(&request.method, Method::Get) {
        let cookie = request.headers.get("cookie").map(String::as_str);
        let (session_id, visits, is_new) = sessions.visit(cookie);

        let body = if is_new {
            format!("<h1>New session created</h1><p>Visits: {visits}</p>")
        } else {
            format!("<h1>Welcome back</h1><p>Visits: {visits}</p>")
        };

        return make_response_with_cookie("200 OK", "text/html", body.as_bytes(), &session_id);
    }

    if request.body.len() > server.max_body_size {
        return make_error_response(server, 413, "413 Payload Too Large");
    }

    let route = match match_route(&server.routes, &request.path) {
        Some(route) => route,

        None => {
            return make_error_response(server, 404, "404 Not Found");
        }
    };

    let method = method_name(&request.method);

    if !route.allowed_methods.iter().any(|item| item == method) {
        return make_method_not_allowed(server, &route.allowed_methods);
    }

    if let Some(location) = &route.redirect {
        return make_redirect(location);
    }

    if let Some((extension, interpreter)) = &route.cgi {
        let path = route_path(&route.path, &request.path);

        if let Some((script, path_info)) =
            resolve_script(&route.root, &route.index, path, extension)
        {
            match run_cgi(interpreter, &script, &request, &path_info) {
                Ok(output) => {
                    return make_response(&output.status, &output.content_type, &output.body);
                }

                Err(error) => {
                    eprintln!("CGI error: {error}");

                    return make_error_response(server, 500, "500 Internal Server Error");
                }
            }
        }
    }

    match request.method {
        Method::Get => {
            let path = route_path(&route.path, &request.path);

            match get_file(&route.root, &route.index, path, route.directory_listing) {
                FileResult::Found { content_type, body } => {
                    make_response("200 OK", content_type, &body)
                }

                FileResult::DirectoryListing { body } => {
                    make_response("200 OK", "text/html; charset=utf-8", &body)
                }

                FileResult::NotFound => make_error_response(server, 404, "404 Not Found"),

                FileResult::Forbidden => make_error_response(server, 403, "403 Forbidden"),

                FileResult::ServerError => {
                    make_error_response(server, 500, "500 Internal Server Error")
                }
            }
        }

        Method::Post => {
            let path = route_path(&route.path, &request.path);

            if path == "/" {
                match request.headers.get("content-type") {
                    Some(content_type) if content_type.starts_with("multipart/form-data") => {
                        match parse_multipart(&request.body, content_type) {
                            Ok(files) => {
                                let file = &files[0];

                                upload_response(
                                    server,
                                    save_file(&route.root, &file.filename, &file.data),
                                )
                            }

                            Err(error) => {
                                eprintln!("Bad multipart upload: {error}");

                                make_error_response(server, 400, "400 Bad Request")
                            }
                        }
                    }

                    _ => make_error_response(server, 400, "400 Bad Request"),
                }
            } else {
                upload_response(server, save_file(&route.root, path, &request.body))
            }
        }

        Method::Delete => {
            let path = route_path(&route.path, &request.path);

            match delete_file(&route.root, path) {
                UploadResult::Saved => make_response("204 No Content", "text/plain", b""),

                UploadResult::NotFound => make_error_response(server, 404, "404 Not Found"),

                UploadResult::Forbidden => make_error_response(server, 403, "403 Forbidden"),

                UploadResult::ServerError => {
                    make_error_response(server, 500, "500 Internal Server Error")
                }
            }
        }
    }
}

fn main() {
    let configs = match load_config("server.conf") {
        Ok(configs) => configs,
        Err(error) => {
            eprintln!("Invalid server.conf: {error}");
            return;
        }
    };

    let mut sessions = SessionStore::new();

    if let Err(error) = event_loop::run(configs, &mut sessions) {
        eprintln!("Server error: {error}");
    }
}
