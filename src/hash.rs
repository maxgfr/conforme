use sha2::{Digest, Sha256};

/// Compute SHA-256 hash of content, returned as hex string.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Check if two contents are identical by hash.
pub fn contents_match(a: &str, b: &str) -> bool {
    content_hash(a) == content_hash(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_content_same_hash() {
        assert!(contents_match("hello world", "hello world"));
    }

    #[test]
    fn test_different_content_different_hash() {
        assert!(!contents_match("hello", "world"));
    }
}
