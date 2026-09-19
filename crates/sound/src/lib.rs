//! Writing a stretch of sound down small enough to compare it with another.
//!
//! The opening titles of a series are the one part of an episode that is the
//! same in every episode of its season. Nothing else is: the reminder of last
//! week is new footage every week, and so is the episode itself. So on a file
//! that says nothing about itself, the way to find the opening is to listen to
//! two episodes and ask what they have in common.
//!
//! Comparing the sound itself would not do. The same opening encoded twice is
//! never the same numbers twice, and the same opening played a little louder
//! is not the same numbers at all. What survives both is which notes are
//! playing and how they move: a descriptor rather than the sound. Every
//! thirtieth of a second becomes thirty two bits saying, band by band, whether
//! that part of the sound gained on its neighbour since an eighth of a second
//! before. Written that way, two things fall out for free: how loud it was
//! cancels, because loudness moved every band together, and so does any gentle
//! reshaping, because that too is the same from one instant to the next.
//!
//! This is the published robust hash of Haitsma and Kalker, which is what
//! every tool of this kind is built on. Nothing here reads a file, launches
//! anything or keeps anything: samples in, a descriptor out, and two
//! descriptors in, the longest thing they share out.
//!
//! Measured here on four minutes of made up sound, which is about what one end
//! of an episode is worth reading: writing it down takes 180 ms, and comparing
//! two of them takes 127 ms. An episode is therefore read for well under a
//! second of arithmetic, and a season of twenty two compared against itself for
//! a quarter of a minute. Neither is anything beside getting the sound off the
//! disk in the first place, which is what this was sized to leave room for.
//!
//! Two kinds of frame are refused a comparison, both for the same reason: they
//! would match anything at all. Near silence says nothing, and neither does a
//! sound holding perfectly still, because a descriptor built on what changed
//! is all zeroes when nothing changes. Two files each holding two silent
//! seconds have nothing in common, and a reading that said otherwise would put
//! a skip button over the first scene of every quiet episode.
//!
//! Which is not the same as saying they end a stretch already under way. A
//! title sequence that pauses for breath is one sound with a hole in it: both
//! episodes fall silent at the same moment of the same opening, and reading
//! that hole as a disagreement tore the opening in two. So a stretch carries a
//! run of frames neither side can describe, for a bounded while, and never
//! begins on one.

#![forbid(unsafe_code)]

mod transform;

use melyxar_core::time::Millis;

use crate::transform::{Complex, Transform};

/// How many samples of one second of sound this reads, and therefore what the
/// caller has to hand it.
///
/// Eight thousand a second keeps everything below four thousand cycles, which
/// is where the whole of a melody lives and most of a voice. Keeping more
/// would cost more at every step and describe nothing the comparison uses.
pub const SAMPLES_A_SECOND: u32 = 8_000;

/// How much sound one window covers.
///
/// A quarter of a second holds enough of a note to tell which one it is, and
/// smooths what it describes over a long enough stretch that reading the same
/// sound from a slightly different place still reads much the same.
const WINDOW: usize = 2_048;

/// How far apart two windows stand.
///
/// This one is not about the sound. Two episodes hold the same opening at
/// their own arbitrary distances from their own first sample, so the two grids
/// of windows never line up, and how badly they miss is half a step. Measured
/// here on a made up opening: missing by an eighth of a second changes more
/// than half the bits of a frame, missing by a sixtieth changes about one in
/// twenty. A thirtieth of a second is therefore the step, not because anything
/// needs describing that often, but because it is what keeps two readings of
/// one sound recognisably the same reading.
const STEP: usize = 256;

/// How long one frame of the descriptor stands for.
pub const A_FRAME: Millis = Millis::new(STEP as i64 * 1_000 / SAMPLES_A_SECOND as i64);

/// How far back a frame looks to say what changed.
///
/// Not one step. Neighbouring windows overlap almost entirely, so what
/// separates them is too small to mean anything and its sign would be settled
/// by rounding. An eighth of a second back is a real change, described thirty
/// times a second.
const LOOK_BACK: usize = 4;

