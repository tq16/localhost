use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::router::Route;

const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub ports: Vec<u16>,
    pub server_names: Vec<String>,
    pub root: String,
    pub index: String,
    pub max_body_size: usize,
    pub routes: Vec<Route>,
    pub error_pages: HashMap<u16, String>,
}

impl ServerConfig {
    fn new() -> Self {
        Self {
            host: String::new(),
            ports: Vec::new(),
            server_names: Vec::new(),
            root: String::new(),
            index: "index.html".to_string(),
            max_body_size: 1024 * 1024,
            routes: Vec::new(),
            error_pages: HashMap::new(),
        }
    }
}

pub fn load_config(path: &str) -> Result<Vec<ServerConfig>, String> {
    let content =
        fs::read_to_string(path).map_err(|error| format!("Could not read config file: {error}"))?;

    parse_config(&content)
}

pub fn parse_config(content: &str) -> Result<Vec<ServerConfig>, String> {
    let mut servers = Vec::new();
    let mut current_server: Option<ServerConfig> = None;
    let mut current_route: Option<Route> = None;

    for (number, raw_line) in content.lines().enumerate() {
        let line_number = number + 1;
        let line = raw_line.trim();

        if line.is_empty() {
            continue;
        }

        if line == "server {" {
            if current_server.is_some() {
                return Err(format!("Line {line_number}: server block is already open"));
            }

            current_server = Some(ServerConfig::new());
            continue;
        }

        let words: Vec<&str> = line.split_whitespace().collect();

        if words.len() == 3 && words[0] == "route" && words[2] == "{" {
            if current_server.is_none() {
                return Err(format!("Line {line_number}: route must be inside a server"));
            }

            if current_route.is_some() {
                return Err(format!("Line {line_number}: route block is already open"));
            }

            if !words[1].starts_with('/') {
                return Err(format!(
                    "Line {line_number}: route path must start with '/'"
                ));
            }

            current_route = Some(Route::new(words[1], ""));
            continue;
        }

        if line == "}" {
            if let Some(route) = current_route.take() {
                let server = current_server
                    .as_mut()
                    .ok_or("Route exists without a server")?;

                validate_route(&route)?;
                server.routes.push(route);
                continue;
            }

            let server = current_server
                .take()
                .ok_or_else(|| format!("Line {line_number}: unexpected closing brace"))?;

            servers.push(finish_server(server)?);
            continue;
        }

        let key = words
            .first()
            .ok_or_else(|| format!("Line {line_number}: invalid setting"))?;

        let values = &words[1..];

        if values.is_empty() {
            return Err(format!("Line {line_number}: missing value for {key}"));
        }

        if let Some(route) = current_route.as_mut() {
            parse_route_setting(route, key, values, line_number)?;
        } else {
            let server = current_server.as_mut().ok_or_else(|| {
                format!("Line {line_number}: setting must be inside server block")
            })?;

            parse_server_setting(server, key, values, line_number)?;
        }
    }

    if current_route.is_some() {
        return Err("Missing closing brace for route block".to_string());
    }

    if current_server.is_some() {
        return Err("Missing closing brace for server block".to_string());
    }

    if servers.is_empty() {
        return Err("Config file has no server blocks".to_string());
    }

    Ok(servers)
}

fn parse_server_setting(
    server: &mut ServerConfig,
    key: &str,
    values: &[&str],
    line_number: usize,
) -> Result<(), String> {
    if key == "server_name" {
        server
            .server_names
            .extend(values.iter().map(|name| name.to_string()));
        return Ok(());
    }

    if key == "error_page" {
        if values.len() != 2 {
            return Err(format!(
                "Line {line_number}: error_page needs a status code and file path"
            ));
        }

        let status_code = values[0]
            .parse::<u16>()
            .map_err(|_| format!("Line {line_number}: invalid error status code"))?;

        match status_code {
            400 | 403 | 404 | 405 | 413 | 500 => {}
            _ => {
                return Err(format!("Line {line_number}: unsupported error status code"));
            }
        }

        if !Path::new(values[1]).is_file() {
            return Err(format!(
                "Line {line_number}: error page file does not exist"
            ));
        }

        server
            .error_pages
            .insert(status_code, values[1].to_string());

        return Ok(());
    }

    if values.len() != 1 {
        return Err(format!("Line {line_number}: {key} needs one value"));
    }

    match key {
        "host" => server.host = values[0].to_string(),

        "port" => {
            let port = values[0]
                .parse::<u16>()
                .map_err(|_| format!("Line {line_number}: invalid port"))?;

            if port == 0 {
                return Err(format!("Line {line_number}: port cannot be 0"));
            }

            server.ports.push(port);
        }

        "root" => server.root = values[0].to_string(),

        "index" => server.index = values[0].to_string(),

        "max_body_size" => {
            server.max_body_size = values[0]
                .parse::<usize>()
                .map_err(|_| format!("Line {line_number}: invalid body size"))?;
        }

        _ => {
            return Err(format!(
                "Line {line_number}: unknown server setting '{key}'"
            ));
        }
    }

    Ok(())
}

