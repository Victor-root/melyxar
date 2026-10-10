//! Looking for every card of the machine and proving what each can do.
//!
//! Nothing here knows one maker from another. What a card is found by, opened
//! with and asked is its path's business, and the search only runs the trials
//! in order and keeps what each said.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use super::trials::{
    run_briefly, sample_in, try_it, try_reading, wide_gamut_sample, witness_writer, Sample,
    TrialInput, TRIAL_HEIGHT, WIDE_GAMUT_CODEC,
};
use super::{encoder_name, Card, CardPath, ToneMapping};
use crate::painting;

/// The codecs a card is asked about, cheapest for a viewer to decode last.
///
/// The first one is not the preferred one: that choice belongs with the client,
/// which is the only party that knows what it can decode. This is only the list
/// of what is worth establishing.
const WORTH_TRYING: &[&str] = &["h264", "hevc", "av1"];

/// The codec every client reads, and therefore the floor.
///
/// A card that cannot produce this one is not used at all: there would be
/// clients it could serve nothing to, and falling back per client is a worse
/// answer than a card that is simply not there.
const THE_FLOOR: &str = "h264";

/// One thing that was tried, and what came of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trial {
    /// What was being established, as a code a page turns into a sentence.
    pub what: String,
    pub device: String,
    pub worked: bool,
    /// What the tool printed when it refused. Empty when it did not.
    pub said: String,
}

/// Everything the search found, whether or not it found a card.
///
/// Kept whole rather than reduced to a yes or a no: a card that was refused is
/// a question with an answer, and the answer is in here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CardSearch {
    /// The devices the machine offered, by name. Empty means the container was
    /// never given them.
    pub devices: Vec<String>,
    pub trials: Vec<Trial>,
    /// Every card that passed, in the order they were found.
    pub cards: Vec<Card>,
}

/// The trial that tells a forbidden device from a driverless one.
const OPENING: &str = "open_the_device";

impl CardSearch {
    /// Whether a device was there and this account could open it.
    ///
    /// The question worth asking when no card was found: a device that would
    /// not open is a permission to grant, and a device that opened and then
    /// answered nothing is a driver to install. Nothing else separates them.
    pub fn a_device_opened(&self) -> bool {
        self.trials
            .iter()
            .any(|trial| trial.what == OPENING && trial.worked)
    }

    /// The devices that opened and yet gave no card: on a machine with several
    /// cards, the one whose driver is missing is told apart from the ones that
    /// work, which `a_device_opened` cannot do.
    pub fn opened_without_a_card(&self) -> Vec<&str> {
        self.trials
            .iter()
            .filter(|trial| trial.what == OPENING && trial.worked)
            .map(|trial| trial.device.as_str())
            .filter(|device| {
                !self
                    .cards
                    .iter()
                    .any(|card| card.device.to_str() == Some(*device))
            })
            .collect()
    }

    /// The card that rebuilds pictures: the one chosen, when it passed here,
    /// otherwise the one that keeps the most away from the processor, the
    /// first found among equals.
    pub fn card(&self, chosen: Option<&str>) -> Option<&Card> {
        chosen
            .and_then(|key| self.cards.iter().find(|card| card.key == key))
            .or_else(|| self.cards.iter().rev().max_by_key(|card| card.reach()))
    }

