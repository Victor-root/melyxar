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
//! What is looked for is skin. A small colour copy of the picture is made and
//! every pixel is asked whether it could be a face: warm, more red than green
//! and more green than blue, and inside the narrow band of hues that skin of
//! every colour shares once brightness is set aside. Where those pixels are is
//! where the people are, and a still of a film is nearly always a still of
//! people. The band is then placed to hold them, a little above the middle,
//! the way somebody framing a photograph would.
//!
//! This replaces counting how much each row of the picture differs from
//! itself, which was tried first and was worse than the one fixed number it
//! was meant to improve on. Detail is not the subject: a field of grass, a
//! burnt circle of ground, a collapsing city, lettering, all hold far more of
//! it than a face does, so the band went to the busiest corner of the picture
//! and left the heads above it. Measured over a shelf of real pictures, the
//! band went to the cape rather than the man, to the fur rather than the face,
//! and to the ground rather than the people standing on it.
//!
//! Three things guard it, each of which was a picture that came out wrong:
//!
//! - Too little of that colour and there is nothing to go on. A night scene,
//!   an empty throne, a spacecraft: those fall back to the old reading of
//!   where the detail is, which is no worse there than anything else.
//! - Too much of it and it is not skin at all but a warm picture: a pink
//!   hotel, a sepia poster, a wall of sand. Over about a third of the picture
//!   nothing is a face any more, and the same fallback is taken.
//! - A single row more than half full of it is a wall or a desert seen edge
//!   on, not a row of faces, so that row is set aside before the rest are
//!   read. This is what stops a picture of a dune sea from pulling the band
//!   down into the sand and cutting the cast off at the top.
//!
//! It knows nothing about faces and it is not meant to. It knows where warm
//! skin-coloured pixels are, which on a film still is nearly always the same
//! place, and it is right far more often than any single number could be.

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
/// the people are, and a band a little taller or shorter lands in the same
/// place.
const A_BAND_IS: f64 = 0.48;

/// Below this share of the picture, what was found is a few stray pixels
/// rather than anybody, and the picture is read for its detail instead.
const ENOUGH_SKIN: f64 = 0.004;

/// Above this share, the whole picture is that colour and none of it is a
/// face. A hotel painted pink reached seven tenths, a poster laid over sand
/// three quarters, and both were placed properly by the fallback.
const TOO_MUCH_SKIN: f64 = 0.35;

/// A row this full of it, across the picture, is a surface rather than a row
/// of people, and is set aside before the rest are read.
const A_CROWDED_ROW: f64 = 0.50;

/// Where the middle of what was found should sit in the band, from its top.
///
/// A little above the middle, because a face sits above the body it belongs
/// to and the room under it is worth less than the room over it.
const WHERE_THE_FACES_SIT: f64 = 0.42;

/// How the room left over is shared out when everything found fits inside the
/// band: this much of it above, the rest below.
const ROOM_ABOVE: f64 = 0.35;

/// How much room is kept over the topmost of it when there is more than the
/// band can hold, so that the highest head keeps its hair.
const HEADROOM: f64 = 0.08;

