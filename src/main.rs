mod config;
mod http;
mod router;
mod static_files;

use std::io::{Read, Write};
use std::net::TcpListener;

use config::load_config;
use http::{Method, parse_request};
use router::match_route;
use static_files::{FileResult, get_file};

fn make_response(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
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

fn make_method_not_allowed(allowed_methods: &[String]) -> Vec<u8> {
    let allowed = allowed_methods.join(", ");
    let body = b"<h1>405 - Method not allowed</h1>";

    format!(
        "HTTP/1.1 405 Method Not Allowed\r\n\
         Content-Type: text/html\r\n\
         Content-Length: {}\r\n\
         Allow: {allowed}\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    )
    .into_bytes()
    .into_iter()
    .chain(body.iter().copied())
    .collect()
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

fn main() {
    let configs = match load_config("server.conf") {
        Ok(configs) => configs,
        Err(error) => {
            eprintln!("Invalid server.conf: {error}");
            return;
        }
    };

    let server = &configs[0];
    let address = format!("{}:{}", server.host, server.ports[0]);

    let listener =
        TcpListener::bind(&address).expect("Could not start server on configured address");

    println!("Server running at http://{address}");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut buffer = [0; 4096];

                match stream.read(&mut buffer) {
                    Ok(size) => match parse_request(&buffer[..size]) {
                        Ok(request) => {
                            println!(
                                "{} {} {}",
                                method_name(&request.method),
                                request.path,
                                request.version
                            );

                            let response = if request.body.len() > server.max_body_size {
                                make_response(
                                    "413 Payload Too Large",
                                    "text/html",
                                    b"<h1>413 - Upload is too large</h1>",
                                )
                            } else {
                                match match_route(&server.routes, &request.path) {
                                    None => make_response(
                                        "404 Not Found",
                                        "text/html",
                                        b"<h1>404 - Page not found</h1>",
                                    ),

                                    Some(route) => {
                                        let method = method_name(&request.method);

                                        if !route.allowed_methods.iter().any(|item| item == method)
                                        {
                                            make_method_not_allowed(&route.allowed_methods)
                                        } else if let Some(location) = &route.redirect {
                                            make_redirect(location)
                                        } else {
                                            match request.method {
                                                Method::Get => {
                                                    let path =
                                                        route_path(&route.path, &request.path);

                                                    match get_file(&route.root, &route.index, path)
                                                    {
                                                        FileResult::Found {
                                                            content_type,
                                                            body,
                                                        } => make_response(
                                                            "200 OK",
                                                            content_type,
                                                            &body,
                                                        ),

                                                        FileResult::NotFound => make_response(
                                                            "404 Not Found",
                                                            "text/html",
                                                            b"<h1>404 - Page not found</h1>",
                                                        ),

                                                        FileResult::Forbidden => make_response(
                                                            "403 Forbidden",
                                                            "text/html",
                                                            b"<h1>403 - Forbidden</h1>",
                                                        ),

                                                        FileResult::ServerError => make_response(
                                                            "500 Internal Server Error",
                                                            "text/html",
                                                            b"<h1>500 - Server error</h1>",
                                                        ),
                                                    }
                                                }

                                                Method::Post => make_response(
                                                    "200 OK",
                                                    "text/html",
                                                    b"<h1>POST received</h1>",
                                                ),

                                                Method::Delete => make_response(
                                                    "204 No Content",
                                                    "text/plain",
                                                    b"",
                                                ),
                                            }
                                        }
                                    }
                                }
                            };

                            let _ = stream.write_all(&response);
                        }

                        Err(error) => {
                            eprintln!("Bad request: {error}");

                            let response = make_response(
                                "400 Bad Request",
                                "text/html",
                                b"<h1>400 - Bad request</h1>",
                            );

                            let _ = stream.write_all(&response);
                        }
                    },

                    Err(error) => eprintln!("Could not read request: {error}"),
                }
            }

            Err(error) => eprintln!("Connection failed: {error}"),
        }
    }
}
