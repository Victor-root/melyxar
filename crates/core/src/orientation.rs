//! Which way up a photo is meant to be seen.
//!
//! A camera held on its side writes its pixels the way its sensor sees them
//! and says in the file how to turn them. Anything that draws the pixels
//! without reading that note draws a portrait lying on its side.

/// How the pixels of a photo are to be turned before being looked at, in the
/// eight ways a camera can say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Drawn as stored, which is what a photo saying nothing means.
    #[default]
    AsStored,
    /// Mirrored left to right.
    Mirrored,
    /// Turned half a turn.
    UpsideDown,
    /// Mirrored top to bottom.
    MirroredUpsideDown,
    /// Mirrored across the line from its top left corner to its bottom right.
    Transposed,
    /// Turned a quarter turn clockwise: the camera was held on its left side.
    TurnedRight,
    /// Mirrored across the line from its top right corner to its bottom left.
    Transversed,
    /// Turned a quarter turn anticlockwise.
    TurnedLeft,
}

impl Orientation {
    /// Reads the number a camera writes, one to eight. Anything else is a
    /// photo drawn as stored: a number nobody defined says nothing.
    pub fn from_camera(value: u32) -> Self {
        match value {
            2 => Self::Mirrored,
            3 => Self::UpsideDown,
            4 => Self::MirroredUpsideDown,
            5 => Self::Transposed,
            6 => Self::TurnedRight,
            7 => Self::Transversed,
            8 => Self::TurnedLeft,
            _ => Self::AsStored,
        }
    }

    /// Whether turning it swaps its width and its height.
    pub fn swaps_its_sides(self) -> bool {
        matches!(
            self,
            Self::Transposed | Self::TurnedRight | Self::Transversed | Self::TurnedLeft
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_number_a_camera_writes_is_read_and_nothing_else_is_guessed() {
        assert_eq!(Orientation::from_camera(1), Orientation::AsStored);
        assert_eq!(Orientation::from_camera(6), Orientation::TurnedRight);
        assert_eq!(Orientation::from_camera(8), Orientation::TurnedLeft);
        assert_eq!(Orientation::from_camera(0), Orientation::AsStored);
        assert_eq!(Orientation::from_camera(9), Orientation::AsStored);
        assert!(Orientation::TurnedRight.swaps_its_sides());
        assert!(!Orientation::UpsideDown.swaps_its_sides());
    }
}
