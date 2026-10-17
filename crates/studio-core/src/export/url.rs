/// `file://` URL for an absolute POSIX path, percent-encoding everything except RFC 3986
/// unreserved characters and `/`. Premiere and Resolve both relink from these.
pub fn file_url(path: &str, host: &str) -> String {
    let mut out = String::with_capacity(path.len() + 16);
    out.push_str("file://");
    out.push_str(host);
    if !path.starts_with('/') {
        out.push('/');
    }
    for byte in path.bytes() {
        let unreserved = byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte);
        if unreserved {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spaces_and_unicode() {
        assert_eq!(
            file_url("/Volumes/Shoot Day/clip é.mov", ""),
            "file:///Volumes/Shoot%20Day/clip%20%C3%A9.mov"
        );
        assert_eq!(
            file_url("/a/b.mov", "localhost"),
            "file://localhost/a/b.mov"
        );
    }
}