/// How many bands the sound is split into, and the range they cover.
///
/// Below three hundred cycles sits the rumble every film shares, and above
/// three thousand eight hundred sits what an encoder throws away first. The
/// bands are spaced the way hearing is, each a fixed multiple of the one
/// below, so a band of the low end is narrow and a band of the high end wide.
const BANDS: usize = 33;
const LOWEST: f32 = 300.0;
const HIGHEST: f32 = 3_800.0;

/// One bit for each pair of neighbouring bands.
const BITS: usize = BANDS - 1;

/// How faint a window has to be before it is held to say nothing.
///
/// Measured as the mean square of the samples, so this is about minus fifty
/// five decibels of full scale: quieter than the room tone under a whispered
/// line, and far quieter than anything anybody put in a soundtrack on purpose.
const TOO_FAINT_TO_DESCRIBE: f32 = 3.2e-6;

/// How many of the thirty two bits have to move before a frame is held to say
/// something.
///
/// A sound that holds perfectly still moves none of them, and would then match
/// every other sound that holds still, silence included.
const ENOUGH_HAPPENING: u32 = 4;

/// How many bits two frames may disagree on and still be the same moment.
///
/// About a third, which is what the tools of this family settle on. It has to
/// cover two readings of one sound that fell on grids a sixtieth of a second
/// apart, which alone costs a bit or two, plus whatever two encoders made of
/// the same music. Two unrelated frames agree this closely about once in
/// twenty, which sounds a lot until one remembers that a stretch worth
/// reporting is hundreds of frames long: a run of chance agreements that long
/// does not happen.
const ALIKE_ENOUGH: u32 = 11;

/// What a frame that disagrees costs the stretch it sits in the middle of.
///
/// A stretch is not ended by the first frame that disagrees, because the two
/// grids of windows never line up and that alone loses a frame here and there.
/// What it must never do is carry on past the end of what is really shared: a
/// button that skips a few seconds into the episode is a button nobody presses
/// twice.
///
/// Costing four keeps a stretch growing while four frames in five agree.
/// Measured, two readings of one sound disagree on well under one frame in ten
/// even at the worst offset the grids can fall on, so four in five leaves room
/// to spare; two unrelated sounds agree on about one frame in twenty, so it is
/// nowhere near enough to grow on. Costing two would let a stretch grow
/// through a third of its frames disagreeing, and a third disagreeing is not
/// the same sound.
const DISAGREEMENT_COSTS: i32 = 4;

/// How long a stretch may carry frames neither side can describe.
///
/// A title sequence that pauses for breath, a second of black screen and
/// silence between two halves of the same music, is one sound with a hole in
/// it and not two sounds. Both episodes fall silent at the same moment of the
/// same opening, which is neither of them agreeing nor either of them
/// disagreeing: it is nothing being said. Counted as a disagreement the hole
/// tears the opening in two and leaves each half too short for a button, which
/// is how a series whose every episode opens the same way came away with none.
///
/// Bounded all the same, and tightly. Silence matches silence everywhere, so a
/// hole long enough to span the quiet between two unrelated moments would let
/// a stretch grow across ground it never really covered. Three seconds is more
/// than any pause anybody writes into a title sequence and far less than the
/// quiet between two scenes.
const A_PAUSE_AT_MOST: usize = 3 * SAMPLES_A_SECOND as usize / STEP;

/// One thirtieth of a second of sound, written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    /// Which bands gained on their neighbour since an eighth of a second ago.
    hash: u32,
    /// Whether this frame says anything a comparison can use.
    worth_comparing: bool,
}

/// A stretch of sound, written down small enough to compare.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listened {
    frames: Vec<Frame>,
}