    /// Looks for every card and proves what each can do, or explains itself.
    ///
    /// `encoders` is what the tool was built with: there is no point trying a
    /// path the binary does not carry, and saying so is a clearer answer than
    /// a driver failure.
    pub async fn run(ffmpeg: &Path, encoders: &BTreeSet<String>) -> Self {
        let mut search = Self::default();
        let mut found = Vec::new();

        for way in CardPath::ALL {
            let cards = way.driver().find();
            tracing::debug!(
                way = way.as_str(),
                found = ?cards.iter().map(|card| (&card.key, &card.name, &card.address)).collect::<Vec<_>>(),
                "looked for the cards this path drives"
            );
            search
                .devices
                .extend(cards.iter().map(|card| card.device.display().to_string()));
            found.extend(search.what_this_build_drives(way, cards, encoders));
        }
        if found.is_empty() {
            return search;
        }

        // Made once for every card: it stands in for a real wide gamut film
        // and nothing about it depends on the card it is shown to.
        let wide_gamut = wide_gamut_sample(ffmpeg).await;
        if let Err(said) = &wide_gamut {
            search.trials.push(Trial {
                what: "make_a_wide_gamut_sample".to_string(),
                device: String::new(),
                worked: false,
                said: said.clone(),
            });
        }
        let wide_gamut = wide_gamut.ok();

        for card in found {
            if let Some(card) = search
                .try_this_card(ffmpeg, card, encoders, wide_gamut.as_ref())
                .await
            {
                search.cards.push(card);
            }
        }

        search
    }

    /// The cards of one path, when the tool was built to drive that path.
    ///
    /// Said only of a machine that has such a card: a build without Nvidia's
    /// encoders is no news on a machine without an Nvidia card.
    fn what_this_build_drives(
        &mut self,
        way: CardPath,
        cards: Vec<Card>,
        encoders: &BTreeSet<String>,
    ) -> Vec<Card> {
        let floor = encoder_name(THE_FLOOR, way);
        if cards.is_empty() || encoders.contains(&floor) {
            return cards;
        }
        for card in &cards {
            self.trials.push(Trial {
                what: "built_with_the_path".to_string(),
                device: card.device.display().to_string(),
                worked: false,
                said: format!("this build carries no {floor} encoder"),
            });
        }
        Vec::new()
    }

    /// Establishes what one card can do, or nothing when it cannot encode at
    /// all.
    async fn try_this_card(
        &mut self,
        ffmpeg: &Path,
        mut card: Card,
        built_with: &BTreeSet<String>,
        wide_gamut: Option<&Sample>,
    ) -> Option<Card> {
        let named = card.device.display().to_string();

        // Asked first, because it is what tells the two failures apart. A
        // device that will not open is an account that is not allowed to use
        // it; a device that opens and then answers nothing is a driver that is
        // not installed. Both come out of the media tool as the same sentence
        // about no display being found, and they are fixed in entirely
        // different places.
        if let Err(error) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&card.device)
        {
            self.trials.push(Trial {
                what: OPENING.to_string(),
                device: named,
                worked: false,
                said: format!(
                    "{error}; the account this server runs as has to be allowed to open it, \
                     which in an unprivileged container means belonging to the group that \
                     owns it inside the container"
                ),
            });
            return None;
        }
        self.trials.push(Trial {
            what: OPENING.to_string(),
            device: named.clone(),
            worked: true,
            said: String::new(),
        });

        for codec in WORTH_TRYING {
            let encoder = encoder_name(codec, card.way);
            if !built_with.contains(&encoder) {
                continue;
            }
            let (worked, said) = try_it(
                ffmpeg,
                card.opening_arguments(false, false),
                &encoder,
                TrialInput::Generated,
                "format=nv12,hwupload",
            )
            .await;
            self.trials.push(Trial {
                what: format!("rebuild_{codec}"),
                device: named.clone(),
                worked,
                said,
            });
            if worked {
                card.encoders.insert((*codec).to_string(), encoder);
            }
        }

        // Without the codec every client reads there would be clients this
        // card could serve nothing to, which is worse than no card at all.
        let floor = card.encoder_for(THE_FLOOR)?.to_string();

        // The remaining trials run the chain a real film will run, rather than
        // something that resembles it: a filter that works on its own and
        // refuses what comes out of the one before it is exactly the failure
        // that only shows up in the middle of somebody's film.
        let chain = card.chain(Some(TRIAL_HEIGHT), None, false).join(",");
        let (can_scale, said) = try_it(
            ffmpeg,
            card.opening_arguments(false, false),
            &floor,
            TrialInput::Generated,
            &chain,
        )
        .await;
        self.trials.push(Trial {
            what: "make_it_smaller".to_string(),
            device: named.clone(),
            worked: can_scale,
            said,
        });
        card.can_scale = can_scale;

