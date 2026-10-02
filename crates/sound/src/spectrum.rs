//! A song written down as how its sound is spread, a few times a second, small
//! enough to be kept with it and drawn behind the player.
//!
//! Nothing here has to be exact: it dresses a bar. What it has to be is cheap
//! to keep and honest about when the song is loud and which pitches carry it.
//! So the pitches are gathered into a couple of dozen bands, climbing the way
//! hearing does, each reading is one byte, and the whole is judged against the
//! song itself rather than against a fixed scale: a song mastered quietly moves
//! as much as one mastered loud, and a loud passage still stands above a quiet
//! one inside the same song.

use crate::transform::{Complex, Transform};

/// How many samples of one second of sound this reads.
///
/// Written twice, like the rate of the comparison of sound: the crate that
/// launches the tool and this one, which does arithmetic, have no business
/// knowing about each other. A test where both are in view holds them
/// together.
pub const SPECTRUM_SAMPLES_A_SECOND: u32 = 16_000;
/// How many bands the sound is spread over.
pub const SPECTRUM_BANDS: usize = 24;
/// How many readings a second.
pub const SPECTRUM_FRAMES_A_SECOND: u32 = 4;

const FRAME: usize = (SPECTRUM_SAMPLES_A_SECOND / SPECTRUM_FRAMES_A_SECOND) as usize;
/// Wide enough for the lowest band to hold at least one pitch.
const WINDOW: usize = 2_048;
/// Windows read within one frame, the last ending where the frame ends.
const WINDOWS_A_FRAME: usize = 3;
const LOWEST: f32 = 40.0;
const HIGHEST: f32 = 7_500.0;
/// What the sound of music loses at every octave, given back to the bands
/// above: without it the highs of every song would stay low.
const TILT_DB_AN_OCTAVE: f32 = 3.0;
/// How far under the song's own loud passages a band can still be told apart
/// from nothing.
const RANGE_DB: f32 = 40.0;
/// The share of the readings, each judged by its strongest band, that stand at
/// or under the song's reference, so that a click or two does not squash
/// everything else.
const REFERENCE_SHARE: f32 = 0.98;
/// A song whose reference is under this, in decibels under a full scale
/// sound, is silence and written as silence.
const SILENCE_DB: f32 = -75.0;

/// The levels of a song: `SPECTRUM_BANDS` bytes for each reading, readings in
/// the order they come, nought for nothing and 255 for as loud as the song
/// gets.
pub fn spectrum_of(samples: &[i16]) -> Vec<u8> {
    let frames = samples.len().div_ceil(FRAME);
    if frames == 0 {
        return Vec::new();
    }
    let transform = Transform::of_size(WINDOW);
    let edges = band_edges();
    let shape = window_shape();
    let tilts = tilts(&edges);

    let mut decibels = Vec::with_capacity(frames * SPECTRUM_BANDS);
    for frame in 0..frames {
        let mut power = [0.0f32; SPECTRUM_BANDS];
        for window in 0..WINDOWS_A_FRAME {
            let start = frame * FRAME + window * (FRAME - WINDOW) / (WINDOWS_A_FRAME - 1);
            add_power_of_window(samples, start, &transform, &shape, &edges, &mut power);
        }
        for (band, power) in power.iter().enumerate() {
            let mean = power / WINDOWS_A_FRAME as f32;
            decibels.push(10.0 * (mean + 1e-12).log10() + tilts[band]);
        }
    }

    let reference = reference_of(&decibels);
    if reference < SILENCE_DB {
        return vec![0; decibels.len()];
    }
    decibels
        .iter()
        .map(|level| {
            (((level - (reference - RANGE_DB)) / RANGE_DB).clamp(0.0, 1.0) * 255.0).round() as u8
        })
        .collect()
}

/// Adds to each band the mean strength of the pitches it holds in the window
/// that starts here; sound the song does not reach is silence.
fn add_power_of_window(
    samples: &[i16],
    start: usize,
    transform: &Transform,
    shape: &[f32; WINDOW],
    edges: &[usize; SPECTRUM_BANDS + 1],
    power: &mut [f32; SPECTRUM_BANDS],
) {
    let mut window: Vec<Complex> = (0..WINDOW)
        .map(|at| {
            let sample = samples.get(start + at).copied().unwrap_or(0);
            Complex::of(f32::from(sample) / f32::from(i16::MAX) * shape[at])
        })
        .collect();
    transform.run(&mut window);
    // A full scale note comes out as one: twice the strength over the sum of
    // the shape the window was read through.
    let scale = 2.0 / (WINDOW as f32 / 2.0);
    for (band, power) in power.iter_mut().enumerate() {
        let pitches = &window[edges[band]..edges[band + 1]];
        let strength: f32 = pitches.iter().map(|pitch| pitch.strength_squared()).sum();
        *power += strength / pitches.len() as f32 * scale * scale;
    }
}

/// How strong the strongest band is, for `REFERENCE_SHARE` of the readings at
/// or under it.
fn reference_of(decibels: &[f32]) -> f32 {
    let mut sorted: Vec<f32> = decibels
        .chunks(SPECTRUM_BANDS)
        .map(|reading| reading.iter().copied().fold(f32::MIN, f32::max))
        .collect();
    sorted.sort_by(f32::total_cmp);
    sorted[((sorted.len() - 1) as f32 * REFERENCE_SHARE) as usize]
}

/// Which pitches fall in which band: each the same multiple of the one below
/// it, and none narrower than one pitch of the window.
fn band_edges() -> [usize; SPECTRUM_BANDS + 1] {
    let spread = HIGHEST / LOWEST;
    let mut edges = [0usize; SPECTRUM_BANDS + 1];
    for band in 0..=SPECTRUM_BANDS {
        let frequency = LOWEST * spread.powf(band as f32 / SPECTRUM_BANDS as f32);
        let pitch = (frequency * WINDOW as f32 / SPECTRUM_SAMPLES_A_SECOND as f32).round() as usize;
        edges[band] = if band == 0 {
            pitch
        } else {
            pitch.max(edges[band - 1] + 1)
        };
    }
    edges
}