fn parse_route_setting(
    route: &mut Route,
    key: &str,
    values: &[&str],
    line_number: usize,
) -> Result<(), String> {
    match key {
        "methods" => {
            route.allowed_methods = values.iter().map(|value| value.to_string()).collect();
        }

        "root" if values.len() == 1 => {
            route.root = values[0].to_string();
        }

        "index" if values.len() == 1 => {
            route.index = values[0].to_string();
        }

        "redirect" if values.len() == 1 => {
            route.redirect = Some(values[0].to_string());
        }

        "directory_listing" if values.len() == 1 => {
            route.directory_listing = match values[0] {
                "on" => true,
                "off" => false,
                _ => {
                    return Err(format!(
                        "Line {line_number}: directory_listing must be on or off"
                    ));
                }
            };
        }

        "cgi" if values.len() == 2 => {
            route.cgi = Some((values[0].to_string(), values[1].to_string()));
        }

        _ => return Err(format!("Line {line_number}: invalid route setting '{key}'")),
    }

    Ok(())
}

fn finish_server(mut server: ServerConfig) -> Result<ServerConfig, String> {
    validate_server(&server)?;

    if server.routes.is_empty() {
        let mut default_route = Route::new("/", &server.root);
        default_route.index = server.index.clone();
        server.routes.push(default_route);
    }

    Ok(server)
}

fn validate_server(server: &ServerConfig) -> Result<(), String> {
    if server.host.is_empty() {
        return Err("Server host is missing".to_string());
    }

    if server.ports.is_empty() {
        return Err("Server needs at least one port".to_string());
    }

    if server.root.is_empty() || !Path::new(&server.root).is_dir() {
        return Err(format!("Invalid root directory: {}", server.root));
    }

    if server.max_body_size == 0 || server.max_body_size > MAX_BODY_SIZE {
        return Err("Body size must be between 1 byte and 100 MB".to_string());
    }

    Ok(())
}

fn validate_route(route: &Route) -> Result<(), String> {
    if !route.path.starts_with('/') {
        return Err(format!("Route path must start with '/': {}", route.path));
    }

    if route.root.is_empty() || !Path::new(&route.root).is_dir() {
        return Err(format!("Invalid route root directory: {}", route.root));
    }

    if route.allowed_methods.is_empty() {
        return Err(format!("Route {} has no allowed methods", route.path));
    }

    if let Some((extension, interpreter)) = &route.cgi {
        if !extension.starts_with('.') || interpreter.is_empty() {
            return Err(format!(
                "Invalid CGI configuration for route {}",
                route.path
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_config_with_route() {
        let config = r#"
            server {
                host 127.0.0.1
                port 8080
                root .
                max_body_size 1024

                route /images {
                    methods GET
                    root .
                    index index.html
                    directory_listing off
                }
            }
        "#;

        let servers = parse_config(config).unwrap();

        assert_eq!(servers[0].routes.len(), 1);
        assert_eq!(servers[0].routes[0].path, "/images");
    }

    #[test]
    fn accepts_shared_ports() {
        let config = r#"
            server {
                host 127.0.0.1
                port 8080
                root .
            }

            server {
                host 127.0.0.1
                port 8080
                root .
            }
        "#;

        assert_eq!(parse_config(config).unwrap().len(), 2);
    }

    #[test]
    fn parses_multiple_server_names() {
        let servers = parse_config("server {\nhost 127.0.0.1\nport 8080\nroot .\nserver_name main.local www.main.local\nserver_name alias.local\n}").unwrap();
        assert_eq!(
            servers[0].server_names,
            ["main.local", "www.main.local", "alias.local"]
        );
    }

    #[test]
    fn rejects_empty_server_name() {
        assert!(
            parse_config("server {\nhost 127.0.0.1\nport 8080\nroot .\nserver_name\n}").is_err()
        );
    }

    #[test]
    fn rejects_invalid_root() {
        let config = r#"
            server {
                host 127.0.0.1
                port 8080
                root does-not-exist
            }
        "#;

        assert!(parse_config(config).is_err());
    }

    #[test]
    fn rejects_body_size_over_limit() {
        let config = r#"
            server {
                host 127.0.0.1
                port 8080
                root .
                max_body_size 999999999
            }
        "#;

        assert!(parse_config(config).is_err());
    }
}
