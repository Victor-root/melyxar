//! The mark of Melyxar in somebody's colour, for the icon of the installed
//! application.
//!
//! The page draws the icon of its tab itself, from the logo's relief laid over
//! a colour in hard light and cut to the logo's shape (see `web/src/mark.ts`).
//! An installed application is given the address of its icon rather than a
//! picture, and the system fetches it on its own; so the same drawing is made
//! here, from the same relief and with the same mix, for an address that
//! carries the colour. A server given a logo of its own wears that instead,
//! laid on the ground where a system cuts icons to a shape of its own.

use std::io::Cursor;

/// A colour, as the six hexadecimal digits the page writes it with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour([u8; 3]);

impl Colour {
    /// Reads six hexadecimal digits, without the hash in front of them.
    pub fn from_hex(digits: &str) -> Option<Self> {
        if digits.len() != 6 || !digits.bytes().all(|digit| digit.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some(Self([channel(0)?, channel(2)?, channel(4)?]))
    }

    /// Written back the way it was read, in small letters.
    pub fn hex(self) -> String {
        let [red, green, blue] = self.0;
        format!("{red:02x}{green:02x}{blue:02x}")
    }
}

/// Why no icon was drawn.
#[derive(Debug, thiserror::Error)]
pub enum MarkError {
    #[error("the relief could not be read: {0}")]
    Unreadable(#[from] png::DecodingError),
    #[error("the relief is not a grey picture with transparency")]
    NotARelief,
    #[error("the icon is not a colour picture with transparency")]
    NotAnIcon,
    #[error("the icon could not be written: {0}")]
    Unwritable(#[from] png::EncodingError),
}

/// Draws the logo in this colour from its relief, a grey picture of its light
/// and shade. With a ground, the logo is laid on it and the icon is a full
/// square; without one, everything around the logo stays transparent.
pub fn draw(relief: &[u8], colour: Colour, ground: Option<Colour>) -> Result<Vec<u8>, MarkError> {
    let mut decoder = png::Decoder::new(Cursor::new(relief));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let mut read = vec![0; reader.output_buffer_size().ok_or(MarkError::NotARelief)?];
    let frame = reader.next_frame(&mut read)?;
    if frame.color_type != png::ColorType::GrayscaleAlpha || frame.bit_depth != png::BitDepth::Eight
    {
        return Err(MarkError::NotARelief);
    }
    let shades = &read[..frame.buffer_size()];

    let mut drawn = Vec::with_capacity(shades.len() * 2);
    for pair in shades.chunks_exact(2) {
        drawn.extend_from_slice(&pixel(pair[0], pair[1], colour, ground));
    }

    written(frame.width, frame.height, &drawn)
}

/// Lays an icon with transparency on a ground, which makes it a full square:
/// what a system that cuts icons to a shape of its own is given.
pub fn laid_on(icon: &[u8], ground: Colour) -> Result<Vec<u8>, MarkError> {
    let mut reader = png::Decoder::new(Cursor::new(icon)).read_info()?;
    let mut read = vec![0; reader.output_buffer_size().ok_or(MarkError::NotAnIcon)?];
    let frame = reader.next_frame(&mut read)?;
    if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
        return Err(MarkError::NotAnIcon);
    }
    let mut laid = read[..frame.buffer_size()].to_vec();
    for pixel in laid.chunks_exact_mut(4) {
        let cover = f32::from(pixel[3]) / 255.0;
        for (index, channel) in ground.0.into_iter().enumerate() {
            let over = f32::from(pixel[index]) / 255.0;
            pixel[index] = to_byte(cover * over + (1.0 - cover) * f32::from(channel) / 255.0);
        }
        pixel[3] = u8::MAX;
    }
    written(frame.width, frame.height, &laid)
}

/// Writes pixels of colour and transparency as a picture.
fn written(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, MarkError> {
    let mut written = Vec::new();
    let mut encoder = png::Encoder::new(&mut written, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    writer.finish()?;
    Ok(written)
}

/// One pixel of the logo: the colour under the relief's shade in hard light,
/// as much of it as the relief covers, and the ground or nothing for the rest.
/// The sums a browser makes for the tab's icon, so the two agree.
fn pixel(shade: u8, cover: u8, colour: Colour, ground: Option<Colour>) -> [u8; 4] {
    if cover == 0 && ground.is_none() {
        return [0; 4];
    }
    let cover = f32::from(cover) / 255.0;
    let shade = f32::from(shade) / 255.0;
    let mut out = [0u8; 4];
    for (index, channel) in colour.0.into_iter().enumerate() {
        let under = f32::from(channel) / 255.0;
        let mixed = cover * hard_light(under, shade) + (1.0 - cover) * under;
        let laid = match ground {
            Some(ground) => cover * mixed + (1.0 - cover) * f32::from(ground.0[index]) / 255.0,
            None => mixed,
        };
        out[index] = to_byte(laid);
    }
    out[3] = if ground.is_some() {
        u8::MAX
    } else {
        to_byte(cover)
    };
    out
}

/// Hard light: the shade darkens the colour below the middle grey and lightens
/// it above, and leaves it as it is at the middle.
fn hard_light(under: f32, shade: f32) -> f32 {
    if shade <= 0.5 {
        under * 2.0 * shade
    } else {
        let lifted = 2.0 * shade - 1.0;
        under + lifted - under * lifted
    }
}

fn to_byte(share: f32) -> u8 {
    // Held to the range first, so the cast only ever drops the fraction.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = (share.clamp(0.0, 1.0) * 255.0).round() as u8;
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Colour = Colour([0xe5, 0x00, 0x00]);
    const NIGHT: Colour = Colour([0x0c, 0x0d, 0x10]);

    /// A relief of one row: fully covered pixels in black, middle grey and
    /// white, then one half covered and one not covered at all.
    fn a_relief() -> Vec<u8> {
        let shades = [0, 255, 128, 255, 255, 255, 128, 128, 77, 0];
        let mut written = Vec::new();
        let mut encoder = png::Encoder::new(&mut written, 5, 1);
        encoder.set_color(png::ColorType::GrayscaleAlpha);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&shades).expect("pixels");
        writer.finish().expect("finished");
        written
    }

    fn pixels_of(icon: &[u8]) -> Vec<u8> {
        let mut reader = png::Decoder::new(Cursor::new(icon))
            .read_info()
            .expect("an icon");
        let mut read = vec![0; reader.output_buffer_size().expect("a size")];
        let frame = reader.next_frame(&mut read).expect("a frame");
        assert_eq!(frame.color_type, png::ColorType::Rgba);
        read.truncate(frame.buffer_size());
        read
    }

    #[test]
    fn a_colour_is_six_hexadecimal_digits() {
        assert_eq!(Colour::from_hex("E50000"), Some(RED));
        assert_eq!(RED.hex(), "e50000");
        for wrong in ["", "#e50000", "e5000", "e500000", "e5000g", "é50000"] {
            assert_eq!(Colour::from_hex(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn the_shade_darkens_below_the_middle_grey_and_lightens_above_it() {
        let icon = draw(&a_relief(), RED, None).expect("drawn");
        assert_eq!(
            pixels_of(&icon),
            [
                0, 0, 0, 255, // black shade: black
                229, 1, 1, 255, // middle grey, a hair above the middle: the colour
                255, 255, 255, 255, // white shade: white
                229, 1, 1, 128, // half covered: the colour, half there
                0, 0, 0, 0, // not covered: nothing
            ]
        );
    }

    #[test]
    fn on_a_ground_the_icon_is_a_full_square() {
        let icon = draw(&a_relief(), RED, Some(NIGHT)).expect("drawn");
        let pixels = pixels_of(&icon);
        assert_eq!(&pixels[4..8], &[229, 1, 1, 255]);
        // Half the colour, half the ground.
        assert_eq!(&pixels[12..16], &[121, 7, 8, 255]);
        assert_eq!(&pixels[16..20], &[12, 13, 16, 255]);
    }

    #[test]
    fn an_icon_laid_on_a_ground_keeps_its_colours_and_loses_its_transparency() {
        let mut icon = Vec::new();
        let mut encoder = png::Encoder::new(&mut icon, 3, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer
            .write_image_data(&[229, 0, 0, 255, 229, 0, 0, 128, 1, 2, 3, 0])
            .expect("pixels");
        writer.finish().expect("finished");

        assert_eq!(
            pixels_of(&laid_on(&icon, NIGHT).expect("laid")),
            [
                229, 0, 0, 255, // covered: the icon
                121, 6, 8, 255, // half covered: half and half
                12, 13, 16, 255, // not covered: the ground
            ]
        );
    }

    #[test]
    fn a_picture_that_is_no_relief_is_refused() {
        let mut written = Vec::new();
        let mut encoder = png::Encoder::new(&mut written, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&[1, 2, 3]).expect("pixels");
        writer.finish().expect("finished");
        assert!(matches!(
            draw(&written, RED, None),
            Err(MarkError::NotARelief)
        ));
        assert!(matches!(
            draw(b"not a picture", RED, None),
            Err(MarkError::Unreadable(_))
        ));
    }
}
