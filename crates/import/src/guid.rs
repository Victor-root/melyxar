//! The identifiers Emby and Jellyfin give everything, written in several ways:
//! as sixteen bytes in the order .NET keeps them, its first three groups the
//! wrong way round, or as text with or without dashes, in either case.

use uuid::Uuid;

/// The thirty-two lower case hexadecimal digits of a GUID kept as .NET
/// writes it to a blob.
pub(crate) fn from_blob(bytes: &[u8]) -> Option<String> {
    Uuid::from_slice_le(bytes).ok().map(|guid| guid.simple().to_string())
}

/// The same for a GUID written as text, whichever way.
pub(crate) fn from_text(text: &str) -> Option<String> {
    Uuid::parse_str(text.trim()).ok().map(|guid| guid.simple().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blob_has_its_first_three_groups_turned_round() {
        let bytes = [
            0x67, 0x45, 0x23, 0x01, 0xab, 0x89, 0xef, 0xcd, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
        ];
        assert_eq!(from_blob(&bytes).as_deref(), Some("0123456789abcdef0123456789abcdef"));
        assert_eq!(from_blob(&bytes[..4]), None);
    }

    #[test]
    fn text_is_read_with_or_without_dashes_and_in_either_case() {
        let wanted = Some("0123456789abcdef0123456789abcdef".to_string());
        assert_eq!(from_text("01234567-89AB-CDEF-0123-456789ABCDEF"), wanted);
        assert_eq!(from_text("0123456789abcdef0123456789abcdef"), wanted);
        assert_eq!(from_text("not a guid"), None);
    }
}