/// How much the upper half is preferred when the picture is read for its
/// detail, which is what happens when no skin was found.
///
/// A still is framed with its subject above the middle and the floor below it,
/// so between two bands holding as much as each other the higher one is the
/// one somebody meant to look at. Small on purpose and measured: at a third,
/// tried first, a picture of somebody standing low in the frame was dragged
/// back up to the middle of them.
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

    /// Reads it off a small colour copy of the picture, three bytes a pixel,
    /// red then green then blue, row after row.
    ///
    /// `across` and `down` are the size of that copy. Anything that is not a
    /// picture, or a picture too small to say anything, falls back to the one
    /// number above rather than inventing an answer.
    pub fn of_picture(pixels: &[u8], across: usize, down: usize) -> Self {
        if across < 2 || down < 4 || pixels.len() < across * down * 3 {
            return Self::by_default();
        }

        // One pass over the picture answers both questions: how much of each
        // row could be skin, and how bright each pixel is for the fallback.
        let mut found = vec![0f64; down];
        let mut grey = vec![0u8; across * down];
        for y in 0..down {
            for x in 0..across {
                let at = (y * across + x) * 3;
                let (red, green, blue) = (pixels[at], pixels[at + 1], pixels[at + 2]);
                grey[y * across + x] = brightness_of(red, green, blue);
                if could_be_skin(red, green, blue) {
                    found[y] += 1.0;
                }
            }
        }

        let everywhere = (across * down) as f64;
        let all_of_it: f64 = found.iter().sum();
        // Measured before the crowded rows are set aside: a picture that is
        // warm all over is one where none of this means anything, and taking
        // its crowded rows out first would hide exactly that.
        if all_of_it < ENOUGH_SKIN * everywhere || all_of_it > TOO_MUCH_SKIN * everywhere {
            return Self(by_detail(&grey, across, down));
        }

        let crowded = A_CROWDED_ROW * across as f64;
        for row in found.iter_mut() {
            if *row > crowded {
                *row = 0.0;
            }
        }
        if found.iter().sum::<f64>() < ENOUGH_SKIN * everywhere {
            return Self(by_detail(&grey, across, down));
        }

        Self(by_skin(&found, down))
    }
}

/// How bright a colour is, the way a screen weighs it.
fn brightness_of(red: u8, green: u8, blue: u8) -> u8 {
    let (red, green, blue) = (red as f64, green as f64, blue as f64);
    (0.299 * red + 0.587 * green + 0.114 * blue).round() as u8
}

/// Whether a pixel could belong to a face.
///
/// Brightness is set aside and only the hue is asked about, which is what lets
/// one rule cover skin of every colour: what differs between them is how dark
/// they are, not where they sit between red and blue. The two checks after it
/// throw out what is the right hue without being skin at all: anything grey,
/// and anything that is not warmer than it is green nor greener than it is
/// blue.
fn could_be_skin(red: u8, green: u8, blue: u8) -> bool {
    if !(red > green && green > blue) {
        return false;
    }
    if red - blue < 18 {
        return false;
    }

    let (r, g, b) = (red as f64, green as f64, blue as f64);
    let brightness = 0.299 * r + 0.587 * g + 0.114 * b;
    if brightness <= 40.0 {
        return false;
    }

    let towards_blue = 128.0 - 0.168_736 * r - 0.331_264 * g + 0.5 * b;
    let towards_red = 128.0 + 0.5 * r - 0.418_688 * g - 0.081_312 * b;
    (77.0..=133.0).contains(&towards_blue) && (133.0..=177.0).contains(&towards_red)
}

/// The row at which the given share of everything found has been passed.
fn where_it_reaches(rows: &[f64], share: f64) -> usize {
    let wanted: f64 = rows.iter().sum::<f64>() * share;
    let mut run = 0f64;
    for (y, row) in rows.iter().enumerate() {
        run += row;
        if run >= wanted {
            return y;
        }
    }
    rows.len().saturating_sub(1)
}

/// Puts the band over what was found, aiming at two things at once.
///
/// One aim wants the middle of it a little above the middle of the band, which
/// is right for a row of people spread across the picture. The other wants the
/// top of it far enough down to leave hair and hat above, which is right for a
/// face filling the frame. Either one alone is wrong on the other kind of
/// picture, and halfway between them was never wrong on any of the pictures
/// this was measured against.
fn by_skin(found: &[f64], down: usize) -> f64 {
    let band = ((down as f64 * A_BAND_IS).round() as usize).clamp(1, down - 1);
    let room = (down - band) as f64;
    let band = band as f64;

    let middle = where_it_reaches(found, 0.50) as f64 + 0.5;
    let top = where_it_reaches(found, 0.10) as f64;
    let foot = where_it_reaches(found, 0.90) as f64;
    let reach = foot - top;

    let by_middle = middle - band * WHERE_THE_FACES_SIT;
    let by_top = match reach <= band {
        true => top - (band - reach) * ROOM_ABOVE,
        false => top - band * HEADROOM,
    };

    ((by_middle + by_top) / 2.0).clamp(0.0, room) / room
}

