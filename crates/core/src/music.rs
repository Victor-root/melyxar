//! What a song, an album and an artist are, once a library has filed them.
//!
//! Worked out by the filing of a folder, in the crate that walks the disk,
//! and written down by the database as it is: the two meet on these plain
//! values and on nothing of each other.

/// A name as it is shown, and as it is sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub sort_name: String,
}

/// The album a song goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumFiling {
    pub title: Named,
    /// Whose album it is. Empty when nobody could be told, which leaves the
    /// album under no artist rather than under a made up one.
    pub artists: Vec<Named>,
    pub is_compilation: bool,
}

/// Where one song goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongFiling {
    pub title: Named,
    /// Who plays it.
    pub artists: Vec<Named>,
    /// Absent for a song that sits at the top of a library and says nothing
    /// of an album: it is a song on its own, which is found among the songs.
    pub album: Option<AlbumFiling>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
}

/// The name a browser gives the form of a song, which is how it says what it
/// plays: `mp3`, `aac`, `flac`, `opus`, `vorbis`, `alac`, `wav`. Nothing for a
/// form no browser plays at all.
pub fn browser_form(codec: &str, container: Option<&str>) -> Option<&'static str> {
    let container = container.unwrap_or_default();
    Some(match codec {
        "mp3" => "mp3",
        "aac" => "aac",
        "flac" => "flac",
        "alac" => "alac",
        "opus" if container.contains("ogg") || container.contains("webm") => "opus",
        "vorbis" if container.contains("ogg") || container.contains("webm") => "vorbis",
        pcm if pcm.starts_with("pcm_") && container.contains("wav") => "wav",
        _ => return None,
    })
}

/// Whether a song reaches a browser as it lies on the disk, given the forms
/// that browser says it plays. Anything else is converted on the way.
pub fn plays_as_it_is(codec: &str, container: Option<&str>, plays: &[&str]) -> bool {
    browser_form(codec, container).is_some_and(|form| plays.contains(&form))
}

/// How loud ReplayGain takes a song to be once raised by its gain, in the
/// same measure the server reads songs with (EBU R128).
const REPLAY_GAIN_REFERENCE_LUFS: f64 = -18.0;

/// How loud a song is, from the gain and the peak a tagging program wrote in
/// it: a song to be lowered by six decibels to reach the reference is six
/// louder than it. The peak is written where one is full scale.
pub fn loudness_from_replay_gain(
    gain_db: Option<f64>,
    peak: Option<f64>,
) -> crate::media::Loudness {
    crate::media::Loudness {
        integrated_lufs: gain_db.map(|gain| REPLAY_GAIN_REFERENCE_LUFS - gain),
        true_peak_dbfs: peak
            .filter(|peak| *peak > 0.0)
            .map(|peak| 20.0 * peak.log10()),
        range_lu: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A_BROWSER: [&str; 5] = ["mp3", "aac", "flac", "opus", "vorbis"];

    #[test]
    fn what_a_browser_plays_goes_as_it_is_and_the_rest_is_converted() {
        assert!(plays_as_it_is("mp3", Some("mp3"), &A_BROWSER));
        assert!(plays_as_it_is("flac", Some("flac"), &A_BROWSER));
        assert!(plays_as_it_is(
            "aac",
            Some("mov,mp4,m4a,3gp,3g2,mj2"),
            &A_BROWSER
        ));
        assert!(plays_as_it_is("opus", Some("ogg"), &A_BROWSER));
        assert!(
            !plays_as_it_is("alac", Some("mov,mp4,m4a,3gp,3g2,mj2"), &A_BROWSER),
            "Apple's lossless, which this one does not play"
        );
        assert!(
            plays_as_it_is("alac", Some("mov,mp4,m4a,3gp,3g2,mj2"), &["alac"]),
            "and one that says it does"
        );
        for never in ["wmav2", "ape", "wavpack", "musepack", "dsd_lsbf"] {
            assert!(!plays_as_it_is(never, None, &A_BROWSER), "{never}");
        }
    }

    #[test]
    fn uncompressed_sound_is_played_only_from_a_wave_file() {
        assert!(plays_as_it_is("pcm_s16le", Some("wav"), &["wav"]));
        assert!(!plays_as_it_is("pcm_s16be", Some("aiff"), &["wav"]));
    }

    #[test]
    fn a_tagged_gain_says_how_loud_the_song_is() {
        let loudness = loudness_from_replay_gain(Some(-6.5), Some(1.0));
        assert_eq!(loudness.integrated_lufs, Some(-11.5));
        assert_eq!(loudness.true_peak_dbfs, Some(0.0));
        let untagged = loudness_from_replay_gain(None, None);
        assert!(!untagged.is_measured());
        assert_eq!(
            loudness_from_replay_gain(Some(2.0), Some(0.0)).true_peak_dbfs,
            None
        );
    }

    #[test]
    fn a_browser_that_says_nothing_is_sent_everything_converted() {
        assert!(!plays_as_it_is("mp3", Some("mp3"), &[]));
    }
}
