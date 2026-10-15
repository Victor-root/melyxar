//! The server said to the home network, so the applications there find it on
//! their own: announced by mDNS / DNS-SD under `_melyxar._tcp`, the way
//! printers and speakers are found.
//!
//! The announcement runs on threads of its own, never on Tokio's, and is
//! withdrawn when the server stops. A network that will not have it costs a
//! line in the journal and nothing else: the address can always be typed.

use mdns_sd::{ServiceDaemon, ServiceInfo};

/// What the applications look for.
const SERVICE_TYPE: &str = "_melyxar._tcp.local.";

/// The name of the machine when it gives none.
const UNNAMED: &str = "melyxar";

/// The server as the home network knows it, withdrawn when this is dropped.
pub struct Announced {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for Announced {
    fn drop(&mut self) {
        // Said gone, so the applications stop offering it at once rather
        // than when what they heard runs out. Nothing left to do if the
        // network no longer hears it.
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Announces the server answering on `port`, under the name of this machine.
/// Nothing when the network refuses it.
pub fn announce(port: u16) -> Option<Announced> {
    let name = machine_name();
    let announced = ServiceDaemon::new().and_then(|daemon| {
        let host = format!("{}.local.", host_label(&name));
        let info = ServiceInfo::new(SERVICE_TYPE, &name, &host, "", port, None)?.enable_addr_auto();
        let fullname = info.get_fullname().to_owned();
        daemon.register(info)?;
        Ok(Announced { daemon, fullname })
    });
    match announced {
        Ok(announced) => {
            tracing::info!(port, name, "announced on the home network");
            Some(announced)
        }
        Err(failure) => {
            tracing::warn!(error = %failure, "the home network could not be told of the server: its address has to be typed");
            None
        }
    }
}

/// The name this machine gives itself.
fn machine_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| UNNAMED.to_owned())
}

/// A name as the home network writes the name of a machine: small letters,
/// digits and dashes, never starting or ending with a dash.
fn host_label(name: &str) -> String {
    let written: String = name
        .chars()
        .map(|letter| if letter.is_ascii_alphanumeric() { letter.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = written.trim_matches('-');
    if trimmed.is_empty() { UNNAMED.to_owned() } else { trimmed.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_is_named_as_the_home_network_writes_it() {
        assert_eq!(host_label("media-box"), "media-box");
        assert_eq!(host_label("Media Box"), "media-box");
        assert_eq!(host_label("_lxc_"), "lxc");
        assert_eq!(host_label("é"), UNNAMED);
    }
}
