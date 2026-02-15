//! Request ID generation

use uuid::Uuid;

/// AWS-style request ID
#[derive(Debug, Clone)]
pub struct RequestId {
    /// Primary request ID (x-amz-request-id)
    pub id: String,
    /// Extended request ID (x-amz-id-2), base64 encoded
    pub extended_id: String,
}

impl RequestId {
    /// Generate a new request ID pair
    pub fn new() -> Self {
        let uuid = Uuid::new_v4();
        let id = uuid.simple().to_string().to_uppercase();

        // Extended ID is typically a base64-encoded value containing additional context
        // For simplicity, we use another UUID encoded
        let extended_uuid = Uuid::new_v4();
        let extended_id = base64_encode(&extended_uuid.as_bytes()[..]);

        Self { id, extended_id }
    }

    /// Create a request ID with a specific value (for testing)
    pub fn with_id(id: impl Into<String>) -> Self {
        let id = id.into();
        let extended_id = base64_encode(id.as_bytes());
        Self { id, extended_id }
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

fn base64_encode(data: &[u8]) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let mut encoder = base64::write::EncoderWriter::new(&mut buf, &base64::engine::general_purpose::STANDARD);
        encoder.write_all(data).unwrap();
    }
    String::from_utf8(buf).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_generation() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();

        // IDs should be unique
        assert_ne!(id1.id, id2.id);
        assert_ne!(id1.extended_id, id2.extended_id);

        // ID should be uppercase hex
        assert!(id1.id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_request_id_with_id() {
        let id = RequestId::with_id("test-id-123");
        assert_eq!(id.id, "test-id-123");
    }
}
