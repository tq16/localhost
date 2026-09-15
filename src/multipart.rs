pub struct UploadedFile {
    pub filename: String,
    pub data: Vec<u8>,
}

pub fn parse_multipart(body: &[u8], content_type: &str) -> Result<Vec<UploadedFile>, String> {
    let boundary = content_type
        .split(';')
        .find_map(|part| part.trim().strip_prefix("boundary="))
        .map(|value| value.trim_matches('"'))
        .ok_or("Multipart request has no boundary")?;

    let marker = format!("--{boundary}").into_bytes();

    if !body.starts_with(&marker) {
        return Err("Invalid multipart body".to_string());
    }

    let mut files = Vec::new();
    let mut position = 0;

    loop {
        if !body[position..].starts_with(&marker) {
            return Err("Invalid multipart boundary".to_string());
        }

        position += marker.len();

        if body[position..].starts_with(b"--") {
            break;
        }

        if !body[position..].starts_with(b"\r\n") {
            return Err("Invalid multipart separator".to_string());
        }

        position += 2;

        let headers_end =
            find_bytes(&body[position..], b"\r\n\r\n").ok_or("Multipart headers are incomplete")?;

        let headers_end = position + headers_end;

        let headers = std::str::from_utf8(&body[position..headers_end])
            .map_err(|_| "Multipart headers are invalid")?;

        position = headers_end + 4;

        let next_boundary = [b"\r\n".as_slice(), marker.as_slice()].concat();

        let data_end = find_bytes(&body[position..], &next_boundary)
            .ok_or("Multipart file data is incomplete")?;

        let data_end = position + data_end;
        let filename = get_filename(headers);

        if let Some(filename) = filename {
            files.push(UploadedFile {
                filename,
                data: body[position..data_end].to_vec(),
            });
        }

        position = data_end + 2;
    }

    if files.is_empty() {
        return Err("No file was included in the form".to_string());
    }

    Ok(files)
}

fn find_bytes(data: &[u8], needle: &[u8]) -> Option<usize> {
    data.windows(needle.len())
        .position(|window| window == needle)
}

fn get_filename(headers: &str) -> Option<String> {
    for line in headers.lines() {
        if !line
            .to_ascii_lowercase()
            .starts_with("content-disposition:")
        {
            continue;
        }

        for part in line.split(';') {
            let part = part.trim();

            if let Some(filename) = part.strip_prefix("filename=") {
                let filename = filename.trim_matches('"');

                if !filename.is_empty() {
                    return Some(filename.to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_file_from_multipart_body() {
        let body = b"--test\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\n\
Content-Type: text/plain\r\n\
\r\n\
Hello!\r\n\
--test--\r\n";

        let files = parse_multipart(body, "multipart/form-data; boundary=test").unwrap();

        assert_eq!(files[0].filename, "hello.txt");
        assert_eq!(files[0].data, b"Hello!");
    }
}
