//! Where a voice starts, read from the sound of a song.
//!
//! A line of lyrics is stamped with the moment it starts being sung, and the
//! stamps of a file made on another version of a song, or timed by ear, are
//! often a fraction of a second off. What a voice starting does to the sound
//! is plain enough to measure without knowing anything of what is sung: in the
//! pitches a voice lives in, between about 300 and 3,400 hertz, something
//! arrives that was not there a moment before. Drums and bass arrive too, but
//! below that range, and the cymbals above it.
//!
//! So each hundredth of a second gets a number: how much the sound gained, in
//! the pitches of a voice, on what it was a few hundredths before. It is a
//! number about the sound and nothing else, which is why this needs no model:
//! it cannot tell a voice from a guitar that starts at the same moment, and it
//! does not try to. What lines the stamps up with it is `lyrics_alignment`.

use crate::transform::{Complex, Transform};

/// How many samples of one second of sound this reads, and therefore what the
/// caller has to hand it: the rate the tool is asked to write the song at.
pub const VOICE_SAMPLES_A_SECOND: u32 = 8_000;

/// How many numbers come out for one second of sound.
pub const ONSETS_A_SECOND: u32 = 100;

/// A window of sound, wide enough to tell the pitches of a voice apart.
const WINDOW: usize = 512;
/// Samples from the start of one window to the start of the next.
const HOP: usize = (VOICE_SAMPLES_A_SECOND / ONSETS_A_SECOND) as usize;
/// The pitches of a voice, in four bands, in hertz: the sound of one band
/// starting is a voice starting; the four together say it more surely.
const BANDS_HZ: [(f32, f32); 4] = [(300.0, 700.0), (700.0, 1_400.0), (1_400.0, 2_400.0), (2_400.0, 3_400.0)];
/// How far back a band is compared with, in frames: far enough back that the
/// slope of a note starting is not itself, near enough that it is the same note.
const BACK: [usize; 3] = [2, 3, 4];
/// How many frames before a note starts its window already shows it: a window
/// of 64 milliseconds that has half of the note in it stands 30 milliseconds
/// before the note. The numbers are given that much later so that each stands
/// where its start is, measured on a note with a sharp attack.
const READS_AHEAD: usize = 3;
/// How long after it starts a sound is looked at again, in frames: a voice or
/// an instrument holds its note this long, a drum hit has already died away.
const HELD_FOR: usize = 8;
/// Under this, in decibels, the voice bands together are silence and nothing
/// starts there, whatever the hiss does from one frame to the next.
const SILENT_DB: f32 = -70.0;
/// The least a band is taken to hold, in decibels.
const FLOOR_DB: f32 = -100.0;
/// Frames on each side that stand for what is usual around a moment: the second
/// before and the second after.
const AROUND: usize = ONSETS_A_SECOND as usize;
/// Added to what is usual before dividing by it, in decibels of change, so
/// that a nearly silent stretch does not turn its own breath into an onset.
const AT_LEAST_USUAL: f32 = 2.0;

/// How strongly a voice seems to start at each hundredth of a second, as a
/// number that is about one for what is usual around it and several for what
/// stands out. Only what lasts counts: a note starting is not a drum being
/// hit. One number for every `HOP` samples, the first at the start.
pub fn voice_onsets(samples: &[i16]) -> Vec<f32> {
    if samples.len() < WINDOW {
        return Vec::new();
    }
    let transform = Transform::of_size(WINDOW);
    let shape = hann();
    let edges = band_edges();
    let frames = (samples.len() - WINDOW) / HOP + 1;

    let mut levels = vec![[FLOOR_DB; BANDS_HZ.len()]; frames];
    let mut together = vec![FLOOR_DB; frames];
    let scale = 2.0 / (WINDOW as f32 / 2.0);
    for frame in 0..frames {
        let start = frame * HOP;
        let mut window: Vec<Complex> = (0..WINDOW)
            .map(|at| Complex::of(f32::from(samples[start + at]) / f32::from(i16::MAX) * shape[at]))
            .collect();
        transform.run(&mut window);
        let mut total = 0.0f32;
        for (band, (from, to)) in edges.iter().enumerate() {
            let pitches = &window[*from..*to];
            let power = pitches.iter().map(|pitch| pitch.strength_squared()).sum::<f32>() / pitches.len() as f32
                * scale
                * scale;
            total += power;
            levels[frame][band] = decibels(power);
        }
        together[frame] = decibels(total);
    }

    let mut gained = vec![0.0f32; frames];
    for frame in 0..frames {
        if together[frame] < SILENT_DB {
            continue;
        }
        let mut flux = 0.0f32;
        for (band, now) in levels[frame].iter().enumerate() {
            let before = BACK
                .iter()
                .map(|back| levels[frame.saturating_sub(*back)][band])
                .fold(FLOOR_DB, f32::max);
            let rise = (now - before).max(0.0);
            if rise > 0.0 {
                // How much of the rise is still there a moment later: all of
                // it for a note, little of it for a hit.
                let later = levels[(frame + HELD_FOR).min(frames - 1)][band] - before;
                flux += rise * (later / rise).clamp(0.0, 1.0);
            }
        }
        gained[frame] = flux;
    }
    let usual = against_what_is_usual(&gained);
    std::iter::repeat_n(0.0, READS_AHEAD).chain(usual).collect()
}

