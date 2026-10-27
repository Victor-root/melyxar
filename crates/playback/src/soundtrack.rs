//! The soundtrack an episode starts with when another episode of its series
//! had one chosen by hand.
//!
//! A season whose files all carry the same soundtracks in the same order is a
//! season where the third one is the same third one in every episode: chosen
//! once, it is chosen for all of them. Two French tracks of one season, the
//! dubbing of France and that of Quebec, cannot be told apart by their
//! language, and the language preferred in general picked the first of them
//! again at each episode.
//!
//! Only when the soundtracks are laid out the same: an episode whose tracks
//! differ is one where the place of a track says nothing, and it is left to the
//! rules that decide for any film.

use melyxar_core::id::TrackId;
use melyxar_core::media::{normalise_language, AudioDetails, Track, TrackKind};

/// What a soundtrack is, as far as telling it from the others goes: what it
/// is in, what it is called, how it is made.
fn looks_like(
    track: &Track,
    details: &AudioDetails,
) -> (Option<String>, Option<String>, String, i32) {
    (
        track.language.as_deref().map(normalise_language),
        track
            .title
            .as_deref()
            .map(|title| title.trim().to_lowercase()),
        details.codec.to_lowercase(),
        details.channels,
    )
}

/// The soundtracks of a file, in the order the file holds them.
fn soundtracks(tracks: &[Track]) -> Vec<(&Track, &AudioDetails)> {
    let mut found: Vec<(&Track, &AudioDetails)> = tracks
        .iter()
        .filter_map(|track| match &track.kind {
            TrackKind::Audio(details) => Some((track, details)),
            _ => None,
        })
        .collect();
    found.sort_by_key(|(track, _)| track.stream_index);
    found
}

/// In `here`, the soundtrack in the place `chosen` held among those of
/// `there`, when both files carry the same soundtracks in the same order.
pub fn the_same_soundtrack<'a>(
    there: &[Track],
    chosen: TrackId,
    here: &'a [Track],
) -> Option<&'a Track> {
    let there = soundtracks(there);
    let here = soundtracks(here);
    if there.len() != here.len() {
        return None;
    }
    let laid_out_the_same = there
        .iter()
        .zip(&here)
        .all(|((one, of_one), (other, of_other))| {
            looks_like(one, of_one) == looks_like(other, of_other)
        });
    if !laid_out_the_same {
        return None;
    }
    let place = there.iter().position(|(track, _)| track.id == chosen)?;
    here.get(place).map(|(track, _)| *track)
}

#[cfg(test)]
mod tests {
    use melyxar_core::id::MediaSourceId;
    use melyxar_core::media::{Loudness, SubtitleDetails, SubtitleLayout};

    use super::*;

    fn soundtrack(stream_index: i32, language: &str, title: Option<&str>, channels: i32) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index,
            language: Some(language.to_string()),
            title: title.map(str::to_string),
            is_default: stream_index == 1,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "eac3".into(),
                profile: None,
                channels,
                channel_layout: None,
                sample_rate: Some(48_000),
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        }
    }

    /// An episode with an English track and two French ones.
    fn episode() -> Vec<Track> {
        vec![
            soundtrack(1, "eng", None, 6),
            soundtrack(2, "fre", Some("VFF"), 6),
            soundtrack(3, "fre", Some("VFQ"), 6),
        ]
    }

    #[test]
    fn the_track_chosen_in_one_episode_is_the_one_of_the_next() {
        let (before, next) = (episode(), episode());
        let found = the_same_soundtrack(&before, before[2].id, &next).expect("found");
        assert_eq!(found.id, next[2].id);
    }

    #[test]
    fn the_same_language_written_another_way_is_the_same_track() {
        let before = episode();
        let mut next = episode();
        next[1].language = Some("fra".into());
        next[2].language = Some("fr".into());
        assert_eq!(
            the_same_soundtrack(&before, before[2].id, &next).map(|track| track.id),
            Some(next[2].id)
        );
    }

    #[test]
    fn soundtracks_laid_out_otherwise_say_nothing() {
        let before = episode();

        let mut fewer = episode();
        fewer.pop();
        assert!(the_same_soundtrack(&before, before[2].id, &fewer).is_none());

        let mut swapped = episode();
        swapped[1].title = Some("VFQ".into());
        swapped[2].title = Some("VFF".into());
        assert!(the_same_soundtrack(&before, before[2].id, &swapped).is_none());

        let mut stereo = episode();
        stereo[2] = soundtrack(3, "fre", Some("VFQ"), 2);
        assert!(the_same_soundtrack(&before, before[2].id, &stereo).is_none());
    }

    #[test]
    fn only_the_soundtracks_are_compared_in_the_order_of_the_file() {
        let before = episode();
        // Listed in another order, with a subtitle among them: the same file.
        let mut next = episode();
        next.reverse();
        let mut written = soundtrack(4, "fre", None, 0);
        written.kind = TrackKind::Subtitle(SubtitleDetails {
            codec: "subrip".into(),
            layout: SubtitleLayout::Text,
            is_hearing_impaired: false,
            is_generated: false,
            is_external: false,
            external_relative_path: None,
            downloaded_file: None,
        });
        next.push(written);
        let found = the_same_soundtrack(&before, before[2].id, &next).expect("found");
        assert_eq!(found.title.as_deref(), Some("VFQ"));
    }

    #[test]
    fn a_track_that_is_not_one_of_the_episode_chosen_in_finds_nothing() {
        let (before, next) = (episode(), episode());
        assert!(the_same_soundtrack(&before, TrackId::new(), &next).is_none());
    }
}
