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
///
/// Held as the place each segment begins rather than as a length, because the
/// two are the same thing only when the server is free to cut where it likes.
/// It is, when it rebuilds the picture: it puts a key frame on every boundary
/// itself. It is not when the picture is carried over untouched, where a
/// segment can only begin where the film already has a picture that stands on
/// its own, and those are wherever the encoder left them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    /// How long the film runs.
    pub total: Millis,
    /// Where each segment begins, ascending, the first one at nothing.
    starts: Vec<Millis>,
    /// Whether those places are where the film can really be started, rather
    /// than a grid the server chose.
    ///
    /// It changes what can be believed downstream: a stream carried over
    /// untouched and set going on one of these begins exactly there, where set
    /// going on a grid it begins at the key frame before and nobody knows how
    /// far before.
    on_key_frames: bool,
}

impl Playlist {
    /// A playlist on the fixed grid, for a picture the server rebuilds.
    ///
    /// Sound on its own takes this too: it can be cut anywhere at all.
    pub fn on_a_fixed_grid(total: Millis) -> Self {
        let mut starts = Vec::new();
        let mut at = 0;
        while at < total.get() {
            starts.push(Millis::new(at));
            at += SEGMENT_DURATION.get();
        }
        Self {
            total,
            starts,
            on_key_frames: false,
        }
    }

    /// A playlist cut where this film can really be started.
    ///
    /// Given the boundaries in order, one segment each: how they were chosen
    /// is settled before they get here, and choosing again would be a second
    /// opinion the tool producing the segments has not been told about.
    pub fn on_these_boundaries(total: Millis, boundaries: &[Millis]) -> Self {
        let starts: Vec<Millis> = boundaries
            .iter()
            .copied()
            .filter(|start| start.get() >= 0 && start.get() < total.get())
            .collect();

        // A film that yielded nothing usable is cut the only way left, which
        // is at least a playlist that plays rather than none at all.
        match starts.first() {
            Some(first) if first.get() == 0 => Self {
                total,
                starts,
                on_key_frames: true,
            },
            _ => Self::on_a_fixed_grid(total),
        }
    }

    /// Whether the segments begin where the film can really be started.
    pub fn cut_where_the_film_allows(&self) -> bool {
        self.on_key_frames
    }

    /// Which segment holds one moment of the film.
    ///
    /// The last one whose beginning is at or before it. Past the end of the
    /// film that is the last segment, which is where anything past the end
    /// belongs.
    pub fn segment_holding(&self, position: Millis) -> u32 {
        match self
            .starts
            .partition_point(|start| start.get() <= position.get())
        {
            0 => 0,
            after => (after - 1) as u32,
        }
    }

    /// How many segments the film is cut into.
    ///
    /// The last one runs to the end of the film however long that leaves it:
    /// leaving it out would cut the ending off.
    pub fn segment_count(&self) -> u32 {
        self.starts.len() as u32
    }

    /// How long one segment lasts, the last one included.
    pub fn duration_of(&self, index: u32) -> Option<Millis> {
        let start = self.starts.get(index as usize)?.get();
        let ends_at = match self.starts.get(index as usize + 1) {
            Some(next) => next.get(),
            None => self.total.get(),
        };
        Some(Millis::new(ends_at - start))
    }

    /// Where a segment starts in the film.
    ///
    /// Asked for one segment past the end, it answers the end of the film:
    /// that is what "past the end of the last segment" means, and the wait for
    /// a segment is expressed that way.
    pub fn start_of(&self, index: u32) -> Millis {
        match self.starts.get(index as usize) {
            Some(start) => *start,
            None => self.total,
        }
    }

    /// The playlist as a player reads it, beginning where the viewer left off.
    ///
    /// Written out here rather than handed to the media tool, which is the
    /// whole point: every segment is listed before any of them exists.
    ///
    /// The starting point is said in the playlist rather than settled by each
    /// player on its own. It is the protocol's own way of saying it, so a
    /// browser reading the playlist itself and a library doing it for one are
    /// told the same thing by the same line, and the server is producing the
    /// part of the film that is about to be asked for.
    pub fn to_text(&self, begin_at: Millis) -> String {
        let mut text = String::new();
        // The longest segment, rounded up, which is what the marker means.
        // A film cut where its own pictures allow can hold one longer than the
        // usual length, and a player told otherwise stalls on it.
        let longest = (0..self.segment_count())
            .filter_map(|index| self.duration_of(index))
            .map(Millis::get)
            .max()
            .unwrap_or(SEGMENT_DURATION.get());
        let target = (longest + 999) / 1000;

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
        // Nothing at all for a film starting at its beginning, which is what
        // a player does when told nothing.
        if begin_at > Millis::ZERO && begin_at < self.total {
            let _ = writeln!(
                text,
                "#EXT-X-START:TIME-OFFSET={:.3}",
                begin_at.as_seconds_f64()
            );
        }

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
        let playlist = Playlist::on_a_fixed_grid(Millis::new(10_000));
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
        let playlist = Playlist::on_a_fixed_grid(Millis::new(12_000));
        assert_eq!(playlist.segment_count(), 3);
        assert_eq!(playlist.duration_of(2), Some(Millis::new(4_000)));
    }

