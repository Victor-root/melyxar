//! Values the server carries so that nothing has to be set up to use it.
//!
//! The provider key identifies the application rather than the person running
//! it, which is how the provider intends it: one registration for Melyxar, and
//! nobody who installs it has anything to create. It travels scrambled rather
//! than written out, so it does not show up in a search of the sources nor in
//! the readable text of the binary. That is a hurdle, not a secret: anything
//! shipped with a program can be taken back out of it.

/// Where the scrambling starts. Any fixed number does.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Each byte sits behind a mask that changes with its position, so two equal
/// bytes do not come out equal here.
const SCRAMBLED: [u8; 32] = [
    0x25, 0xa9, 0xee, 0xec, 0x01, 0x21, 0x95, 0x40, 0x39, 0xa5, 0xb9, 0xe4, 0x19, 0x6f, 0xda, 0x5e,
    0x67, 0xed, 0x1a, 0xfb, 0x17, 0x60, 0xd7, 0x51, 0xcf, 0xb2, 0x11, 0xa0, 0x28, 0xc4, 0xe1, 0x6e,
];

const fn mask_at(position: usize) -> u8 {
    let from_seed = (SEED >> ((position % 8) * 8)) as u8;
    from_seed.wrapping_add((position as u8).wrapping_mul(31))
}

/// What the server uses to talk to the provider.
///
/// Answers nothing when the block above no longer decodes, which can only
/// happen if it was edited by hand. The server then says films cannot be
/// looked up, rather than sending nonsense and reporting a refusal that would
/// send whoever reads the logs looking in the wrong place.
pub fn provider_key() -> Option<String> {
    let rebuilt: Vec<u8> = SCRAMBLED
        .iter()
        .enumerate()
        .map(|(position, byte)| byte ^ mask_at(position))
        .collect();

    let value = String::from_utf8(rebuilt).ok()?;
    has_the_expected_shape(&value).then_some(value)
}

fn has_the_expected_shape(value: &str) -> bool {
    value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_comes_back_has_the_expected_shape() {
        assert!(has_the_expected_shape(
            &provider_key().expect("a value is carried")
        ));
    }

    #[test]
    fn nothing_readable_sits_in_the_block() {
        let value = provider_key().expect("a value is carried");
        let readable: String = SCRAMBLED.iter().map(|byte| *byte as char).collect();
        assert!(!readable.contains(&value));
    }

    #[test]
    fn two_equal_bytes_do_not_come_out_equal() {
        assert_ne!(mask_at(0), mask_at(1));
        assert_ne!(mask_at(8), mask_at(16));
    }

    #[test]
    fn a_block_that_no_longer_decodes_gives_nothing_rather_than_nonsense() {
        assert!(!has_the_expected_shape(""));
        assert!(!has_the_expected_shape("not of that shape at all"));
        assert!(!has_the_expected_shape(&"z".repeat(32)));
        assert!(has_the_expected_shape(&"a1".repeat(16)));
    }
}
