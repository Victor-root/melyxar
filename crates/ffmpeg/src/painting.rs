//! Laying a subtitle made of pictures onto the picture, and saying how it went.
//!
//! A browser cannot be handed a subtitle made of pictures, so the only way to
//! show one is to paint it onto every frame. Where that happens decides what a
//! film costs. On the processor the picture is read, painted on and written
//! again, which a film in four thousand pixels does at a fraction of real time.
//! On a card the picture never leaves it, and the painting is one more pass
//! beside the ones that make it smaller.
//!
//! The subtitle is a picture the size of the frame the film was authored for,
//! which is not the size of the picture being written: a film can be made
//! smaller, and one whose black bars were cut off is shorter than the frame the
//! subtitle was drawn for. Stretching it to both sizes would squash the words,
//! and leaving it at its own size would put them anywhere. It is fitted to the
//! picture without changing its shape, never larger than the picture, and set
//! at the foot of it, which is where subtitles are.
//!
//! Cards do not lay one the same way. Intel's and AMD's filter fits what it
//! lays to a size it is given as a formula, so the picture is never measured
//! here. Nvidia's filter only places: what it lays has to arrive at its final
//! size, in the one layout of its that carries transparency, onto a picture in
//! the one layout that accepts it. The size is then worked out here from the
//! size of the picture, and the processor brings the subtitle to it on its way
//! up, which costs next to nothing since a subtitle is small and rarely
//! changes.
//!
//! Everything said here is said under one tag of the journal, `painting`,
//! because nothing proves this on the maintainer's machine but the maintainer's
//! machine: what was chosen and why, what the tool itself complained about, and
//! how much memory and time it takes while it works.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use melyxar_core::time::Millis;

use crate::command::{Command, VideoEncode};
use crate::hardware::{Card, CardPath, A_GENERATED_PICTURE_SIZE, TRIAL_HEIGHT};

/// The canvas a subtitle made of pictures is drawn on: the one every Blu-ray
/// authors them for.
pub(crate) const CANVAS: (i32, i32) = (1920, 1080);

/// What the painted picture is called inside the filter graph.
pub(crate) const PAINTED: &str = "[painted]";

/// Where the picture is when the subtitle is laid on it.
pub(crate) enum Place<'a> {
    /// On the processor, where the subtitle is brought to its size there and
    /// laid at the foot, when the size of the picture is known.
    Processor {
        sized: Option<(i32, i32)>,
    },
    Card {
        way: CardPath,
        layout: &'a str,
        /// The size the subtitle is brought to before it is handed up, for a
        /// card whose filter cannot size what it lays. Nothing for one that
        /// fits it itself, or when the size of the picture is not known.
        sized: Option<(i32, i32)>,
    },
}

impl<'a> Place<'a> {
    /// Where a rebuild lays a subtitle: on the card when it was proved to and
    /// the picture stays up there, otherwise on the processor.
    pub(crate) fn of(encode: &'a VideoEncode) -> Self {
        let proved = encode
            .card()
            .filter(|(card, _)| !comes_down(card, encode.tone_map))
            .and_then(|(card, _)| card.picture_subtitle_layout().map(|layout| (card, layout)));
        match proved {
            Some((card, layout)) => Self::Card {
                way: card.way,
                layout,
                sized: encode.picture_size.and_then(|picture| {
                    sized_for(card, picture, encode.scale_to_height, CANVAS)
                }),
            },
            // Every chain that ends on the processor makes the picture the
            // height asked for, whatever the card could do on its own.
            None => Self::Processor {
                sized: encode.picture_size.map(|picture| {
                    fitted(CANVAS, rebuilt_size(picture, encode.scale_to_height))
                }),
            },
        }
    }
}

/// Whether the picture leaves the card before it is encoded, which is where
/// a subtitle is then laid on it.
fn comes_down(card: &Card, tone_map: bool) -> bool {
    tone_map
        && card
            .tone_mapping
            .is_some_and(crate::hardware::ToneMapping::brings_the_picture_down)
}