impl Listened {
    /// Listens to a stretch of sound handed over as single channel samples at
    /// [`SAMPLES_A_SECOND`].
    ///
    /// A stretch too short to hold a window and the look back behind it
    /// describes nothing, and says so rather than describing what little it
    /// has: a handful of frames could not carry an opening anyway.
    pub fn of(samples: &[i16]) -> Self {
        let windows = match samples.len().checked_sub(WINDOW) {
            Some(room) => room / STEP + 1,
            None => return Self::default(),
        };
        let count = match windows.checked_sub(LOOK_BACK) {
            Some(count) if count > 0 => count,
            _ => return Self::default(),
        };

        let transform = Transform::of_size(WINDOW);
        let edges = band_edges();
        let shape = window_shape();

        let heard: Vec<Heard> = (0..windows)
            .map(|window| {
                let at = window * STEP;
                listen_to_one_window(&samples[at..at + WINDOW], &transform, &edges, &shape)
            })
            .collect();

        let frames = (0..count)
            .map(|frame| compare_with_what_came_before(&heard[frame + LOOK_BACK], &heard[frame]))
            .collect();
        Self { frames }
    }

    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Where in a stretch of sound something begins and ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stretch {
    pub start: Millis,
    pub end: Millis,
}

impl Stretch {
    pub fn length(self) -> Millis {
        self.end.saturating_sub(self.start)
    }
}

/// The longest thing two stretches of sound turned out to share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InCommon {
    first_from: usize,
    second_from: usize,
    frames: usize,
}

impl InCommon {
    /// Where it sits in the first stretch.
    pub fn in_the_first(self) -> Stretch {
        self.stretch_from(self.first_from)
    }

    /// Where it sits in the second.
    pub fn in_the_second(self) -> Stretch {
        self.stretch_from(self.second_from)
    }

    /// How long the two of them share.
    pub fn length(self) -> Millis {
        Millis::new(self.frames as i64 * A_FRAME.get())
    }

    /// Both ends are the moment a frame stands for, which leaves the stretch a
    /// touch inside what the two really share, at both ends. That is the right
    /// way round to be wrong: a button that skips a moment less leaves a
    /// moment of music, and one that skips a moment more cuts into the
    /// episode.
    fn stretch_from(self, frame: usize) -> Stretch {
        Stretch {
            start: moment_of(frame),
            end: moment_of(frame + self.frames),
        }
    }
}

/// Where in the stretch of sound a frame stands.
///
/// A frame says what changed between a window and the one an eighth of a
/// second before it, so it stands where the later of the two does.
pub fn moment_of(frame: usize) -> Millis {
    Millis::new((frame + LOOK_BACK) as i64 * A_FRAME.get())
}

/// Finds the longest run of sound two stretches share, wherever it sits in
/// either of them.
///
/// Every alignment of the two is tried rather than the likely ones, because
/// the reminder of last week is a different length every week and is exactly
/// what moves an opening about. There is nothing clever to be had here: the
/// whole search of four minutes against four minutes is a few tens of millions
/// of comparisons of two numbers, which is nothing beside reading the sound
/// off the disk in the first place.
///
/// Nothing at all means the two share not a single usable frame.
pub fn what_they_have_in_common(first: &Listened, second: &Listened) -> Option<InCommon> {
    let (first, second) = (&first.frames, &second.frames);
    if first.is_empty() || second.is_empty() {
        return None;
    }

    let mut best: Option<InCommon> = None;
    let mut best_score = 0i32;
    let first_len = first.len() as isize;
    let second_len = second.len() as isize;

    for shift in (1 - first_len)..second_len {
        let from = (-shift).clamp(0, first_len) as usize;
        let to = (second_len - shift).clamp(0, first_len) as usize;
        if from >= to {
            continue;
        }
        let against = (from as isize + shift) as usize;

        let mut running = 0i32;
        let mut began = from;
        let mut carried = 0usize;
        for (step, (one, other)) in first[from..to]
            .iter()
            .zip(second[against..against + (to - from)].iter())
            .enumerate()
        {
            match how_they_compare(*one, *other) {
                HowTheyCompare::Disagree => {
                    running = (running - DISAGREEMENT_COSTS).max(0);
                    carried = 0;
                    continue;
                }
                // Nothing said on either side never begins a stretch, only
                // carries one already under way: a stretch begun on silence
                // would match the quiet start of every episode there is.
                HowTheyCompare::NeitherSays => {
                    if running > 0 {
                        carried += 1;
                        if carried > A_PAUSE_AT_MOST {
                            running = 0;
                            carried = 0;
                        }
                    }
                    continue;
                }
                HowTheyCompare::TheSameMoment => carried = 0,
            }
            let at = from + step;
            if running <= 0 {
                running = 1;
                began = at;
            } else {
                running += 1;
            }
            if running > best_score {
                best_score = running;
                best = Some(InCommon {
                    first_from: began,
                    second_from: (began as isize + shift) as usize,
                    frames: at + 1 - began,
                });
            }
        }
    }
    best
}

