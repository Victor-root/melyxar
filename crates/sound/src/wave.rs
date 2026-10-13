//! A song written down as how loud it is a hundred times a second, for the
//! bar a song is cut on: fine enough to find the end of a silence by eye once
//! the bar is zoomed in on a second of it.
//!
//! Each moment is the loudest sample in it, in decibels under a full scale
//! sound and laid out over a fixed range, so that a quiet fade still shows
//! and true silence lies flat. Unlike the spectrum, nothing is judged against
//! the rest of the song: a silence is a silence however loud the song is.

/// How many samples of one second of sound this reads: the rate the tool
/// hands the spectrum, so both are read from the same output.
const WAVE_SAMPLES_A_SECOND: u32 = crate::SPECTRUM_SAMPLES_A_SECOND;
/// How many moments a second.
pub const WAVE_MOMENTS_A_SECOND: u32 = 100;
/// How far under a full scale sound a moment still shows above nothing: a
/// moment written as 0 is this quiet or quieter, one written as 255 is full
/// scale.
const WAVE_RANGE_DB: f32 = 60.0;

const MOMENT: usize = (WAVE_SAMPLES_A_SECOND / WAVE_MOMENTS_A_SECOND) as usize;

/// The wave of a song, worked out as its sound arrives: only the loudest
/// sample of the moment under way is held besides what is written.
#[derive(Debug, Default)]
pub struct WaveReading {
    /// How many samples the moment under way has heard.
    heard: usize,
    loudest: u16,
    levels: Vec<u8>,
}

impl WaveReading {
    /// The next of the song, in however many pieces it comes.
    pub fn hear(&mut self, samples: &[i16]) {
        for sample in samples {
            self.loudest = self.loudest.max(sample.unsigned_abs());
            self.heard += 1;
            if self.heard == MOMENT {
                self.write_down();
            }
        }
    }

    /// One byte a moment, in the order they came; a last part of a moment
    /// counts as one.
    pub fn levels(mut self) -> Vec<u8> {
        if self.heard > 0 {
            self.write_down();
        }
        self.levels
    }

    fn write_down(&mut self) {
        self.levels.push(level_of(self.loudest));
        self.heard = 0;
        self.loudest = 0;
    }
}

/// A sample's size as a byte over the range: nought for silence.
fn level_of(loudest: u16) -> u8 {
    if loudest == 0 {
        return 0;
    }
    let decibels = 20.0 * (f32::from(loudest) / 32_768.0).log10();
    (((decibels + WAVE_RANGE_DB) / WAVE_RANGE_DB).clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(samples: &[i16], pieces: usize) -> Vec<u8> {
        let mut reading = WaveReading::default();
        for piece in samples.chunks(pieces) {
            reading.hear(piece);
        }
        reading.levels()
    }

    #[test]
    fn each_moment_is_its_loudest_sample_and_silence_is_nought() {
        let mut samples = vec![0i16; MOMENT];
        samples.extend(vec![0i16; MOMENT - 1]);
        samples.push(i16::MIN);
        samples.extend(vec![-3_277i16; MOMENT]);
        assert_eq!(read(&samples, 7), vec![0, 255, 170]);
    }

    #[test]
    fn a_sound_under_the_range_lies_flat() {
        assert_eq!(read(&[16; MOMENT], MOMENT), vec![0]);
    }

    #[test]
    fn pieces_of_any_length_read_the_same_and_a_last_part_counts() {
        let samples: Vec<i16> = (0..(MOMENT * 3 + 11))
            .map(|at| i16::try_from(at % 9_000).unwrap_or(0))
            .collect();
        let whole = read(&samples, samples.len());
        assert_eq!(whole.len(), 4);
        assert_eq!(read(&samples, 1), whole);
        assert_eq!(read(&samples, 333), whole);
    }

    #[test]
    fn nothing_heard_is_no_wave() {
        assert!(WaveReading::default().levels().is_empty());
    }
}