/// The size a subtitle drawn on `canvas` is brought to before a card lays it
/// on a picture read at `picture`, for a card whose filter cannot size it.
fn sized_for(
    card: &Card,
    picture: (i32, i32),
    scale_to_height: Option<i32>,
    canvas: (i32, i32),
) -> Option<(i32, i32)> {
    (!card.way.driver().sizes_what_it_lays()).then(|| {
        fitted(
            canvas,
            rebuilt_size(picture, scale_to_height.filter(|_| card.can_scale)),
        )
    })
}

/// The size of a picture once it is made `height` tall, keeping its shape, as
/// the card's resize makes it: as wide as that shape gives, to an even number.
fn rebuilt_size(picture: (i32, i32), height: Option<i32>) -> (i32, i32) {
    let (width, tall) = picture;
    match height {
        Some(height) if tall > 0 => (
            even(i64::from(width) * i64::from(height) / i64::from(tall)),
            height,
        ),
        _ => picture,
    }
}

/// How large a subtitle drawn on `canvas` is laid on `picture`: as wide as the
/// picture lets it be without changing shape, and never taller than it either.
/// The same rule the formula below gives the cards that size it themselves.
fn fitted(canvas: (i32, i32), picture: (i32, i32)) -> (i32, i32) {
    let (canvas_width, canvas_height) = (i64::from(canvas.0), i64::from(canvas.1));
    let (width, height) = (i64::from(picture.0), i64::from(picture.1));
    if canvas_width <= 0 || canvas_height <= 0 {
        return picture;
    }
    (
        even(width.min(height * canvas_width / canvas_height)),
        even(height.min(width * canvas_height / canvas_width)),
    )
}

/// Down to an even number, never below two: the layouts a subtitle is handed
/// up in halve the colour in both directions.
fn even(value: i64) -> i32 {
    i32::try_from((value / 2 * 2).max(2)).unwrap_or(i32::MAX - 1)
}

/// Where a subtitle that arrives at its size is put: centred across, at the
/// foot.
pub(crate) const AT_THE_FOOT: &str = "x='(main_w-overlay_w)/2':y='main_h-overlay_h'";

/// The filter graph that lays a subtitle onto the picture.
///
/// `picture` names the stream the picture comes from and `before` is
/// everything done to it first: painting the words on and then shrinking would
/// shrink the words with it, which is how subtitles end up unreadable on a
/// small screen.
pub(crate) fn graph(picture: &str, before: Option<&str>, subtitle: i32, place: &Place) -> String {
    match place {
        Place::Processor { sized } => {
            let picture = match before {
                Some(filters) => format!("{picture}{filters}[picture];[picture]"),
                None => picture.to_string(),
            };
            match sized {
                Some((width, height)) => format!(
                    "[0:{subtitle}]scale={width}:{height}[words];\
                     {picture}[words]overlay=shortest=0:{AT_THE_FOOT}{PAINTED}"
                ),
                None => format!("{picture}[0:{subtitle}]overlay=shortest=0{PAINTED}"),
            }
        }
        // How a card lays it is its path's business.
        Place::Card { way, layout, sized } => {
            way.driver()
                .paint(picture, before, subtitle, layout, *sized)
        }
    }
}

/// What the journal is told when a tool that paints a subtitle is set going.
pub fn announce(session: &str, index: u32, command: &Command) {
    let Some((encode, subtitle)) = command.painted_subtitle() else {
        return;
    };
    let place = Place::of(encode);
    let painted_by = match (&place, encode.card()) {
        (Place::Card { .. }, _) => "card",
        (Place::Processor { .. }, Some((card, _))) if comes_down(card, encode.tone_map) => {
            "processor, the picture comes down from the card once its colours are converted"
        }
        (Place::Processor { .. }, Some(_)) => "processor, this card was never proved to paint",
        (Place::Processor { .. }, None) => "processor",
    };
    let (layout, subtitle_sized_to) = match place {
        Place::Card { layout, sized, .. } => (Some(layout), sized),
        Place::Processor { sized } => (None, sized),
    };
    tracing::debug!(
        session,
        index,
        subtitle_stream = subtitle,
        painted_by,
        film_read_by = encode
            .card()
            .map(|(_, reads)| if reads { "card" } else { "processor" }),
        card = encode.card().map(|(card, _)| card.name.as_str()),
        layout,
        canvas = ?CANVAS,
        picture_read_at = ?encode.picture_size,
        subtitle_sized_to = ?subtitle_sized_to,
        scale_to_height = encode.scale_to_height,
        tone_map = encode.tone_map,
        sound_read_apart = command.reads_the_sound_apart(),
        graph = command.picture_painted_with_subtitles(),
        "a subtitle made of pictures is painted onto the picture"
    );
}

