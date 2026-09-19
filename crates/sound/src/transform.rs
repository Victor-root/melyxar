//! Turning a window of sound into the strength of every pitch it holds.
//!
//! Sound arrives as a line of numbers saying how far the air was pushed at
//! each instant. Nothing about that line says which notes are playing, and
//! which notes are playing is the only thing that survives a file being
//! encoded again, played louder, or mixed under a different voice. The
//! transform is what turns one into the other.
//!
//! Written here rather than taken from elsewhere, for the same reason the
//! reading of a container's index is: it is a fixed piece of arithmetic, it
//! has one right answer, and a test can show it gives that answer. The naive
//! way of computing it is four lines long and unbearably slow; the fast way is
//! thirty lines and exact to the same answer. The test below runs both and
//! compares them, which is a proof rather than a hope.

use std::f32::consts::PI;

/// A number with two parts, which is what a pitch needs: how strong it is and
/// where in its cycle it stands.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Complex {
    pub real: f32,
    pub imaginary: f32,
}

impl Complex {
    pub const fn of(real: f32) -> Self {
        Self {
            real,
            imaginary: 0.0,
        }
    }

    /// How strong this pitch is, squared.
    ///
    /// Squared because taking the root costs more than everything else here
    /// put together and nothing below needs it: what is compared is one
    /// strength against another, and squaring keeps that order intact.
    pub fn strength_squared(self) -> f32 {
        self.real * self.real + self.imaginary * self.imaginary
    }

    fn plus(self, other: Self) -> Self {
        Self {
            real: self.real + other.real,
            imaginary: self.imaginary + other.imaginary,
        }
    }

    fn minus(self, other: Self) -> Self {
        Self {
            real: self.real - other.real,
            imaginary: self.imaginary - other.imaginary,
        }
    }

    fn times(self, other: Self) -> Self {
        Self {
            real: self.real * other.real - self.imaginary * other.imaginary,
            imaginary: self.real * other.imaginary + self.imaginary * other.real,
        }
    }
}

/// The transform, set up once for one window size.
///
/// The turns it needs are the same for every window, so they are worked out
/// once here rather than once per window: a ten minute stretch of sound is
/// nine thousand windows, and computing the same few hundred cosines nine
/// thousand times over is most of the work for none of the answer.
#[derive(Debug, Clone)]
pub struct Transform {
    turns: Vec<Complex>,
}

impl Transform {
    /// Prepares the transform for windows of this many samples.
    ///
    /// The size must be a power of two, which is what lets the work be halved
    /// and halved again. Every caller here asks for a constant, so this is a
    /// statement about the code rather than about anything a file could say.
    pub fn of_size(size: usize) -> Self {
        assert!(
            size.is_power_of_two() && size >= 2,
            "the transform halves its work, so its window is a power of two"
        );
        let turns = (0..size / 2)
            .map(|step| {
                let angle = -2.0 * PI * step as f32 / size as f32;
                Complex {
                    real: angle.cos(),
                    imaginary: angle.sin(),
                }
            })
            .collect();
        Self { turns }
    }

    pub fn size(&self) -> usize {
        self.turns.len() * 2
    }

    /// Replaces a window of sound with the strength of each of its pitches.
    ///
    /// The answer is symmetric: the second half mirrors the first, so only the
    /// first half carries anything, and only that half is ever read.
    pub fn run(&self, window: &mut [Complex]) {
        let size = self.size();
        assert_eq!(
            window.len(),
            size,
            "a window of the size this was set up for"
        );

        // The halving leaves the answers in an order that is each position's
        // own number written backwards. Putting them back in order first is
        // what lets every step afterwards work on neighbours.
        let mut mirrored = 0;
        for position in 1..size {
            let mut bit = size >> 1;
            while mirrored & bit != 0 {
                mirrored ^= bit;
                bit >>= 1;
            }
            mirrored |= bit;
            if position < mirrored {
                window.swap(position, mirrored);
            }
        }

        let mut span = 2;
        while span <= size {
            let half = span / 2;
            let stride = size / span;
            let mut start = 0;
            while start < size {
                for step in 0..half {
                    let turn = self.turns[step * stride];
                    let held = window[start + step];
                    let turned = window[start + step + half].times(turn);
                    window[start + step] = held.plus(turned);
                    window[start + step + half] = held.minus(turned);
                }
                start += span;
            }
            span <<= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The transform written the way its definition reads, which is far too
    /// slow to use and therefore exactly what the fast one has to agree with.
    fn the_long_way(values: &[Complex]) -> Vec<Complex> {
        let size = values.len();
        (0..size)
            .map(|pitch| {
                let mut total = Complex::default();
                for (position, value) in values.iter().enumerate() {
                    let angle = -2.0 * PI * pitch as f32 * position as f32 / size as f32;
                    total = total.plus(value.times(Complex {
                        real: angle.cos(),
                        imaginary: angle.sin(),
                    }));
                }
                total
            })
            .collect()
    }

    /// Two notes and a little hiss, so that every pitch carries something and
    /// a transform that dropped one would be caught.
    fn a_bit_of_sound(size: usize) -> Vec<Complex> {
        let mut hiss = 1u32;
        (0..size)
            .map(|position| {
                hiss = hiss.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let at = position as f32;
                let turns = 2.0 * PI * at / size as f32;
                Complex::of(
                    (0.7 * (3.0 * turns).sin())
                        + (0.3 * (11.0 * turns).cos())
                        + (0.1 * (hiss as f32 / u32::MAX as f32 - 0.5)),
                )
            })
            .collect()
    }

    #[test]
    fn the_fast_transform_answers_what_the_definition_answers() {
        for size in [2, 8, 64, 1024] {
            let sound = a_bit_of_sound(size);
            let slow = the_long_way(&sound);

            let mut fast = sound.clone();
            Transform::of_size(size).run(&mut fast);

            for (pitch, (fast, slow)) in fast.iter().zip(slow.iter()).enumerate() {
                let apart = (fast.real - slow.real).abs() + (fast.imaginary - slow.imaginary).abs();
                assert!(
                    apart < 1e-2 * (1.0 + slow.strength_squared().sqrt()),
                    "size {size}, pitch {pitch}: {fast:?} against {slow:?}"
                );
            }
        }
    }

    #[test]
    fn a_single_note_shows_up_at_its_own_pitch_and_nowhere_else() {
        let size = 64;
        let note = 7;
        let mut window: Vec<Complex> = (0..size)
            .map(|position| {
                Complex::of((2.0 * PI * note as f32 * position as f32 / size as f32).cos())
            })
            .collect();
        Transform::of_size(size).run(&mut window);

        let strongest = (0..size / 2)
            .max_by(|one, other| {
                window[*one]
                    .strength_squared()
                    .total_cmp(&window[*other].strength_squared())
            })
            .expect("a window holds something");
        assert_eq!(strongest, note);
        assert!(
            window[note].strength_squared() > 100.0 * window[note + 1].strength_squared(),
            "a note that is exactly one of the pitches lands on that pitch alone"
        );
    }

    #[test]
    fn silence_holds_no_pitch_at_all() {
        let mut window = vec![Complex::default(); 32];
        Transform::of_size(32).run(&mut window);
        assert!(window.iter().all(|pitch| pitch.strength_squared() == 0.0));
    }
}
