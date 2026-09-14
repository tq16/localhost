use std::collections::HashSet;
use std::fs;
use std::path::Path;

const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub ports: Vec<u16>,
    pub root: String,
    pub index: String,
    pub max_body_size: usize,
}

impl ServerConfig {
    fn new() -> Self {
        Self {
            host: String::new(),
            ports: Vec::new(),
            root: String::new(),
            index: "index.html".to_string(),
            max_body_size: 1024 * 1024,
        }
    }
}

pub fn load_config(path: &str) -> Result<Vec<ServerConfig>, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Could not read config file: {error}"))?;

    parse_config(&content)
}

pub fn parse_config(content: &str) -> Result<Vec<ServerConfig>, String> {
    let mut servers = Vec::new();
    let mut current_server: Option<ServerConfig> = None;

    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index + 1;
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

        if line == "}" {
            let server = current_server
                .take()
                .ok_or_else(|| format!("Line {line_number}: unexpected closing brace"))?;

            validate_server(&server)?;
            servers.push(server);
            continue;
        }

        let server = current_server
            .as_mut()
            .ok_or_else(|| format!("Line {line_number}: setting must be inside server block"))?;

        let mut parts = line.split_whitespace();
        let key = parts
            .next()
            .ok_or_else(|| format!("Line {line_number}: invalid setting"))?;

        let value = parts
            .next()
            .ok_or_else(|| format!("Line {line_number}: missing value for {key}"))?;

        if parts.next().is_some() {
            return Err(format!("Line {line_number}: too many values"));
        }

        match key {
            "host" => server.host = value.to_string(),

            "port" => {
                let port = value
                    .parse::<u16>()
                    .map_err(|_| format!("Line {line_number}: invalid port"))?;

                if port == 0 {
                    return Err(format!("Line {line_number}: port cannot be 0"));
                }

                server.ports.push(port);
            }

            "root" => server.root = value.to_string(),

            "index" => server.index = value.to_string(),

            "max_body_size" => {
                server.max_body_size = value
                    .parse::<usize>()
                    .map_err(|_| format!("Line {line_number}: invalid body size"))?;
            }

            _ => return Err(format!("Line {line_number}: unknown setting '{key}'")),
        }
    }

    if current_server.is_some() {
        return Err("Missing closing brace in config file".to_string());
    }

    if servers.is_empty() {
        return Err("Config file has no server blocks".to_string());
    }

    validate_no_port_conflicts(&servers)?;

    Ok(servers)
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

fn validate_no_port_conflicts(servers: &[ServerConfig]) -> Result<(), String> {
    let mut used_addresses = HashSet::new();

    for server in servers {
        for port in &server.ports {
            let address = format!("{}:{port}", server.host);

            if !used_addresses.insert(address.clone()) {
                return Err(format!("Conflicting server address: {address}"));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_config() {
        let config = r#"
            server {
                host 127.0.0.1
                port 8080
                root .
                index index.html
                max_body_size 1024
            }
        "#;

        assert!(parse_config(config).is_ok());
    }

    #[test]
    fn rejects_conflicting_ports() {
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

        assert!(parse_config(config).is_err());
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