/// How the tool is getting on, as the session last heard it.
#[derive(Debug, Clone, Copy)]
pub struct Pace {
    /// Speed relative to real time.
    pub speed: f64,
    pub pictures_a_second: f64,
    /// How far into the film the tool has written.
    pub reached: Millis,
}

/// How often the journal is told how a painting tool is getting on.
const HOW_OFTEN_IT_IS_SAID: Duration = Duration::from_secs(5);

/// Says every few seconds how much memory the tool holds and how fast it goes,
/// until it is gone.
///
/// The memory is the number that was missing: a tool that holds several
/// gigabytes was seen once, and nothing said so until the whole machine was
/// swapping. Read from the system rather than guessed, so on a system that
/// does not offer it nothing is said, which is no worse than before.
pub fn watch(session: String, index: u32, pid: u32, pace: impl Fn() -> Pace + Send + 'static) {
    tokio::spawn(async move {
        let began = Instant::now();
        let mut ticks = tokio::time::interval(HOW_OFTEN_IT_IS_SAID);
        // The first tick is immediate, and there is nothing to say yet.
        ticks.tick().await;
        loop {
            ticks.tick().await;
            let Some(memory_mb) = resident_megabytes(pid) else {
                return;
            };
            let pace = pace();
            tracing::debug!(
                session,
                index,
                tool_running_for_s = began.elapsed().as_secs(),
                tool_memory_mb = memory_mb,
                speed = pace.speed,
                pictures_a_second = pace.pictures_a_second,
                written_up_to_s = pace.reached.as_seconds_f64(),
                "the tool is painting a subtitle made of pictures"
            );
        }
    });
}

/// How much of the machine's memory a process holds, in megabytes.
fn resident_megabytes(pid: u32) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    resident_kilobytes_in(&status).map(|kilobytes| kilobytes / 1024)
}