/// What one frame of a stretch has to say about the frame it was put against.
///
/// Three answers rather than two, because a frame neither side can describe is
/// not a frame the two disagree on. One of them silent while the other plays
/// is a real disagreement; both silent at once is nothing being said, and the
/// difference is what tells a pause inside an opening from the end of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HowTheyCompare {
    TheSameMoment,
    NeitherSays,
    Disagree,
}

fn how_they_compare(one: Frame, other: Frame) -> HowTheyCompare {
    if !one.worth_comparing && !other.worth_comparing {
        HowTheyCompare::NeitherSays
    } else if the_same_moment(one, other) {
        HowTheyCompare::TheSameMoment
    } else {
        HowTheyCompare::Disagree
    }
}

/// Whether two frames describe the same moment of sound.
fn the_same_moment(one: Frame, other: Frame) -> bool {
    one.worth_comparing
        && other.worth_comparing
        && (one.hash ^ other.hash).count_ones() <= ALIKE_ENOUGH
}

/// What one window of sound turned out to hold.
#[derive(Debug, Clone, Copy)]
struct Heard {
    /// How much sound sits in each band.
    bands: [f32; BANDS],
    /// How loud the window was, as a mean square of its samples.
    power: f32,
}

/// Reads one window: how loud it was, and how its sound is spread across the
/// bands.
fn listen_to_one_window(
    samples: &[i16],
    transform: &Transform,
    edges: &[usize; BANDS + 1],
    shape: &[f32; WINDOW],
) -> Heard {
    let mut power = 0.0f32;
    let mut window = Vec::with_capacity(WINDOW);
    for (at, sample) in samples.iter().enumerate() {
        let value = f32::from(*sample) / f32::from(i16::MAX);
        power += value * value;
        window.push(Complex::of(value * shape[at]));
    }
    transform.run(&mut window);

    let mut bands = [0.0f32; BANDS];
    for (band, strength) in bands.iter_mut().enumerate() {
        *strength = window[edges[band]..edges[band + 1]]
            .iter()
            .map(|pitch| pitch.strength_squared())
            .sum();
    }

    Heard {
        bands,
        power: power / WINDOW as f32,
    }
}

/// Turns a window and the one an eighth of a second before it into a frame.
fn compare_with_what_came_before(now: &Heard, before: &Heard) -> Frame {
    let mut hash = 0u32;
    for bit in 0..BITS {
        let gained_now = now.bands[bit] - now.bands[bit + 1];
        let gained_before = before.bands[bit] - before.bands[bit + 1];
        if gained_now > gained_before {
            hash |= 1 << bit;
        }
    }

    let loud_enough = now.power > TOO_FAINT_TO_DESCRIBE && before.power > TOO_FAINT_TO_DESCRIBE;
    Frame {
        hash,
        worth_comparing: loud_enough && hash.count_ones() >= ENOUGH_HAPPENING,
    }
}