        card.tone_mapping = self
            .how_it_converts_colour(ffmpeg, &card, &floor, wide_gamut)
            .await;

        card.picture_subtitle_layout = self.which_layout_it_paints_in(ffmpeg, &card, &floor).await;

        card.paints_through_vulkan = card
            .tone_mapping
            .is_some_and(ToneMapping::may_paint_through_vulkan)
            && self.whether_vulkan_paints(ffmpeg, &card, &floor).await;

        card.decoders = self
            .which_codecs_it_reads(ffmpeg, &card, &floor, wide_gamut, &named, built_with)
            .await;

        Some(card)
    }

    /// Establishes how the card converts wide gamut colour, trying what its
    /// path offers best first and keeping the first that works.
    ///
    /// The one trial that cannot be run on a picture made on the spot: it is
    /// run on the sample, which is a real wide gamut film down to the ten bits
    /// and the screen it says it was graded on.
    async fn how_it_converts_colour(
        &mut self,
        ffmpeg: &Path,
        card: &Card,
        floor: &str,
        wide_gamut: Option<&Sample>,
    ) -> Option<ToneMapping> {
        // Said once already, when the sample could not be made.
        let sample = wide_gamut?;

        for recipe in card.way.driver().recipes() {
            let reads = recipe.read_in_its_trial();
            let chain = card.chain(Some(TRIAL_HEIGHT), Some(*recipe), reads).join(",");
            let (worked, said) = try_it(
                ffmpeg,
                card.opening(reads, Some(*recipe)),
                floor,
                TrialInput::File(sample.path()),
                &chain,
            )
            .await;
            self.trials.push(Trial {
                what: recipe.trial_name().to_string(),
                device: card.device.display().to_string(),
                worked,
                said,
            });
            if worked {
                return Some(*recipe);
            }
        }
        None
    }

    /// Establishes whether the card lays a subtitle made of pictures onto a
    /// picture, and in which layout it takes the subtitle.
    ///
    /// Run through the same graph a film will run, so that what is proved is
    /// the placement and the hand-up and not something that resembles them. The
    /// layouts are tried in order and the first that works is kept, each with
    /// what the tool said when it refused.
    async fn which_layout_it_paints_in(
        &mut self,
        ffmpeg: &Path,
        card: &Card,
        floor: &str,
    ) -> Option<String> {
        for layout in card.way.driver().subtitle_layouts() {
            let (worked, said) =
                match run_briefly(ffmpeg, painting::trial_arguments(card, floor, layout)).await {
                    Ok(outcome) => outcome,
                    Err(said) => (false, said),
                };
            self.trials.push(Trial {
                what: format!("paint_a_picture_subtitle_{layout}"),
                device: card.device.display().to_string(),
                worked,
                said,
            });
            if worked {
                return Some((*layout).to_string());
            }
        }
        None
    }

    /// Establishes whether Vulkan, converting a wide gamut picture on the card,
    /// lays a subtitle made of pictures on it, through the graph a film runs.
    async fn whether_vulkan_paints(&mut self, ffmpeg: &Path, card: &Card, floor: &str) -> bool {
        let (worked, said) =
            match run_briefly(ffmpeg, painting::trial_through_vulkan_arguments(card, floor)).await {
                Ok(outcome) => outcome,
                Err(said) => (false, said),
            };
        self.trials.push(Trial {
            what: "paint_a_picture_subtitle_through_vulkan".to_string(),
            device: card.device.display().to_string(),
            worked,
            said,
        });
        worked
    }

    /// Establishes which codecs the card reads for itself.
    ///
    /// Reading is asked separately from writing because they are separate
    /// abilities, and the only honest way to ask is to hand the card a film in
    /// that codec and see. Each sample is written first, by a software encoder
    /// of the build where it carries one, since that is the shape a real film
    /// arrives in, and by the card itself otherwise, so the question does not
    /// become "is this build carrying a software encoder for that codec".
    async fn which_codecs_it_reads(
        &mut self,
        ffmpeg: &Path,
        card: &Card,
        floor: &str,
        wide_gamut: Option<&Sample>,
        named: &str,
        built_with: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        let mut reads = BTreeSet::new();

        for codec in WORTH_TRYING {
            // A wide gamut sample carries ten bits to a channel, which is what
            // every film worth the card is, and reading eight proves nothing
            // about reading ten. Where one exists it is the better witness.
            let sample = match (*codec == WIDE_GAMUT_CODEC, wide_gamut) {
                (true, Some(sample)) => Some(Kept::Borrowed(sample)),
                _ => match witness_writer(codec, card, built_with) {
                    Some(writer) => match sample_in(ffmpeg, card, &writer).await {
                        Ok(made) => Some(Kept::Owned(made)),
                        Err(said) => {
                            self.trials.push(Trial {
                                what: format!("make_a_{codec}_sample"),
                                device: named.to_string(),
                                worked: false,
                                said,
                            });
                            None
                        }
                    },
                    // Nothing here can write this codec, so nothing here can
                    // ask whether the card reads it. Said plainly rather than
                    // recorded as a refusal it never made.
                    None => None,
                },
            };
            let Some(sample) = sample else { continue };

            // Run through the chain a real film in that codec runs: the wide
            // gamut one is a wide gamut film and goes through the conversion,
            // the others through what an ordinary film goes through. A card
            // that reads a film and then offers the frames in a layout the
            // encoder refuses has not read it, as far as a viewer is
            // concerned, and that refusal comes at the end of the chain.
            //
            // What is asked here is what the card's own reader reads, so the
            // wide gamut sample is converted only by a recipe that reader
            // feeds: a recipe that reads for itself is proved to with that recipe.
            let tone_map = *codec == WIDE_GAMUT_CODEC
                && card
                    .tone_mapping
                    .is_some_and(ToneMapping::fed_by_the_cards_own_reader);
            let chain = card.filters_for(None, tone_map, true).join(",");
            let (worked, said) = try_reading(
                ffmpeg,
                card.opening_arguments(true, tone_map),
                floor,
                sample.path(),
                &chain,
            )
            .await;
            self.trials.push(Trial {
                what: format!("read_{codec}"),
                device: named.to_string(),
                worked,
                said,
            });
            if worked {
                reads.insert((*codec).to_string());
            }
        }

        reads
    }
}