/// Where the band goes when no face could be found: over the busiest part of
/// the picture, which is the best guess left once the subject is unknown.
fn by_detail(grey: &[u8], across: usize, down: usize) -> f64 {
    // How busy each row is: how much each pixel differs from the one beside it
    // and the one above it. Differences rather than brightness, because a
    // white wall is bright and holds nothing.
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
    // The first row has nothing above it, so it borrows what the second found
    // rather than counting as empty.
    busy[0] = busy[1];

    let everything: f64 = busy.iter().sum();
    if everything <= f64::EPSILON {
        return BY_DEFAULT;
    }

    let band = ((down as f64 * A_BAND_IS).round() as usize).clamp(1, down - 1);
    let room = down - band;

    let mut best = 0usize;
    let mut best_worth = f64::MIN;
    for top in 0..=room {
        let held: f64 = busy[top..top + band].iter().sum();
        // What it holds is counted out of everything the picture holds, so the
        // leaning is a share of that too rather than of some number of pixels,
        // and it means the same on every picture.
        let worth = held / everything + LEANING * (1.0 - top as f64 / room as f64);
        if worth > best_worth {
            best_worth = worth;
            best = top;
        }
    }

    best as f64 / room as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACROSS: usize = 96;
    const DOWN: usize = 108;

    /// A colour that passes for skin, and one that cannot.
    const SKIN: [u8; 3] = [200, 150, 120];
    const STONE: [u8; 3] = [90, 90, 90];

    fn filled_with(colour: [u8; 3]) -> Vec<u8> {
        colour.repeat(ACROSS * DOWN)
    }

    fn paint(picture: &mut [u8], rows: std::ops::Range<usize>, wide: usize, colour: [u8; 3]) {
        for y in rows {
            for x in 0..wide {
                let at = (y * ACROSS + x) * 3;
                picture[at..at + 3].copy_from_slice(&colour);
            }
        }
    }

    /// A picture with detail in one band of it and nothing anywhere else,
    /// which is what a subject on a plain background looks like to the
    /// fallback.
    fn with_detail_between(from: usize, to: usize) -> Vec<u8> {
        let mut picture = filled_with([40, 40, 40]);
        for y in from..to {
            for x in 0..ACROSS {
                let at = (y * ACROSS + x) * 3;
                // A checker, which differs from everything around it, and grey
                // so that none of it passes for skin.
                let value = if (x + y) % 2 == 0 { 20 } else { 230 };
                picture[at..at + 3].copy_from_slice(&[value, value, value]);
            }
        }
        picture
    }

    #[test]
    fn a_band_is_taken_where_the_faces_are() {
        // Faces high in the frame, which is what a still of somebody standing
        // usually is: the band stays up.
        let mut high = filled_with(STONE);
        paint(&mut high, 10..20, 20, SKIN);
        let high = Framing::of_picture(&high, ACROSS, DOWN);

        // Faces low in the frame: the band comes down to them.
        let mut low = filled_with(STONE);
        paint(&mut low, 80..90, 20, SKIN);
        let low = Framing::of_picture(&low, ACROSS, DOWN);

        assert!(high.share() < 0.25, "faces near the top stay up: {high:?}");
        assert!(low.share() > 0.6, "faces near the foot come down: {low:?}");
    }

    #[test]
    fn a_face_filling_the_frame_is_kept_rather_than_the_neck_under_it() {
        // A close up, shaped like one: a face across the upper third and a
        // neck and shoulders running from it down to the foot. There is more
        // skin here than the band can hold, and what has to be kept is the
        // face.
        const FACE: std::ops::Range<usize> = 24..56;
        let mut close = filled_with(STONE);
        paint(&mut close, FACE, 44, SKIN);
        paint(&mut close, 56..DOWN, 14, SKIN);
        let close = Framing::of_picture(&close, ACROSS, DOWN);

        let band = (DOWN as f64 * A_BAND_IS).round();
        let top = close.share() * (DOWN as f64 - band);
        let held = (top + band).min(FACE.end as f64) - top.max(FACE.start as f64);
        assert!(
            held >= (FACE.end - FACE.start) as f64 * 0.85,
            "the band holds nearly all of the face: {close:?}"
        );
    }

    #[test]
    fn a_picture_warm_all_over_is_not_a_picture_of_faces() {
        // A wall painted the colour of skin. Read as faces it would centre the
        // band on the wall; it has to fall back instead, which on a picture
        // with nothing in it is the one number.
        let wall = filled_with(SKIN);
        assert_eq!(Framing::of_picture(&wall, ACROSS, DOWN).share(), BY_DEFAULT);
    }

    #[test]
    fn a_band_of_sand_does_not_pull_the_band_down_to_it() {
        // Faces along the top and a desert filling the lower half, which is
        // the shape of picture that sent the band into the sand and cut the
        // cast off above it.
        let mut desert = filled_with(STONE);
        paint(&mut desert, 60..DOWN, ACROSS, SKIN);
        paint(&mut desert, 12..22, 24, SKIN);

        let framing = Framing::of_picture(&desert, ACROSS, DOWN);
        assert!(
            framing.share() < 0.3,
            "the rows full of it are set aside and the faces decide: {framing:?}"
        );
    }

    #[test]
    fn a_picture_with_nobody_in_it_is_read_for_its_detail() {
        let low = Framing::of_picture(&with_detail_between(72, 102), ACROSS, DOWN);
        let high = Framing::of_picture(&with_detail_between(6, 36), ACROSS, DOWN);

        assert!(high.share() < 0.25, "detail near the top: {high:?}");
        assert!(low.share() > 0.6, "detail near the foot: {low:?}");
    }

    #[test]
    fn between_two_places_as_busy_as_each_other_the_higher_one_wins() {
        let mut both = with_detail_between(9, 30);
        for (rank, value) in with_detail_between(78, 99).iter().enumerate() {
            both[rank] = both[rank].max(*value);
        }

        let framing = Framing::of_picture(&both, ACROSS, DOWN);
        assert!(
            framing.share() < 0.5,
            "a still is framed with its subject above the middle: {framing:?}"
        );
    }

    #[test]
    fn a_picture_with_nothing_in_it_keeps_the_one_number() {
        let flat = filled_with([90, 90, 90]);
        assert_eq!(Framing::of_picture(&flat, ACROSS, DOWN).share(), BY_DEFAULT);
    }

    #[test]
    fn anything_that_is_not_a_picture_keeps_the_one_number() {
        assert_eq!(Framing::of_picture(&[], ACROSS, DOWN).share(), BY_DEFAULT);
        assert_eq!(
            Framing::of_picture(&[1, 2, 3], ACROSS, DOWN).share(),
            BY_DEFAULT
        );
        assert_eq!(
            Framing::of_picture(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], 2, 2).share(),
            BY_DEFAULT
        );
    }

    #[test]
    fn skin_of_every_colour_is_read_the_same_way() {
        // The same hue at four brightnesses. All of them are faces.
        for shade in [[255, 205, 175], [225, 170, 140], [160, 105, 75], [110, 70, 50]] {
            assert!(could_be_skin(shade[0], shade[1], shade[2]), "{shade:?}");
        }
    }

    #[test]
    fn what_is_not_a_face_is_not_taken_for_one() {
        // Grey, a clear sky, grass, and a lit screen.
        for colour in [[90, 90, 90], [70, 130, 200], [60, 120, 50], [230, 230, 250]] {
            assert!(
                !could_be_skin(colour[0], colour[1], colour[2]),
                "{colour:?}"
            );
        }
    }

    #[test]
    fn what_a_stylesheet_is_given_is_the_same_number_out_of_a_hundred() {
        assert_eq!(Framing::by_default().percent(), 38.0);
    }
}
