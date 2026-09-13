//! The playlist, written by the server.
//!
//! This is the decision the whole of streaming rests on: the server knows how
//! long the film is, so it writes the whole playlist at once, from the first
//! segment to the last, before a single one has been produced. A player can
//! then jump anywhere, ask for the segment it landed on, and the server starts
//! producing from there.
//!
//! The usual shortcut is to let the media tool write the playlist as it goes.
//! It works until someone drags the cursor forward, at which point the player
//! is asking for a segment the playlist does not mention yet, and the whole
//! thing has to be restarted and reloaded. Owning the playlist is what makes
//! moving through a film ordinary rather than a special case.

use std::fmt::Write;

use melyxar_core::time::Millis;

/// How long one segment lasts.
///
/// Four seconds is the usual compromise: short enough that starting a film or
/// jumping in it does not mean producing much, long enough that a two hour
/// film is under two thousand segments and the playlist stays small.
pub const SEGMENT_DURATION: Millis = Millis::new(4_000);

/// The playlist of one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    /// How long the film runs.
    pub total: Millis,
    /// Nominal length of one segment.
    pub segment: Millis,
}

impl Playlist {
    pub fn new(total: Millis) -> Self {
        Self {
            total,
            segment: SEGMENT_DURATION,
        }
    }

    /// How many segments the film is cut into.
    ///
    /// The last one is shorter than the others unless the film divides evenly,
    /// and it is still a segment: leaving it out would cut the ending off.
    pub fn segment_count(&self) -> u32 {
        if self.total.get() <= 0 || self.segment.get() <= 0 {
            return 0;
        }
        let full = self.total.get() / self.segment.get();
        let remainder = self.total.get() % self.segment.get();
        (full + i64::from(remainder > 0)) as u32
    }

    /// How long one segment lasts, the last one included.
    pub fn duration_of(&self, index: u32) -> Option<Millis> {
        if index >= self.segment_count() {
            return None;
        }
        let start = self.segment.get() * i64::from(index);
        Some(Millis::new(
            (self.total.get() - start).min(self.segment.get()),
        ))
    }

    /// Where a segment starts in the film.
    pub fn start_of(&self, index: u32) -> Millis {
        Millis::new(self.segment.get() * i64::from(index))
    }

    /// The playlist as a player reads it.
    ///
    /// Written out here rather than handed to the media tool, which is the
    /// whole point: every segment is listed before any of them exists.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        let target = (self.segment.get() + 999) / 1000;

        text.push_str("#EXTM3U\n");
        // Version seven is what fragmented segments need.
        text.push_str("#EXT-X-VERSION:7\n");
        text.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        // Every segment can be decoded without the one before it, which is
        // what lets a player start in the middle.
        text.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n");
        let _ = writeln!(text, "#EXT-X-TARGETDURATION:{target}");
        text.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        // The header shared by every segment, produced once with the first.
        text.push_str("#EXT-X-MAP:URI=\"init.mp4\"\n");

        for index in 0..self.segment_count() {
            let seconds = self
                .duration_of(index)
                .map(|value| value.get() as f64 / 1000.0)
                .unwrap_or_default();
            let _ = writeln!(text, "#EXTINF:{seconds:.3},");
            let _ = writeln!(text, "segment-{index}.m4s");
        }

        text.push_str("#EXT-X-ENDLIST\n");
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_film_is_cut_into_whole_segments_and_a_last_short_one() {
        let playlist = Playlist::new(Millis::new(10_000));
        assert_eq!(playlist.segment_count(), 3);
        assert_eq!(playlist.duration_of(0), Some(Millis::new(4_000)));
        assert_eq!(playlist.duration_of(1), Some(Millis::new(4_000)));
        assert_eq!(
            playlist.duration_of(2),
            Some(Millis::new(2_000)),
            "the ending is a segment too, however short"
        );
        assert_eq!(playlist.duration_of(3), None);
    }

    #[test]
    fn a_film_that_divides_evenly_has_no_short_segment() {
        let playlist = Playlist::new(Millis::new(12_000));
        assert_eq!(playlist.segment_count(), 3);
        assert_eq!(playlist.duration_of(2), Some(Millis::new(4_000)));
    }

    #[test]
    fn a_film_of_no_length_has_nothing_to_play_rather_than_one_empty_segment() {
        assert_eq!(Playlist::new(Millis::ZERO).segment_count(), 0);
        assert_eq!(Playlist::new(Millis::new(-1)).segment_count(), 0);
    }

    #[test]
    fn a_segment_starts_where_the_one_before_it_ended() {
        let playlist = Playlist::new(Millis::new(10_000));
        assert_eq!(playlist.start_of(0), Millis::ZERO);
        assert_eq!(playlist.start_of(1), Millis::new(4_000));
        assert_eq!(playlist.start_of(2), Millis::new(8_000));
    }

    #[test]
    fn the_whole_film_is_listed_before_any_of_it_exists() {
        let text = Playlist::new(Millis::new(10_000)).to_text();

        assert!(text.starts_with("#EXTM3U\n"));
        assert!(text.contains("#EXT-X-PLAYLIST-TYPE:VOD"));
        assert!(text.contains("#EXT-X-MAP:URI=\"init.mp4\""));
        assert!(text.contains("#EXT-X-TARGETDURATION:4"));
        assert!(
            text.contains("segment-0.m4s") && text.contains("segment-2.m4s"),
            "a player must be able to jump to the end without waiting: {text}"
        );
        assert!(text.trim_end().ends_with("#EXT-X-ENDLIST"));
        assert_eq!(
            text.matches("#EXTINF:").count(),
            3,
            "one line per segment, and the last one says two seconds"
        );
        assert!(text.contains("#EXTINF:2.000,"));
    }

    #[test]
    fn a_two_hour_film_stays_a_small_playlist() {
        let playlist = Playlist::new(Millis::new(2 * 3_600_000));
        assert_eq!(playlist.segment_count(), 1_800);
        assert!(
            playlist.to_text().len() < 64 * 1024,
            "a playlist is fetched before anything plays, so it stays small"
        );
    }
}
