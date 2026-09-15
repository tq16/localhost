use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

pub enum FileResult {
    Found {
        content_type: &'static str,
        body: Vec<u8>,
    },
    DirectoryListing {
        body: Vec<u8>,
    },
    NotFound,
    Forbidden,
    ServerError,
}

pub enum UploadResult {
    Saved,
    NotFound,
    Forbidden,
    ServerError,
}

pub fn get_file(
    root: &str,
    index: &str,
    url_path: &str,
    directory_listing: bool,
) -> FileResult {
    let requested_path = if url_path == "/" {
        index
    } else {
        url_path.trim_start_matches('/')
    };

    let safe_requested_path = match safe_path(requested_path) {
        Ok(path) => path,
        Err(()) => return FileResult::Forbidden,
    };

    let full_path = Path::new(root).join(safe_requested_path);

    if full_path.is_file() {
        return match fs::read(&full_path) {
            Ok(body) => FileResult::Found {
                content_type: content_type(&full_path),
                body,
            },
            Err(_) => FileResult::ServerError,
        };
    }

    if directory_listing {
        let directory_path = if url_path == "/" {
            PathBuf::from(root)
        } else {
            let safe_directory_path = match safe_path(url_path.trim_start_matches('/')) {
                Ok(path) => path,
                Err(()) => return FileResult::Forbidden,
            };

            Path::new(root).join(safe_directory_path)
        };

        if directory_path.is_dir() {
            return match create_directory_listing(&directory_path) {
                Ok(body) => FileResult::DirectoryListing {
                    body: body.into_bytes(),
                },
                Err(_) => FileResult::ServerError,
            };
        }
    }

    FileResult::NotFound
}

pub fn save_file(root: &str, url_path: &str, body: &[u8]) -> UploadResult {
    let requested_path = url_path.trim_start_matches('/');

    let safe_path = match safe_path(requested_path) {
        Ok(path) => path,
        Err(()) => return UploadResult::Forbidden,
    };

    let full_path = Path::new(root).join(safe_path);

    if let Some(parent) = full_path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return UploadResult::ServerError;
        }
    }

    match fs::write(full_path, body) {
        Ok(()) => UploadResult::Saved,
        Err(_) => UploadResult::ServerError,
    }
}

pub fn delete_file(root: &str, url_path: &str) -> UploadResult {
    let requested_path = url_path.trim_start_matches('/');

    let safe_path = match safe_path(requested_path) {
        Ok(path) => path,
        Err(()) => return UploadResult::Forbidden,
    };

    let full_path = Path::new(root).join(safe_path);

    match fs::remove_file(full_path) {
        Ok(()) => UploadResult::Saved,
        Err(error) if error.kind() == ErrorKind::NotFound => UploadResult::NotFound,
        Err(_) => UploadResult::ServerError,
    }
}

fn safe_path(requested_path: &str) -> Result<PathBuf, ()> {
    if requested_path.is_empty() {
        return Err(());
    }

    let mut result = PathBuf::new();

    for component in Path::new(requested_path).components() {
        match component {
            Component::Normal(part) => result.push(part),
            _ => return Err(()),
        }
    }

    Ok(result)
}

fn create_directory_listing(directory: &Path) -> Result<String, ()> {
    let mut names = Vec::new();

    for entry in fs::read_dir(directory).map_err(|_| ())? {
        let entry = entry.map_err(|_| ())?;
        let name = entry.file_name().to_string_lossy().to_string();
        names.push(name);
    }

    names.sort();

    let mut html = String::from(
        "<!doctype html>\
         <html>\
         <head><title>Directory listing</title></head>\
         <body>\
         <h1>Directory listing</h1>\
         <ul>",
    );

    for name in names {
        let visible_name = escape_html(&name);
        let link_name = url_encode(&name);

        html.push_str(&format!(
            "<li><a href=\"./{link_name}\">{visible_name}</a></li>"
        ));
    }

    html.push_str("</ul></body></html>");

    Ok(html)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_directory_paths() {
        assert!(safe_path("../Cargo.toml").is_err());
    }

    #[test]
    fn escapes_html_in_file_names() {
        assert_eq!(escape_html("<file>.txt"), "&lt;file&gt;.txt");
    }
}