fn resident_kilobytes_in(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// How many lines the tool says are kept in the journal for one run.
///
/// The first ones are the ones that explain a run, and a tool that complains
/// about every frame would otherwise push everything else out of a journal
/// that keeps a few thousand lines.
const LINES_KEPT_PER_RUN: usize = 40;

/// What the tool said while it ran, kept for the journal.
///
/// A tool painting a subtitle is asked to say warnings as well as errors, and
/// those are where a subtitle that is the wrong size, or a frame dropped, shows
/// up. Said under the same tag as the rest so they are read in order with it.
pub(crate) struct ToolSaid {
    painting: bool,
    said: usize,
    tail: VecDeque<String>,
}

/// How many of the last lines a failure carries with it.
const LINES_KEPT_FOR_A_FAILURE: usize = 20;

impl ToolSaid {
    pub(crate) fn new(command: &Command) -> Self {
        Self {
            painting: command.painted_subtitle().is_some(),
            said: 0,
            tail: VecDeque::new(),
        }
    }

    pub(crate) fn hear(&mut self, line: &str) {
        if self.painting && self.said < LINES_KEPT_PER_RUN {
            self.said += 1;
            tracing::debug!(said = line, "the tool, painting a subtitle, said");
        }
        if self.tail.len() == LINES_KEPT_FOR_A_FAILURE {
            self.tail.pop_front();
        }
        self.tail.push_back(line.to_string());
    }

    /// The last things it said, as one line.
    pub(crate) fn tail(&self) -> String {
        self.tail
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

/// Lays a subtitle on a picture on this card, as a film will, to prove it can.
pub(crate) fn trial_arguments(card: &Card, encoder: &str, layout: &str) -> Vec<String> {
    let place = Place::Card {
        way: card.way,
        layout,
        sized: sized_for(
            card,
            A_GENERATED_PICTURE_SIZE,
            Some(TRIAL_HEIGHT),
            A_SUBTITLE_SIZE,
        ),
    };
    let before = card.filters_for(Some(TRIAL_HEIGHT), false, false).join(",");
    let graph = graph("[0:0]", Some(&before), 1, &place);

    ["-hide_banner", "-nostdin", "-loglevel", "error"]
        .map(str::to_string)
        .into_iter()
        .chain(card.opening_arguments(false, false))
        .chain(
            [
                "-f",
                "lavfi",
                "-i",
                A_PICTURE_AND_A_SUBTITLE,
                "-filter_complex",
                &graph,
                "-map",
                PAINTED,
                "-c:v",
                encoder,
                "-f",
                "null",
                "-",
            ]
            .map(str::to_string),
        )
        .collect()
}

/// The size of the subtitle the trial lays.
const A_SUBTITLE_SIZE: (i32, i32) = (480, 360);

/// What the trial lays together: a picture and, beside it, a translucent
/// subtitle narrower than it, so that the placement is the one worth proving.
///
/// The subtitle comes out in the layout the tool draws a real one in, and the
/// graph converts it from there, as it will for a film.
const A_PICTURE_AND_A_SUBTITLE: &str = "testsrc2=size=640x360:rate=25:duration=0.4[out0];\
     color=c=white@0.5:size=480x360:rate=25:duration=0.4,format=bgra[out1]";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_the_processor_the_subtitle_is_laid_on_the_picture_where_it_is() {
        let graph = graph("[0:0]", Some("scale=-2:1080"), 3, &Place::Processor { sized: None });
        assert_eq!(
            graph,
            "[0:0]scale=-2:1080[picture];[picture][0:3]overlay=shortest=0[painted]"
        );
    }

    #[test]
    fn on_a_card_the_picture_stays_there_and_only_the_subtitle_is_handed_up() {
        let place = Place::Card {
            way: CardPath::Vaapi,
            layout: "bgra",
            sized: None,
        };
        let graph = graph("[0:0]", Some("scale_vaapi=format=nv12"), 3, &place);

        assert!(
            graph.starts_with(
                "[0:0]scale_vaapi=format=nv12[picture];[0:3]format=bgra,hwupload[words];"
            ),
            "{graph}"
        );
        assert!(
            !graph.contains("hwdownload"),
            "nothing comes back down to the processor: {graph}"
        );
        assert!(graph.contains("[picture][words]overlay_vaapi="), "{graph}");
        assert!(graph.ends_with("[painted]"), "{graph}");
    }

    #[test]
    fn the_subtitle_is_fitted_to_the_picture_and_set_at_its_foot() {
        let place = Place::Card {
            way: CardPath::Vaapi,
            layout: "bgra",
            sized: None,
        };
        let graph = graph("[0:0]", Some("scale_vaapi=format=nv12"), 3, &place);

        // Both bounds, so a subtitle drawn for a taller frame than the
        // picture is never taller than it.
        assert!(graph.contains("w='min(main_w,main_h*overlay_iw/overlay_ih)'"));
        assert!(graph.contains("h='min(main_h,main_w*overlay_ih/overlay_iw)'"));
        assert!(graph.contains("x='(main_w-w)/2':y='main_h-h'"));
    }

    #[test]
    fn a_card_with_nothing_to_do_to_the_picture_still_names_it() {
        let place = Place::Card {
            way: CardPath::Vaapi,
            layout: "bgra",
            sized: None,
        };
        let graph = graph("[0:0]", None, 3, &place);
        assert!(graph.starts_with("[0:0]null[picture];"), "{graph}");
    }

    #[test]
    fn a_picture_made_smaller_keeps_its_shape_to_an_even_width() {
        assert_eq!(rebuilt_size((3840, 1600), Some(1080)), (2592, 1080));
        assert_eq!(rebuilt_size((1920, 1080), Some(720)), (1280, 720));
        // An odd width comes down to the even number below it.
        assert_eq!(rebuilt_size((1998, 1080), Some(721)), (1332, 721));
        assert_eq!(rebuilt_size((1920, 1080), None), (1920, 1080));
    }

    #[test]
    fn a_subtitle_is_fitted_the_way_the_formula_fits_it() {
        // As wide as the picture: a picture narrower than the canvas's shape.
        assert_eq!(fitted(CANVAS, (1920, 800)), (1422, 800));
        // A picture wider than the canvas's shape: as tall as the picture.
        assert_eq!(fitted(CANVAS, (2592, 1080)), (1920, 1080));
        // The same shape: the whole picture.
        assert_eq!(fitted(CANVAS, (1280, 720)), (1280, 720));
        // A taller frame than the picture: never taller than the picture.
        assert_eq!(fitted((480, 360), (320, 180)), (240, 180));
    }

    #[test]
    fn a_card_that_sizes_what_it_lays_is_never_handed_it_sized() {
        // The open interface fits it with a formula, so nothing is measured
        // for it, and a wrong measure could not put the words out of place.
        let card = Card::unproved(
            CardPath::Vaapi,
            "vaapi:0000:03:00.0".to_string(),
            "Intel".to_string(),
            std::path::PathBuf::from("/dev/dri/renderD128"),
            "/dev/dri/renderD128".to_string(),
        );
        assert_eq!(sized_for(&card, (3840, 2160), Some(1080), CANVAS), None);

        let nvidia = Card {
            way: CardPath::Cuda,
            ..card
        };
        assert_eq!(
            sized_for(&nvidia, (3840, 2160), Some(1080), CANVAS),
            Some((1920, 1080))
        );
        // A card that cannot make a picture smaller leaves it at its size, and
        // the subtitle is fitted to that.
        let unscaled = Card {
            can_scale: false,
            ..nvidia
        };
        assert_eq!(
            sized_for(&unscaled, (3840, 2160), Some(1080), CANVAS),
            Some((3840, 2160))
        );
    }

    #[test]
    fn on_an_nvidia_card_the_picture_is_put_in_the_layout_that_takes_a_transparent_subtitle() {
        let place = Place::Card {
            way: CardPath::Cuda,
            layout: "yuva420p",
            sized: Some((1440, 800)),
        };
        assert_eq!(
            graph("[0:0]", None, 3, &place),
            "[0:0]scale_cuda=format=yuv420p[picture];\
             [0:3]scale=1440:800,format=yuva420p,hwupload[words];\
             [picture][words]overlay_cuda=x='(main_w-overlay_w)/2':y='main_h-overlay_h'[painted]"
        );
        assert_eq!(CardPath::Cuda.driver().subtitle_layouts(), &["yuva420p"]);
    }

    #[test]
    fn a_tool_that_complains_of_every_frame_is_not_kept_for_ever() {
        let mut said = ToolSaid {
            painting: false,
            said: 0,
            tail: VecDeque::new(),
        };
        for number in 0..100 {
            said.hear(&format!("line {number}"));
        }
        let tail = said.tail();
        assert!(tail.starts_with("line 80 | "), "{tail}");
        assert!(tail.ends_with("line 99"), "{tail}");
    }

    #[test]
    fn what_the_tool_holds_is_read_from_the_system() {
        let status = "Name:\tffmpeg\nVmPeak:\t 9000000 kB\nVmRSS:\t 6815744 kB\nThreads:\t12\n";
        assert_eq!(resident_kilobytes_in(status), Some(6_815_744));
        assert_eq!(resident_kilobytes_in("Name:\tffmpeg\n"), None);
    }
}
