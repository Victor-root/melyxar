//! What a camera's address may be, and the address an application opens it
//! at, account included.

/// Whether `address` is one a camera is watched at: RTSP, plain or over TLS,
/// naming a machine, and carrying no account of its own, which is kept apart
/// and sealed.
pub fn is_a_camera_address(address: &str) -> bool {
    let Some(rest) = address
        .strip_prefix("rtsp://")
        .or_else(|| address.strip_prefix("rtsps://"))
    else {
        return false;
    };
    let machine = rest.split(['/', '?']).next().unwrap_or_default();
    !machine.is_empty() && !machine.contains('@') && !address.chars().any(char::is_whitespace)
}

/// The address with its account written in, as RTSP readers take it: each
/// part escaped, so a password holding `@`, `:` or `/` is read as a password.
/// The address as it is for a camera opened without an account.
pub fn opened_with(address: &str, username: &str, password: Option<&str>) -> String {
    if username.is_empty() && password.is_none() {
        return address.to_string();
    }
    let Some((scheme, rest)) = address.split_once("://") else {
        return address.to_string();
    };
    let account = match password {
        Some(password) => format!("{}:{}", escaped(username), escaped(password)),
        None => escaped(username),
    };
    format!("{scheme}://{account}@{rest}")
}

/// Every byte but the few an address leaves as they are, written as `%XX`.
fn escaped(part: &str) -> String {
    part.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camera_is_watched_at_an_rtsp_address_naming_a_machine() {
        assert!(is_a_camera_address("rtsp://192.168.1.20:554/stream1"));
        assert!(is_a_camera_address("rtsps://door.home/live?channel=1"));
        assert!(!is_a_camera_address("http://192.168.1.20/stream"));
        assert!(!is_a_camera_address("rtsp:///stream"), "no machine");
        assert!(!is_a_camera_address("rtsp://viewer:secret@192.168.1.20/"), "its account is kept apart");
        assert!(!is_a_camera_address("rtsp://192.168.1.20/a stream"));
    }

    #[test]
    fn the_account_is_written_into_the_address_escaped() {
        assert_eq!(
            opened_with("rtsp://192.168.1.20:554/stream1", "viewer", Some("p@ss:w/rd é")),
            "rtsp://viewer:p%40ss%3Aw%2Frd%20%C3%A9@192.168.1.20:554/stream1"
        );
        assert_eq!(
            opened_with("rtsp://192.168.1.20/live", "viewer", None),
            "rtsp://viewer@192.168.1.20/live"
        );
        assert_eq!(opened_with("rtsp://192.168.1.20/live", "", None), "rtsp://192.168.1.20/live");
    }
}