/// Which pitches fall in which band.
///
/// Each band is the same multiple of the one below it, so the edges climb the
/// way hearing does rather than in equal steps of cycles.
fn band_edges() -> [usize; BANDS + 1] {
    let spread = HIGHEST / LOWEST;
    let mut edges = [0usize; BANDS + 1];
    for (band, edge) in edges.iter_mut().enumerate() {
        let frequency = LOWEST * spread.powf(band as f32 / BANDS as f32);
        *edge = (frequency * WINDOW as f32 / SAMPLES_A_SECOND as f32).round() as usize;
    }
    edges
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

    /// A run of made up sound, built note by note so that it moves the way
    /// music moves. A sound that held still would describe nothing, which is
    /// the whole point of the descriptor and is tested for on its own below.
    struct MadeUpSound {
        samples: Vec<i16>,
        next: u32,
    }

    impl MadeUpSound {
        fn new(seed: u32) -> Self {
            Self {
                samples: Vec::new(),
                next: seed | 1,
            }
        }

        fn roll(&mut self) -> f32 {
            self.next = self
                .next
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            self.next as f32 / u32::MAX as f32
        }

        /// Adds a run of notes, each a fifth of a second.
        fn tune(mut self, seconds: f32, loudness: f32) -> Self {
            let notes = (seconds * 5.0).round() as usize;
            for _ in 0..notes {
                let pitch = 400.0 + self.roll() * 2_000.0;
                self = self.note(0.2, pitch, loudness);
            }
            self
        }

        fn note(mut self, seconds: f32, pitch: f32, loudness: f32) -> Self {
            let count = (seconds * SAMPLES_A_SECOND as f32).round() as usize;
            let already = self.samples.len() as f32;
            for at in 0..count {
                let moment = (already + at as f32) / SAMPLES_A_SECOND as f32;
                let turns = 2.0 * std::f32::consts::PI * pitch * moment;
                // Two pitches at once, so that a band gains on its neighbour
                // rather than the whole of the sound moving together.
                let value = 0.7 * turns.sin() + 0.3 * (2.5 * turns).sin();
                self.samples
                    .push((value * loudness * f32::from(i16::MAX) * 0.5) as i16);
            }
            self
        }

        fn silence(mut self, seconds: f32) -> Self {
            let count = (seconds * SAMPLES_A_SECOND as f32).round() as usize;
            self.samples.extend(std::iter::repeat_n(0i16, count));
            self
        }

        fn done(self) -> Vec<i16> {
            self.samples
        }
    }

    /// The opening two made up episodes share, six seconds of it.
    fn an_opening(loudness: f32) -> Vec<i16> {
        MadeUpSound::new(7).tune(6.0, loudness).done()
    }

    /// An episode: something of its own, then the opening, then the episode.
    ///
    /// The lengths are given in samples and land nowhere near a window
    /// boundary, on purpose, because that is what a real file does: the
    /// opening begins where it begins, and two grids of windows never line up.
    fn episode(before: usize, opening: &[i16], after: usize, seed: u32) -> Vec<i16> {
        let seconds = |count: usize| count as f32 / SAMPLES_A_SECOND as f32;
        let mut samples = MadeUpSound::new(seed)
            .tune(seconds(before) + 0.5, 0.8)
            .done();
        samples.truncate(before);
        samples.extend_from_slice(opening);
        samples.extend(
            MadeUpSound::new(seed.wrapping_add(1_000))
                .tune(seconds(after), 0.8)
                .done(),
        );
        samples
    }

    fn about(found: Millis, expected: f64) -> bool {
        (found.as_seconds_f64() - expected).abs() <= 0.6
    }

    #[test]
    fn the_opening_two_episodes_share_is_found_wherever_it_sits() {
        // One episode opens almost straight away, the other after a reminder
        // of last week. Neither is a whole number of windows in, which is the
        // hard part and the reason the step is as short as it is.
        let opening = an_opening(1.0);
        let first = episode(4_137, &opening, 40_000, 11);
        let second = episode(32_909, &opening, 40_000, 29);

        let found = what_they_have_in_common(&Listened::of(&first), &Listened::of(&second))
            .expect("two episodes sharing six seconds of opening share something");

        assert!(
            about(found.length(), 6.0),
            "the whole of the opening and little else: {:?}",
            found.length()
        );
        assert!(
            about(found.in_the_first().start, 4_137.0 / 8_000.0),
            "found where it really sits in the first: {:?}",
            found.in_the_first()
        );
        assert!(
            about(found.in_the_second().start, 32_909.0 / 8_000.0),
            "and where it really sits in the second: {:?}",
            found.in_the_second()
        );
    }

    #[test]
    fn an_opening_that_pauses_for_breath_is_found_whole_rather_than_in_halves() {
        // The defect this exists for: a real series opens on five seconds of
        // music, a second of black screen and silence, and five seconds more.
        // Both episodes are silent at the same moment of the same opening, so
        // the pause is not two episodes disagreeing; it is neither of them
        // saying anything. Read as a disagreement it tore the opening in two
        // and left a stretch too short for a button, which is how a series
        // with an opening on every episode came away with none.
        let opening = {
            let mut sound = MadeUpSound::new(7).tune(5.0, 1.0).silence(1.0).done();
            sound.extend(MadeUpSound::new(13).tune(5.0, 1.0).done());
            sound
        };
        let first = episode(4_137, &opening, 40_000, 11);
        let second = episode(32_909, &opening, 40_000, 29);

        let found = what_they_have_in_common(&Listened::of(&first), &Listened::of(&second))
            .expect("two episodes sharing an opening share something");
        assert!(
            about(found.length(), 11.0),
            "the whole opening, pause included, rather than one half of it: {:?}",
            found.length()
        );
        assert!(
            about(found.in_the_first().start, 4_137.0 / 8_000.0),
            "starting where the opening really starts: {:?}",
            found.in_the_first()
        );
    }

    #[test]
    fn an_opening_holding_the_very_first_sample_is_found_from_its_first_moment() {
        // Where a real series puts its titles more often than anywhere else:
        // straight at the start of the file, nothing before them. Nothing here
        // can look behind the first sample, so the answer begins one look back
        // in, and that is the whole of what is lost.
        let opening = {
            let mut sound = MadeUpSound::new(7).tune(5.0, 1.0).silence(1.0).done();
            sound.extend(MadeUpSound::new(13).tune(5.0, 1.0).done());
            sound
        };
        let first = episode(0, &opening, 40_000, 11);
        let second = episode(0, &opening, 40_000, 29);

        let found = what_they_have_in_common(&Listened::of(&first), &Listened::of(&second))
            .expect("two episodes opening on the same titles share them");
        assert!(
            about(found.length(), 11.0),
            "the whole of it: {:?}",
            found.length()
        );
        assert!(
            found.in_the_first().start.get() <= A_FRAME.get() * (LOOK_BACK as i64 + 1),
            "from the first moment there is one: {:?}",
            found.in_the_first()
        );
    }

    #[test]
    fn a_long_silence_never_welds_two_shared_moments_into_one() {
        // The other half of carrying a pause. Silence matches silence
        // everywhere, so a hole allowed to grow without bound would let a
        // stretch run from the opening titles across a quiet minute and into
        // whatever else the two episodes happen to share, and a button
        // offering to skip all of it would cut into the episode.
        let both = {
            let mut sound = MadeUpSound::new(7).tune(3.0, 1.0).silence(8.0).done();
            sound.extend(MadeUpSound::new(13).tune(3.0, 1.0).done());
            sound
        };
        let first = episode(4_137, &both, 40_000, 11);
        let second = episode(32_909, &both, 40_000, 29);

        let found = what_they_have_in_common(&Listened::of(&first), &Listened::of(&second))
            .expect("the two do share something");
        assert!(
            found.length().as_seconds_f64() < 5.0,
            "one of the two shared moments, not both and the quiet between \
             them: {:?}",
            found.length()
        );
    }

    #[test]
    fn an_opening_played_louder_is_still_the_same_opening() {
        // What is written down is which bands gained on their neighbours, and
        // turning the sound up moves every band together, so it cancels.
        let first = episode(8_123, &an_opening(1.0), 32_000, 3);
        let second = episode(24_599, &an_opening(0.25), 32_000, 5);

        let found = what_they_have_in_common(&Listened::of(&first), &Listened::of(&second))
            .expect("the same opening at a quarter of the volume is the same opening");
        assert!(about(found.length(), 6.0), "{:?}", found.length());
    }

    #[test]
    fn two_episodes_sharing_nothing_share_nothing_worth_reporting() {
        let first = MadeUpSound::new(101).tune(12.0, 0.8).done();
        let second = MadeUpSound::new(202).tune(12.0, 0.8).done();

        let shared = what_they_have_in_common(&Listened::of(&first), &Listened::of(&second))
            .map_or(Millis::ZERO, |found| found.length());
        assert!(
            shared.as_seconds_f64() < 1.5,
            "two unrelated stretches agree here and there by chance, never for \
             long: {shared:?}"
        );
    }

    #[test]
    fn two_stretches_of_silence_have_nothing_in_common() {
        // The one answer that would be catastrophic: a skip button over the
        // first scene of every episode that opens quietly.
        let first = MadeUpSound::new(1).silence(10.0).done();
        let second = MadeUpSound::new(2).silence(10.0).done();

        assert_eq!(
            what_they_have_in_common(&Listened::of(&first), &Listened::of(&second)),
            None
        );
    }

    #[test]
    fn a_sound_that_holds_perfectly_still_matches_nothing_either() {
        // A held note describes nothing, because nothing changed. Two
        // different held notes would otherwise be found to be the same sound.
        let first = MadeUpSound::new(1).note(10.0, 440.0, 0.8).done();
        let second = MadeUpSound::new(2).note(10.0, 990.0, 0.8).done();

        let shared = what_they_have_in_common(&Listened::of(&first), &Listened::of(&second))
            .map_or(Millis::ZERO, |found| found.length());
        assert!(shared.as_seconds_f64() < 1.5, "{shared:?}");
    }

    #[test]
    fn silence_on_one_side_is_never_the_same_as_music_on_the_other() {
        let music = MadeUpSound::new(5).tune(10.0, 0.8).done();
        let quiet = MadeUpSound::new(6).silence(10.0).done();

        let shared = what_they_have_in_common(&Listened::of(&music), &Listened::of(&quiet))
            .map_or(Millis::ZERO, |found| found.length());
        assert!(shared.as_seconds_f64() < 1.5, "{shared:?}");
    }

    #[test]
    fn a_stretch_compared_with_itself_is_itself_from_end_to_end() {
        let sound = Listened::of(&MadeUpSound::new(17).tune(8.0, 0.8).done());
        let found = what_they_have_in_common(&sound, &sound).expect("a sound is like itself");

        assert_eq!(found.frames, sound.frames());
        assert_eq!(found.in_the_first(), found.in_the_second());
    }

    #[test]
    fn a_stretch_too_short_to_describe_describes_nothing() {
        let barely = MadeUpSound::new(1).tune(0.2, 0.8).done();
        assert!(Listened::of(&barely).is_empty());
        assert!(Listened::of(&[]).is_empty());
        assert_eq!(
            what_they_have_in_common(&Listened::default(), &Listened::default()),
            None
        );
    }

    #[test]
    fn a_frame_stands_for_a_thirtieth_of_a_second() {
        assert_eq!(A_FRAME, Millis::new(32));
        assert_eq!(moment_of(0), Millis::new(128));
        assert_eq!(moment_of(10), Millis::new(448));
    }

    #[test]
    fn the_bands_climb_and_never_run_past_what_was_heard() {
        let edges = band_edges();
        for pair in edges.windows(2) {
            assert!(
                pair[1] > pair[0],
                "every band holds at least one pitch: {edges:?}"
            );
        }
        assert!(
            *edges.last().expect("edges") < WINDOW / 2,
            "only the first half of a transform carries anything"
        );
        assert_eq!(BITS, 32, "one frame is one number of thirty two bits");
    }
}
