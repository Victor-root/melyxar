//! Lining the lyrics of a song up with its sound, for an administrator.
//!
//! The song is read through once, as plain samples, and the places where a
//! voice starts are worked out from them (see `melyxar_sound`); the stamps of
//! its lyrics are then lined up with those. What comes of it is kept as how
//! far each line was moved, not as other lyrics: the words and stamps as
//! written stay as they were, and the moves are put back on them each time
//! they are read, as long as they still have the same lines.
//!
//! Nothing is believed that the sound does not make plain. A song whose voice
//! cannot be told from its instruments, or whose lyrics have too few lines to
//! say, is left as it is and told so.

use melyxar_core::id::WorkId;
use melyxar_core::time::Millis;
use melyxar_core::user::User;
use melyxar_database::music_lyrics_timing::LyricsTiming;
use melyxar_ffmpeg::AskedToStop;
use melyxar_ffmpeg::sound::song_samples;
use melyxar_sound::{Stamp, Verdict, align, voice_onsets};

use crate::music::lyrics::found_lyrics;
use crate::reach::may_read_the_work;
use crate::{AppError, AppState, Result};

/// The longest song that is read through: a song, not an audiobook, which is
/// hours of sound held in memory as numbers.
const LONGEST_SONG_SECONDS: i64 = 15 * 60;

/// What came of lining a song up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    pub conclusion: Conclusion,
    pub shift_ms: i64,
    pub lines: u32,
    pub lines_moved: u32,
    pub confidence: f32,
}

/// Whether the lyrics were moved, and if not why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conclusion {
    /// The lines were moved to fall where the voice starts.
    Aligned,
    /// They already fall there.
    AlreadyFits,
    /// The sound does not make plain where the voice starts.
    NotSure,
    /// Too few lines to say.
    TooFewLines,
    /// The lyrics are not stamped: there is nothing to line up.
    NotStamped,
    /// The song is too long to be read through.
    TooLong,
}

impl Conclusion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::AlreadyFits => "already_fits",
            Self::NotSure => "not_sure",
            Self::TooFewLines => "too_few_lines",
            Self::NotStamped => "not_stamped",
            Self::TooLong => "too_long",
        }
    }
}

fn may_manage(who: &User) -> Result<()> {
    match who.permissions.is_administrator {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::forbidden(
            "only an administrator lines the lyrics up with the sound",
        ))),
    }
}

