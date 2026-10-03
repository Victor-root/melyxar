//! How busy an NVIDIA card is, asked of NVIDIA's management library.
//!
//! NVIDIA's driver keeps no tally the kernel shows for work done through its
//! own interface. The library its own `nvidia-smi` is built on is installed
//! with the driver, so it is there wherever such a card converts films, and
//! asking it is one call where `nvidia-smi` would be a program launched at
//! every reading. It sees the whole card, not only this server's work.

use nvml_wrapper::Nvml;

/// NVIDIA's library, loaded and kept.
pub struct Nvidia(Nvml);

impl Nvidia {
    /// Loads the library. Nothing when the driver did not install it.
    pub fn open() -> Option<Self> {
        Nvml::init().ok().map(Self)
    }

    /// The share of the busiest engine of the card at this slot (computing,
    /// encoding or decoding), from nought to one, over the last stretch the
    /// driver sampled. Nothing when the card cannot be asked.
    pub fn busy_share(&self, slot: &str) -> Option<f64> {
        let card = self.0.device_by_pci_bus_id(slot).ok()?;
        Some(busiest(&[
            card.utilization_rates().ok()?.gpu,
            card.encoder_utilization().map_or(0, |reading| reading.utilization),
            card.decoder_utilization().map_or(0, |reading| reading.utilization),
        ]))
    }
}

/// The busiest of several engines given in percent, as a share. The busiest
/// rather than an average: an encoder kept full is a card that cannot take
/// another film, whatever the rest of it does.
fn busiest(percents: &[u32]) -> f64 {
    (f64::from(percents.iter().copied().max().unwrap_or(0)) / 100.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_busiest_engine_is_the_card_s_share() {
        assert_eq!(busiest(&[12, 87, 30]), 0.87);
        assert_eq!(busiest(&[0, 0, 0]), 0.0);
        assert_eq!(busiest(&[140]), 1.0, "never more than the whole card");
    }
}
