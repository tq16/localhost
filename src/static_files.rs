use std::fs;
use std::path::{Component, Path, PathBuf};

pub enum FileResult {
    Found {
        content_type: &'static str,
        body: Vec<u8>,
    },
    NotFound,
    Forbidden,
    ServerError,
}

pub fn get_file(root: &str, index: &str, url_path: &str) -> FileResult {
    let requested_path = if url_path == "/" {
        index
    } else {
        url_path.trim_start_matches('/')
    };

    let mut safe_path = PathBuf::new();

    for component in Path::new(requested_path).components() {
        match component {
            Component::Normal(part) => safe_path.push(part),
            _ => return FileResult::Forbidden,
        }
    }

    let full_path = Path::new(root).join(safe_path);

    if !full_path.exists() {
        return FileResult::NotFound;
    }

    if full_path.is_dir() {
        return FileResult::NotFound;
    }

    match fs::read(&full_path) {
        Ok(body) => FileResult::Found {
            content_type: content_type(&full_path),
            body,
        },
        Err(_) => FileResult::ServerError,
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}