/// Each number over the mean of those around it, so that a loud passage full of
/// small starts and a quiet one with a single clear one are read alike.
fn against_what_is_usual(gained: &[f32]) -> Vec<f32> {
    let mut running = vec![0.0f64; gained.len() + 1];
    for (at, value) in gained.iter().enumerate() {
        running[at + 1] = running[at] + f64::from(*value);
    }
    (0..gained.len())
        .map(|at| {
            let from = at.saturating_sub(AROUND);
            let to = (at + AROUND + 1).min(gained.len());
            let usual = ((running[to] - running[from]) / (to - from) as f64) as f32;
            gained[at] / (usual + AT_LEAST_USUAL)
        })
        .collect()
}

fn decibels(power: f32) -> f32 {
    (10.0 * (power + 1e-12).log10()).max(FLOOR_DB)
}

fn hann() -> [f32; WINDOW] {
    let mut shape = [0.0f32; WINDOW];
    for (at, value) in shape.iter_mut().enumerate() {
        *value = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * at as f32 / WINDOW as f32).cos();
    }
    shape
}

/// The pitches of each band: where it starts and where it stops, in pitches of
/// the window, none narrower than one.
fn band_edges() -> Vec<(usize, usize)> {
    BANDS_HZ
        .iter()
        .map(|(from, to)| {
            let pitch = |hertz: f32| (hertz * WINDOW as f32 / VOICE_SAMPLES_A_SECOND as f32).round() as usize;
            (pitch(*from), pitch(*to).max(pitch(*from) + 1))
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A made up song: a bass note and a kick every half second under
    /// everything, and a voice, which is a note with its overtones, starting at
    /// each of these moments, in seconds, and lasting `how_long`.
    pub(crate) fn a_song(seconds: f32, voice_starts: &[f32], how_long: f32) -> Vec<i16> {
        a_song_with_guitar(seconds, voice_starts, &[], how_long)
    }

    /// The same, with a guitar playing its own notes, in the same pitches as
    /// the voice, starting at the moments `guitar_starts`.
    pub(crate) fn a_song_with_guitar(
        seconds: f32,
        voice_starts: &[f32],
        guitar_starts: &[f32],
        how_long: f32,
    ) -> Vec<i16> {
        let rate = VOICE_SAMPLES_A_SECOND as f32;
        let mut hiss = 12_345u32;
        (0..(seconds * rate) as usize)
            .map(|at| {
                let moment = at as f32 / rate;
                hiss = hiss.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = hiss as f32 / u32::MAX as f32 - 0.5;
                let bass = 0.25 * (2.0 * std::f32::consts::PI * 55.0 * moment).sin();
                let since_kick = moment % 0.5;
                let kick = 0.5 * (-since_kick * 25.0).exp() * (2.0 * std::f32::consts::PI * 70.0 * moment).sin();
                let voice: f32 = voice_starts
                    .iter()
                    .filter(|start| moment >= **start && moment < **start + how_long)
                    .map(|start| {
                        let age = moment - start;
                        let attack = (age / 0.02).min(1.0);
                        let fade = (1.0 - age / how_long).clamp(0.0, 1.0).sqrt();
                        (1..=8)
                            .map(|overtone| {
                                (2.0 * std::f32::consts::PI * 220.0 * overtone as f32 * moment).sin() / overtone as f32
                            })
                            .sum::<f32>()
                            * 0.25
                            * attack
                            * fade
                    })
                    .sum();
                let guitar: f32 = guitar_starts
                    .iter()
                    .filter(|start| moment >= **start && moment < **start + how_long)
                    .map(|start| {
                        let age = moment - start;
                        let fade = (1.0 - age / how_long).clamp(0.0, 1.0);
                        (1..=6)
                            .map(|overtone| {
                                (2.0 * std::f32::consts::PI * 330.0 * overtone as f32 * moment).sin() / overtone as f32
                            })
                            .sum::<f32>()
                            * 0.2
                            * (age / 0.01).min(1.0)
                            * fade
                    })
                    .sum();
                let sample = bass + kick + voice + guitar + 0.01 * noise;
                (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
            })
            .collect()
    }

    fn strongest_near(onsets: &[f32], moment: f32) -> usize {
        let centre = (moment * ONSETS_A_SECOND as f32) as usize;
        (centre.saturating_sub(15)..(centre + 15).min(onsets.len()))
            .max_by(|a, b| onsets[*a].total_cmp(&onsets[*b]))
            .unwrap_or(0)
    }

    #[test]
    fn a_voice_starting_stands_out_and_the_drums_under_it_do_not() {
        let starts = [2.0, 5.0, 8.0, 11.0];
        let onsets = voice_onsets(&a_song(14.0, &starts, 1.2));
        assert!(onsets.len() > 1_300 && onsets.len() < 1_410, "{}", onsets.len());
        for start in starts {
            let at = strongest_near(&onsets, start);
            let found = at as f32 / ONSETS_A_SECOND as f32;
            // Within a hundredth either way: the curve is one number a hundredth.
            assert!((found - start).abs() <= 0.011, "a voice starts at {start}, read at {found}");
            assert!(onsets[at] > 3.0, "{}", onsets[at]);
        }
        // The kicks fall every half second and are not voices.
        let kick_at = (3.5 * ONSETS_A_SECOND as f32) as usize;
        let loudest_kick = onsets[kick_at.saturating_sub(5)..kick_at + 10].iter().copied().fold(0.0, f32::max);
        let weakest_voice = starts.iter().map(|start| onsets[strongest_near(&onsets, *start)]).fold(f32::MAX, f32::min);
        assert!(loudest_kick < weakest_voice / 3.0, "a kick reads {loudest_kick}, a voice {weakest_voice}");
    }

    #[test]
    fn silence_and_too_little_sound_start_nothing() {
        assert!(voice_onsets(&[0; 100]).is_empty());
        let silence = voice_onsets(&vec![0; 16_000]);
        assert!(silence.iter().all(|value| *value == 0.0));
    }
}