/// What each band is given back, from the middle pitch it holds.
fn tilts(edges: &[usize; SPECTRUM_BANDS + 1]) -> [f32; SPECTRUM_BANDS] {
    let mut tilts = [0.0f32; SPECTRUM_BANDS];
    for (band, tilt) in tilts.iter_mut().enumerate() {
        let middle = (edges[band] + edges[band + 1]) as f32 / 2.0;
        let frequency = middle * SPECTRUM_SAMPLES_A_SECOND as f32 / WINDOW as f32;
        *tilt = TILT_DB_AN_OCTAVE * (frequency / 1_000.0).log2();
    }
    tilts
}

/// The shape a window is read through, so that a note straddling two windows
/// is not read as a click by both of them.
fn window_shape() -> [f32; WINDOW] {
    let mut shape = [0.0f32; WINDOW];
    for (at, value) in shape.iter_mut().enumerate() {
        let turns = 2.0 * std::f32::consts::PI * at as f32 / (WINDOW - 1) as f32;
        *value = 0.5 - 0.5 * turns.cos();
    }
    shape
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A note of this pitch and loudness, for this many seconds.
    fn note(pitch: f32, loudness: f32, seconds: f32) -> Vec<i16> {
        let count = (seconds * SPECTRUM_SAMPLES_A_SECOND as f32) as usize;
        (0..count)
            .map(|at| {
                let turns = 2.0 * std::f32::consts::PI * pitch * at as f32
                    / SPECTRUM_SAMPLES_A_SECOND as f32;
                (turns.sin() * loudness * f32::from(i16::MAX)) as i16
            })
            .collect()
    }

    fn reading(levels: &[u8], frame: usize) -> &[u8] {
        &levels[frame * SPECTRUM_BANDS..(frame + 1) * SPECTRUM_BANDS]
    }

    fn strongest(reading: &[u8]) -> usize {
        reading
            .iter()
            .enumerate()
            .max_by_key(|(_, level)| **level)
            .map(|(band, _)| band)
            .expect("a reading has bands")
    }

    /// The band that holds this pitch.
    fn band_of(pitch: f32) -> usize {
        let edges = band_edges();
        let at = (pitch * WINDOW as f32 / SPECTRUM_SAMPLES_A_SECOND as f32).round() as usize;
        (0..SPECTRUM_BANDS)
            .find(|band| edges[*band] <= at && at < edges[*band + 1])
            .expect("the pitch is in a band")
    }

    #[test]
    fn a_note_lights_the_band_that_holds_its_pitch_and_not_the_far_ones() {
        for pitch in [100.0, 1_000.0, 5_000.0] {
            let levels = spectrum_of(&note(pitch, 0.5, 3.0));
            let middle = reading(&levels, 5);
            assert_eq!(strongest(middle), band_of(pitch), "{pitch} Hz: {middle:?}");
            let far = if band_of(pitch) < 12 {
                SPECTRUM_BANDS - 1
            } else {
                0
            };
            assert!(
                middle[far] < 40,
                "{pitch} Hz leaks into band {far}: {middle:?}"
            );
        }
    }

    #[test]
    fn it_gives_a_reading_every_quarter_of_a_second_and_counts_a_part_of_one() {
        let one_second = note(440.0, 0.5, 1.0);
        assert_eq!(spectrum_of(&one_second).len(), 4 * SPECTRUM_BANDS);
        assert_eq!(
            spectrum_of(&one_second[..one_second.len() - 10]).len(),
            4 * SPECTRUM_BANDS
        );
        assert_eq!(
            spectrum_of(&note(440.0, 0.5, 1.1)).len(),
            5 * SPECTRUM_BANDS
        );
        assert!(spectrum_of(&[]).is_empty());
    }

    #[test]
    fn silence_is_written_as_nothing_at_all() {
        let levels = spectrum_of(&vec![0i16; 3 * SPECTRUM_SAMPLES_A_SECOND as usize]);
        assert_eq!(levels.len(), 12 * SPECTRUM_BANDS);
        assert!(levels.iter().all(|level| *level == 0));
    }

    #[test]
    fn a_loud_passage_stands_above_a_quiet_one_of_the_same_song() {
        let mut song = note(1_000.0, 0.8, 2.0);
        song.extend(note(1_000.0, 0.08, 2.0));
        let levels = spectrum_of(&song);
        let loud = *reading(&levels, 3).iter().max().expect("bands");
        let quiet = *reading(&levels, 12).iter().max().expect("bands");
        assert!(loud > 200, "{loud}");
        assert!(quiet < loud - 60, "{quiet} against {loud}");
    }

    #[test]
    fn a_quiet_song_moves_as_much_as_a_loud_one() {
        let loud = spectrum_of(&note(1_000.0, 0.8, 2.0));
        let quiet = spectrum_of(&note(1_000.0, 0.05, 2.0));
        let top = |levels: &[u8]| *reading(levels, 3).iter().max().expect("bands");
        assert!(
            top(&quiet) > 200 && top(&loud) > 200,
            "{} and {}",
            top(&quiet),
            top(&loud)
        );
    }

    #[test]
    fn the_bands_climb_without_ever_being_empty() {
        let edges = band_edges();
        assert!(edges.windows(2).all(|pair| pair[0] < pair[1]), "{edges:?}");
        assert!(edges[SPECTRUM_BANDS] <= WINDOW / 2, "{edges:?}");
    }
}