    #[test]
    fn a_film_of_no_length_has_nothing_to_play_rather_than_one_empty_segment() {
        assert_eq!(Playlist::on_a_fixed_grid(Millis::ZERO).segment_count(), 0);
        assert_eq!(
            Playlist::on_a_fixed_grid(Millis::new(-1)).segment_count(),
            0
        );
    }

    #[test]
    fn a_segment_starts_where_the_one_before_it_ended() {
        let playlist = Playlist::on_a_fixed_grid(Millis::new(10_000));
        assert_eq!(playlist.start_of(0), Millis::ZERO);
        assert_eq!(playlist.start_of(1), Millis::new(4_000));
        assert_eq!(playlist.start_of(2), Millis::new(8_000));
    }

    #[test]
    fn the_playlist_says_where_the_viewer_left_off() {
        // Said here so every player is told the same thing by the same line.
        // Left to work it out for itself, a player asks for the opening of a
        // film nobody is at, and the server produces a segment that will never
        // be seen before it can produce the one that will.
        let playlist = Playlist::on_a_fixed_grid(Millis::new(60_000));

        let from_the_middle = playlist.to_text(Millis::new(20_500));
        assert!(
            from_the_middle.contains("#EXT-X-START:TIME-OFFSET=20.500"),
            "{from_the_middle}"
        );

        assert!(
            !playlist.to_text(Millis::ZERO).contains("#EXT-X-START"),
            "a film beginning at its beginning is what a player does anyway"
        );
        assert!(
            !playlist
                .to_text(Millis::new(90_000))
                .contains("#EXT-X-START"),
            "past the end is not a place to begin"
        );
    }

    #[test]
    fn the_whole_film_is_listed_before_any_of_it_exists() {
        let text = Playlist::on_a_fixed_grid(Millis::new(10_000)).to_text(Millis::ZERO);

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
    fn a_film_is_cut_where_it_can_really_be_started() {
        // Ten seconds apart is an ordinary film. There is no picture at the
        // fourth second to start at, so the segment runs to the next place
        // there is one.
        let boundaries = [
            Millis::new(0),
            Millis::new(10_000),
            Millis::new(20_000),
            Millis::new(30_000),
        ];
        let playlist = Playlist::on_these_boundaries(Millis::new(35_000), &boundaries);

        assert!(playlist.cut_where_the_film_allows());
        assert_eq!(playlist.segment_count(), 4);
        assert_eq!(playlist.start_of(2), Millis::new(20_000));
        assert_eq!(playlist.duration_of(0), Some(Millis::new(10_000)));
        assert_eq!(
            playlist.duration_of(3),
            Some(Millis::new(5_000)),
            "the last one runs to the end of the film however long that leaves it"
        );
        assert_eq!(playlist.duration_of(4), None);
        assert_eq!(
            playlist.start_of(4),
            Millis::new(35_000),
            "one past the end is the end of the film, which is what waiting for a segment asks"
        );
    }

    #[test]
    fn the_marker_a_player_reads_covers_the_longest_segment_there_is() {
        // A film cut where its own pictures allow holds segments longer than
        // the usual length, and a player told otherwise stalls on one.
        let text = Playlist::on_these_boundaries(
            Millis::new(35_000),
            &[Millis::new(0), Millis::new(10_000), Millis::new(20_000)],
        )
        .to_text(Millis::ZERO);
        assert!(
            text.contains("#EXT-X-TARGETDURATION:15"),
            "the longest here is fifteen seconds: {text}"
        );
    }

    #[test]
    fn a_film_that_yielded_nothing_usable_is_still_cut_somehow() {
        // A playlist that plays badly beats no playlist at all, and a film
        // nobody could read for its key frames must still be watchable.
        let nothing = Playlist::on_these_boundaries(Millis::new(10_000), &[]);
        assert!(!nothing.cut_where_the_film_allows());
        assert_eq!(nothing.segment_count(), 3);

        // Boundaries that do not begin at the beginning would be a playlist
        // missing the opening of the film.
        let late = Playlist::on_these_boundaries(
            Millis::new(10_000),
            &[Millis::new(3_000), Millis::new(7_000)],
        );
        assert!(!late.cut_where_the_film_allows());
        assert_eq!(late.start_of(0), Millis::ZERO);
    }

    #[test]
    fn the_segment_holding_a_moment_is_the_one_that_began_at_or_before_it() {
        let playlist = Playlist::on_these_boundaries(
            Millis::new(35_000),
            &[Millis::new(0), Millis::new(10_000), Millis::new(20_000)],
        );
        assert_eq!(playlist.segment_holding(Millis::ZERO), 0);
        assert_eq!(playlist.segment_holding(Millis::new(9_999)), 0);
        assert_eq!(playlist.segment_holding(Millis::new(10_000)), 1);
        assert_eq!(playlist.segment_holding(Millis::new(19_999)), 1);
        assert_eq!(playlist.segment_holding(Millis::new(20_000)), 2);
        assert_eq!(
            playlist.segment_holding(Millis::new(900_000)),
            2,
            "anything past the end belongs to the last one"
        );
    }

    #[test]
    fn a_two_hour_film_stays_a_small_playlist() {
        let playlist = Playlist::on_a_fixed_grid(Millis::new(2 * 3_600_000));
        assert_eq!(playlist.segment_count(), 1_800);
        assert!(
            playlist.to_text(Millis::ZERO).len() < 64 * 1024,
            "a playlist is fetched before anything plays, so it stays small"
        );
    }
}
