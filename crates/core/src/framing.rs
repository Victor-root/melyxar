//! Where to cut a wide picture so that a band of it still shows the film.
//!
//! A banner is a band across the top of a screen and a wide picture is nearly
//! twice as tall as that band. Something is always left out, and where the
//! band is taken from decides whether what is left is a film or two people cut
//! at the neck. One fixed place cannot serve both: a still framed with its
//! subject high wants the band high, and one framed with a sky above wants it
//! low, and the interface was wrong on one of them whichever it chose.
//!
//! So the place is read off each picture, once, when the picture is prepared.
//!
//! How: the picture is squeezed to a small grey copy, every pixel is compared
//! with the one beside it and the one above it, and what comes out is how busy
//! each row is. A sky, a wall, a field of grass out of focus: quiet. A face, a
//! hand, an edge of a coat, lettering: busy. The band is then put where it
//! holds the most of that, with a leaning towards the upper half, because a
//! picture of somebody is framed with their head above the middle and what is
//! below them is the floor.
//!
//! It is a rule of thumb and it is meant to be. It has no idea what a face is;
//! it knows where the detail is, which on a film still is nearly always the
//! same place. What it replaces is one number that was wrong half the time.

/// What a picture's framing answers: where the band is taken from, as a share
/// of the room there is to move it in.
///
/// Nought is the very top of the picture, one is the very bottom, and it is
/// written the way the interface reads it: the same number, out of a hundred,
/// is what a stylesheet is given to place the picture with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Framing(f64);

/// Where a band is taken from when nothing could be read: a little above the
/// middle, which is the best one number for a picture nobody has looked at.
pub const BY_DEFAULT: f64 = 0.38;

/// How tall the band is, as a share of the picture, for working this out.
///
/// The banner is about half the height of the picture behind it on the screens
/// this is drawn on. The exact share hardly matters: what comes out is where
/// the busy part is, and a band a little taller or shorter lands in the same
/// place.
const A_BAND_IS: f64 = 0.48;

/// How much the upper half is preferred, at the very top of the picture.
///
/// A still is framed with its subject above the middle and the floor below it,
/// so between two bands holding as much as each other the higher one is the
/// one somebody meant to look at.
///
/// Small on purpose, and measured: it is a share of the picture's whole
/// detail, so a twentieth only decides between bands that are already close.
/// At a third, tried first, a picture of somebody standing low in the frame
/// was dragged back up to the middle of them, which is the very fault this
/// exists to end.
const LEANING: f64 = 0.05;

impl Framing {
    /// The share, between nought and one.
    pub fn share(self) -> f64 {
        self.0
    }

    /// The same as a stylesheet writes it, out of a hundred.
    pub fn percent(self) -> f64 {
        self.0 * 100.0
    }

    /// Where a band is taken from when nothing could be read.
    pub fn by_default() -> Self {
        Self(BY_DEFAULT)
    }

    /// Reads it off a small grey copy of the picture, row after row.
    ///
    /// `across` and `down` are the size of that copy. Anything that is not a
    /// picture, or a picture too small to say anything, falls back to the one
    /// number above rather than inventing an answer.
    pub fn of_grey(grey: &[u8], across: usize, down: usize) -> Self {
        if across < 2 || down < 4 || grey.len() < across * down {
            return Self::by_default();
        }

        // How busy each row is: how much each pixel differs from the one
        // beside it and the one above it. Differences rather than brightness,
        // because a white wall is bright and holds nothing.
        let mut busy = vec![0f64; down];
        for y in 1..down {
            let mut total = 0f64;
            for x in 1..across {
                let here = grey[y * across + x] as f64;
                let left = grey[y * across + x - 1] as f64;
                let above = grey[(y - 1) * across + x] as f64;
                total += (here - left).abs() + (here - above).abs();
            }
            busy[y] = total;
        }
        // The first row has nothing above it, so it borrows what the second
        // found rather than counting as empty.
        busy[0] = busy[1];

        let everything: f64 = busy.iter().sum();
        if everything <= f64::EPSILON {
            return Self::by_default();
        }

        // Every place the band could be taken from, and how much of the busy
        // part each one holds. The band is a whole number of rows, and there
        // is at least one row of room to move it in.
        let band = ((down as f64 * A_BAND_IS).round() as usize).clamp(1, down - 1);
        let room = down - band;

        let mut best = 0usize;
        let mut best_worth = f64::MIN;
        for top in 0..=room {
            let held: f64 = busy[top..top + band].iter().sum();
            // What it holds is counted out of everything the picture holds,
            // so the leaning is a share of that too rather than of some
            // number of pixels, and it means the same on every picture.
            let worth = held / everything + LEANING * (1.0 - top as f64 / room as f64);
            if worth > best_worth {
                best_worth = worth;
                best = top;
            }
        }

        Self(best as f64 / room as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture with one busy band in it and nothing anywhere else, which is
    /// what a subject on a plain background looks like to this.
    fn with_detail_between(across: usize, down: usize, from: usize, to: usize) -> Vec<u8> {
        let mut grey = vec![40u8; across * down];
        for y in from..to {
            for x in 0..across {
                // A checker, which differs from everything around it.
                grey[y * across + x] = if (x + y) % 2 == 0 { 20 } else { 230 };
            }
        }
        grey
    }

    #[test]
    fn a_band_is_taken_where_the_picture_is_busy() {
        let across = 64;
        let down = 72;

        // A subject low in the frame: the band comes down to it.
        let low = Framing::of_grey(&with_detail_between(across, down, 48, 68), across, down);
        assert!(
            low.share() > 0.6,
            "a picture busy near its foot is shown from low down: {low:?}"
        );

        // A subject high in the frame, which is what a still of somebody
        // standing usually is: the band stays up.
        let high = Framing::of_grey(&with_detail_between(across, down, 4, 26), across, down);
        assert!(
            high.share() < 0.25,
            "a picture busy near its top is shown from the top: {high:?}"
        );

        assert!(
            high.share() < low.share(),
            "and the two never come out the same way round"
        );
    }

    #[test]
    fn a_picture_with_nothing_in_it_keeps_the_one_number() {
        let flat = vec![90u8; 64 * 72];
        assert_eq!(Framing::of_grey(&flat, 64, 72).share(), BY_DEFAULT);
    }

    #[test]
    fn anything_that_is_not_a_picture_keeps_the_one_number() {
        assert_eq!(Framing::of_grey(&[], 64, 72).share(), BY_DEFAULT);
        assert_eq!(Framing::of_grey(&[1, 2, 3], 64, 72).share(), BY_DEFAULT);
        assert_eq!(Framing::of_grey(&[1, 2, 3, 4], 2, 2).share(), BY_DEFAULT);
    }

    #[test]
    fn between_two_places_as_busy_as_each_other_the_higher_one_wins() {
        let across = 64;
        let down = 72;
        let mut both = with_detail_between(across, down, 6, 20);
        for (rank, value) in with_detail_between(across, down, 52, 66).iter().enumerate() {
            both[rank] = both[rank].max(*value);
        }

        let framing = Framing::of_grey(&both, across, down);
        assert!(
            framing.share() < 0.5,
            "a still is framed with its subject above the middle: {framing:?}"
        );
    }

    #[test]
    fn what_a_stylesheet_is_given_is_the_same_number_out_of_a_hundred() {
        assert_eq!(Framing::by_default().percent(), 38.0);
    }
}