/// A sample this trial owns, or one it was lent.
enum Kept<'a> {
    Owned(Sample),
    Borrowed(&'a Sample),
}

impl Kept<'_> {
    fn path(&self) -> &Path {
        match self {
            Self::Owned(sample) => sample.path(),
            Self::Borrowed(sample) => sample.path(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_cards::{intel as card, nvidia as nvidia_card};
    use super::*;
    use crate::ToolPaths;

    #[test]
    fn the_chosen_card_does_the_work_when_it_passed_here() {
        let search = CardSearch {
            cards: vec![card(true, true), nvidia_card()],
            ..CardSearch::default()
        };
        assert_eq!(
            search.card(Some("cuda:0000:0c:00.0")).map(|card| card.way),
            Some(CardPath::Cuda)
        );
        // A choice kept for a card that is gone is no reason to use none.
        assert_eq!(
            search.card(Some("cuda:0000:01:00.0")).map(|card| card.way),
            Some(CardPath::Vaapi)
        );
    }

    #[test]
    fn without_a_choice_the_card_that_keeps_the_most_off_the_processor_works() {
        // Converting colour first: a film that needs it and is refused goes
        // to the processor whole, whatever else the other card does better.
        let search = CardSearch {
            cards: vec![nvidia_card(), card(true, true)],
            ..CardSearch::default()
        };
        assert_eq!(
            search.card(None).map(|card| card.way),
            Some(CardPath::Vaapi)
        );

        // Among equals, the first one found.
        let twins = CardSearch {
            cards: vec![
                card(true, false),
                Card {
                    key: "vaapi:0000:04:00.0".to_string(),
                    ..card(true, false)
                },
            ],
            ..CardSearch::default()
        };
        assert_eq!(
            twins.card(None).map(|card| card.key.as_str()),
            Some("vaapi:0000:03:00.0")
        );
        assert!(CardSearch::default().card(None).is_none());
    }

    #[test]
    fn a_path_this_build_does_not_carry_is_said_plainly_rather_than_tried() {
        let mut search = CardSearch::default();
        let kept = search.what_this_build_drives(
            CardPath::Cuda,
            vec![nvidia_card()],
            &["h264_vaapi".to_string()].into_iter().collect(),
        );

        assert!(kept.is_empty());
        assert_eq!(search.trials.len(), 1);
        assert_eq!(search.trials[0].what, "built_with_the_path");
        assert_eq!(search.trials[0].device, "/dev/nvidia0");
        assert!(search.trials[0].said.contains("h264_nvenc"));
    }

    #[test]
    fn a_build_without_a_path_is_no_news_on_a_machine_without_its_cards() {
        let mut search = CardSearch::default();
        let kept =
            search.what_this_build_drives(CardPath::Cuda, Vec::new(), &BTreeSet::new());
        assert!(kept.is_empty());
        assert!(search.trials.is_empty(), "{:?}", search.trials);

        let carried = search.what_this_build_drives(
            CardPath::Cuda,
            vec![nvidia_card()],
            &["h264_nvenc".to_string()].into_iter().collect(),
        );
        assert_eq!(carried.len(), 1);
        assert!(search.trials.is_empty());
    }

    #[test]
    fn a_forbidden_device_and_a_driverless_one_are_told_apart() {
        // The media tool words both the same way, as no display being found,
        // and they are fixed in entirely different places: one is a permission
        // to grant, the other a package to install.
        let opened = CardSearch {
            trials: vec![Trial {
                what: OPENING.to_string(),
                device: "/dev/dri/renderD128".to_string(),
                worked: true,
                said: String::new(),
            }],
            ..CardSearch::default()
        };
        assert!(opened.a_device_opened());

        let forbidden = CardSearch {
            trials: vec![Trial {
                what: OPENING.to_string(),
                device: "/dev/dri/renderD128".to_string(),
                worked: false,
                said: "permission denied".to_string(),
            }],
            ..CardSearch::default()
        };
        assert!(!forbidden.a_device_opened());
        assert!(!CardSearch::default().a_device_opened());
    }

    #[test]
    fn a_device_that_opened_and_gave_no_card_is_told_apart_from_the_ones_that_work() {
        let opened = |device: &str| Trial {
            what: OPENING.to_string(),
            device: device.to_string(),
            worked: true,
            said: String::new(),
        };
        let search = CardSearch {
            trials: vec![opened("/dev/dri/renderD128"), opened("/dev/dri/renderD129")],
            cards: vec![card(true, true)],
            ..CardSearch::default()
        };
        assert_eq!(search.opened_without_a_card(), vec!["/dev/dri/renderD129"]);
        assert!(CardSearch::default().opened_without_a_card().is_empty());
    }

    #[tokio::test]
    async fn a_machine_with_no_card_says_so_without_claiming_one() {
        // What a container that was never given the graphics device looks
        // like, and what this working environment is.
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let encoders = ["h264_vaapi".to_string(), "h264_nvenc".to_string()]
            .into_iter()
            .collect();
        let search = CardSearch::run(&tools.ffmpeg, &encoders).await;

        if search.devices.is_empty() {
            assert!(search.cards.is_empty(), "there is no device to have used");
            assert!(
                search.trials.is_empty(),
                "nothing to try, and nothing is claimed: {:?}",
                search.trials
            );
        } else {
            // A machine that does have one: whatever the outcome, every trial
            // that failed carries what the tool said about it.
            for trial in &search.trials {
                assert!(
                    trial.worked || !trial.said.is_empty(),
                    "a refusal with nothing to read is a debugging session: {trial:?}"
                );
            }
        }
    }
}