/// Reads the song through and lines the stamps of its lyrics up with it.
pub async fn synchronise(state: &AppState, who: &User, song: WorkId) -> Result<Outcome> {
    may_manage(who)?;
    may_read_the_work(state, who, song).await?;
    let database = state.database();
    let nothing = |conclusion, lines| Outcome { conclusion, shift_ms: 0, lines, lines_moved: 0, confidence: 0.0 };

    let Some(found) = found_lyrics(state, who, song).await? else {
        return Ok(nothing(Conclusion::NotStamped, 0));
    };
    let stamps: Vec<Stamp> = found
        .lyrics
        .synced
        .iter()
        .map(|line| Stamp { at_ms: line.at.get(), sung: !line.text.trim().is_empty() })
        .collect();
    let lines = stamps.len() as u32;
    if stamps.is_empty() {
        return Ok(nothing(Conclusion::NotStamped, 0));
    }
    if let Some(asked) = database.song_to_look_up(song).await?
        && asked.duration_ms.is_some_and(|ms| ms / 1000 > LONGEST_SONG_SECONDS)
    {
        return Ok(nothing(Conclusion::TooLong, lines));
    }
    let Some(file) = database.music_song_file(song).await? else {
        return Err(AppError::Domain(melyxar_core::Error::not_found("the song has no file on a disk")));
    };
    let Some(tools) = state.tools() else {
        return Err(AppError::Domain(melyxar_core::Error::dependency_missing("the media tools are missing")));
    };

    let started = std::time::Instant::now();
    tracing::debug!(%song, file = %file.path.display(), lines, "lyrics: the song is read through to line its lyrics up");
    let samples = song_samples(
        tools,
        &file.path,
        Millis::new((LONGEST_SONG_SECONDS + 1) * 1000),
        AskedToStop::never(),
    )
    .await?;
    let read_in = started.elapsed();
    let seconds = samples.len() as f64 / f64::from(melyxar_sound::VOICE_SAMPLES_A_SECOND);
    let lined_up = {
        let stamps = stamps.clone();
        tokio::task::spawn_blocking(move || align(&voice_onsets(&samples), &stamps))
            .await
            .map_err(|error| AppError::Domain(melyxar_core::Error::internal(error.to_string())))?
    };
    tracing::debug!(
        %song,
        sound_seconds = seconds,
        read_ms = read_in.as_millis() as u64,
        took_ms = started.elapsed().as_millis() as u64,
        verdict = lined_up.verdict.as_word(),
        shift_ms = lined_up.shift_ms,
        confidence = lined_up.confidence,
        clear_of_chance = lined_up.clear_of_chance,
        lines_moved = lined_up.lines_moved,
        "lyrics: the lines were lined up with the sound"
    );
    if cfg!(debug_assertions) {
        for (line, moved) in found.lyrics.synced.iter().zip(&lined_up.moves_ms).take(60) {
            tracing::debug!(%song, at_ms = line.at.get(), moved_ms = moved, text = %line.text.chars().take(30).collect::<String>(), "lyrics: a line");
        }
    }

    let outcome = |conclusion| Outcome {
        conclusion,
        shift_ms: lined_up.shift_ms,
        lines,
        lines_moved: lined_up.lines_moved as u32,
        confidence: lined_up.confidence,
    };
    Ok(match lined_up.verdict {
        Verdict::Aligned => {
            database
                .keep_lyrics_timing(
                    song,
                    &LyricsTiming {
                        shift_ms: lined_up.shift_ms,
                        moves_ms: lined_up.moves_ms.clone(),
                        lines_moved: lined_up.lines_moved as u32,
                        confidence: lined_up.confidence,
                    },
                    melyxar_core::time::now(),
                )
                .await?;
            outcome(Conclusion::Aligned)
        }
        Verdict::AlreadyFits => {
            database.forget_lyrics_timing(song).await?;
            outcome(Conclusion::AlreadyFits)
        }
        Verdict::NotSure => outcome(Conclusion::NotSure),
        Verdict::TooFewLines => outcome(Conclusion::TooFewLines),
    })
}

/// Puts the lines of a song back as they are stamped.
pub async fn forget(state: &AppState, who: &User, song: WorkId) -> Result<()> {
    may_manage(who)?;
    may_read_the_work(state, who, song).await?;
    tracing::debug!(%song, "lyrics: the lines are put back as they are stamped");
    state.database().forget_lyrics_timing(song).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_the_song_is_read_at_is_the_rate_the_voice_is_read_in() {
        assert_eq!(melyxar_ffmpeg::sound::SAMPLES_A_SECOND, melyxar_sound::VOICE_SAMPLES_A_SECOND);
    }

    #[tokio::test]
    async fn only_an_administrator_lines_the_lyrics_up() {
        let (_directory, state) = crate::an_empty_server().await;
        let viewer = crate::a_viewer(&state).await;
        let song = WorkId::new();
        let refused = |outcome: Result<()>| {
            matches!(outcome, Err(AppError::Domain(error)) if error.to_string().contains("only an administrator"))
        };
        assert!(refused(synchronise(&state, &viewer, song).await.map(|_| ())));
        assert!(refused(forget(&state, &viewer, song).await));
    }

    #[test]
    fn every_way_it_can_end_has_a_word_of_its_own() {
        let words: std::collections::BTreeSet<&str> = [
            Conclusion::Aligned,
            Conclusion::AlreadyFits,
            Conclusion::NotSure,
            Conclusion::TooFewLines,
            Conclusion::NotStamped,
            Conclusion::TooLong,
        ]
        .iter()
        .map(|verdict| verdict.as_str())
        .collect();
        assert_eq!(words.len(), 6);
    }
}
