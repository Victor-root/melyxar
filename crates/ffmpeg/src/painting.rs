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
//! Everything said here is said under one tag of the journal, `painting`,
//! because nothing proves this on the maintainer's machine but the maintainer's
//! machine: what was chosen and why, what the tool itself complained about, and
//! how much memory and time it takes while it works.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use melyxar_core::time::Millis;

use crate::command::{Command, VideoEncode};
use crate::hardware::Card;

/// The canvas a subtitle made of pictures is drawn on: the one every Blu-ray
/// authors them for.
pub(crate) const CANVAS: &str = "1920x1080";

/// What the painted picture is called inside the filter graph.
pub(crate) const PAINTED: &str = "[painted]";

/// The layouts a card is offered the subtitle in, in the order they are tried.
///
/// The first is the one the tool itself draws a subtitle in, so it costs no
/// conversion. The second is there for a driver that does not take it: which
/// one a card accepts is a thing to establish, not to presume.
pub(crate) const LAYOUTS: &[&str] = &["bgra", "rgba"];

/// Where the subtitle lies on the picture.
///
/// As wide as the picture lets it be without changing shape, and never taller
/// than the picture either: the first bound meets a subtitle drawn for a frame
/// as wide as the picture, the second one drawn for a taller frame. Centred
/// across, at the foot.
const PLACEMENT: &str = "w='min(main_w,main_h*overlay_iw/overlay_ih)'\
    :h='min(main_h,main_w*overlay_ih/overlay_iw)'\
    :x='(main_w-w)/2':y='main_h-h'";

/// Where the picture is when the subtitle is laid on it.
pub(crate) enum Place<'a> {
    Processor,
    Card { way: &'a str, layout: &'a str },
}

impl<'a> Place<'a> {
    /// Where a rebuild lays a subtitle: on the card when it was proved to,
    /// otherwise on the processor.
    pub(crate) fn of(encode: &'a VideoEncode) -> Self {
        let proved = encode
            .card()
            .and_then(|(card, _)| card.picture_subtitle_layout().map(|layout| (card, layout)));
        match proved {
            Some((card, layout)) => Self::Card {
                way: card.way.as_str(),
                layout,
            },
            None => Self::Processor,
        }
    }
}

/// The filter graph that lays a subtitle onto the picture.
///
/// `picture` names the stream the picture comes from and `before` is
/// everything done to it first: painting the words on and then shrinking would
/// shrink the words with it, which is how subtitles end up unreadable on a
/// small screen.
pub(crate) fn graph(picture: &str, before: Option<&str>, subtitle: i32, place: &Place) -> String {
    match place {
        Place::Processor => {
            let picture = match before {
                Some(filters) => format!("{picture}{filters}[picture];[picture]"),
                None => picture.to_string(),
            };
            format!("{picture}[0:{subtitle}]overlay=shortest=0{PAINTED}")
        }
        // The subtitle is handed up to the card in a layout with an alpha
        // channel, which is what lets the card see through what is not
        // lettering. The picture is already up there.
        Place::Card { way, layout } => format!(
            "{picture}{}[picture];\
             [0:{subtitle}]format={layout},hwupload[words];\
             [picture][words]overlay_{way}={PLACEMENT}{PAINTED}",
            before.unwrap_or("null")
        ),
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
        (Place::Processor, Some(_)) => "processor, this card was never proved to paint",
        (Place::Processor, None) => "processor",
    };
    let layout = match place {
        Place::Card { layout, .. } => Some(layout),
        Place::Processor => None,
    };
    tracing::debug!(
        session,
        index,
        subtitle_stream = subtitle,
        painted_by,
        film_read_by = encode
            .card()
            .map(|(_, reads)| if reads { "card" } else { "processor" }),
        layout,
        canvas = CANVAS,
        scale_to_height = encode.scale_to_height,
        tone_map = encode.tone_map,
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
        way: card.way.as_str(),
        layout,
    };
    let before = card
        .filters_for(Some(crate::hardware::TRIAL_HEIGHT), false, false)
        .join(",");
    let graph = graph("[0:0]", Some(&before), 1, &place);

    ["-hide_banner", "-nostdin", "-loglevel", "error"]
        .map(str::to_string)
        .into_iter()
        .chain(card.opening_arguments(false))
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
        let graph = graph("[0:0]", Some("scale=-2:1080"), 3, &Place::Processor);
        assert_eq!(
            graph,
            "[0:0]scale=-2:1080[picture];[picture][0:3]overlay=shortest=0[painted]"
        );
    }

    #[test]
    fn on_a_card_the_picture_stays_there_and_only_the_subtitle_is_handed_up() {
        let place = Place::Card {
            way: "vaapi",
            layout: "bgra",
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
            way: "vaapi",
            layout: "bgra",
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
            way: "vaapi",
            layout: "bgra",
        };
        let graph = graph("[0:0]", None, 3, &place);
        assert!(graph.starts_with("[0:0]null[picture];"), "{graph}");
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
