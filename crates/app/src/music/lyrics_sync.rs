//! Lining the lyrics of a song up with its sound, for an administrator.
//!
//! The song is listened to by the speech tool, which writes down the words it
//! hears and when (see `melyxar_ffmpeg::speech`); the words of the lyrics are
//! then laid beside them, and each line is moved to where its words are heard
//! (see `melyxar_sound`). Listening to a song takes minutes, so this is a job
//! of its own, started for one song and followed on the page.
//!
//! What comes of it is kept as how far each line was moved, not as other
//! lyrics: the words and stamps as written stay as they were, and the moves
//! are put back on them each time they are read, as long as they still have
//! the same lines. What came of the attempt is kept too when nothing was
//! moved, so the window can say why.
//!
//! The tool is never asked what is sung, since the words are known: what it
//! hears is used only where it agrees with them. A song it cannot make out,
//! because the music covers the voice, is left as it is and told so.

use melyxar_core::id::WorkId;
use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::user::User;
use melyxar_database::music_lyrics_timing::LyricsTiming;
use melyxar_ffmpeg::speech::{Listener, listen_for_words};
use melyxar_ffmpeg::{AskedToStop, FfmpegError};
use melyxar_jobs::{JobHandle, StartedJob};
use melyxar_sound::{HeardWord, LyricLine, Verdict, align};

use crate::music::lyrics::found_lyrics;
use crate::reach::may_read_the_work;
use crate::speech::{Ready, effort, ready};
use crate::{AppError, AppState, Result};

/// The longest song that is listened to: a song, not an audiobook, which
/// would be hours of the processor.
const LONGEST_SONG_SECONDS: i64 = 15 * 60;

/// How many words heard are written to the journal when it is read closely.
const HEARD_SHOWN: usize = 60;

/// Whether the lyrics were moved, and if not why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conclusion {
    /// The lines were moved to fall where their words are heard.
    Aligned,
    /// They already fall there.
    AlreadyFits,
    /// Too few of the words were heard to say where the lines are.
    NotSure,
    /// Too few lines to say.
    TooFewLines,
    /// The song is too long to be listened to.
    TooLong,
}

impl Conclusion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::AlreadyFits => "already_fits",
            Self::NotSure => "not_sure",
            Self::TooFewLines => "too_few_lines",
            Self::TooLong => "too_long",
        }
    }
}

impl From<Verdict> for Conclusion {
    fn from(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Aligned => Self::Aligned,
            Verdict::AlreadyFits => Self::AlreadyFits,
            Verdict::NotSure => Self::NotSure,
            Verdict::TooFewLines => Self::TooFewLines,
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

/// Starts listening to a song to line its lyrics up, as a job the page can
/// follow and stop. Refused when there is nothing to line up or nothing to
/// listen with.
pub async fn start(state: &AppState, who: &User, song: WorkId) -> Result<StartedJob> {
    may_manage(who)?;
    let found = found_lyrics(state, who, song)
        .await?
        .filter(|found| !found.lyrics.synced.is_empty())
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("these lyrics are not stamped line by line")))?;
    let Some(listening) = ready(state).await? else {
        return Err(AppError::Domain(melyxar_core::Error::dependency_missing(
            "no speech tool or no listening model is set up",
        )));
    };
    let lines = lines_of(&found.lyrics.synced);

    let owned = state.clone();
    let started = state
        .jobs()
        .clone()
        .start(
            JobKind::LineUpLyrics,
            JobPriority::REQUESTED,
            Some(song.to_string()),
            move |handle| async move {
                line_up(&owned, song, listening, lines, &handle)
                    .await
                    .map_err(|error| error.to_string())
            },
        )
        .await?;
    Ok(started)
}

fn lines_of(synced: &[melyxar_core::music_lyrics::SyncedLine]) -> Vec<LyricLine> {
    synced.iter().map(|line| LyricLine { at_ms: line.at.get(), text: line.text.clone() }).collect()
}

/// Listens to the song, lays the words heard beside the lines and keeps what
/// comes of it.
async fn line_up(
    state: &AppState,
    song: WorkId,
    listening: Ready,
    lines: Vec<LyricLine>,
    handle: &JobHandle,
) -> Result<()> {
    let database = state.database();
    let kept = |conclusion: Conclusion, alignment: Option<&melyxar_sound::Alignment>| LyricsTiming {
        conclusion: conclusion.as_str().to_string(),
        shift_ms: alignment.map_or(0, |alignment| alignment.shift_ms),
        moves_ms: alignment
            .filter(|_| conclusion == Conclusion::Aligned)
            .map_or_else(Vec::new, |alignment| alignment.moves_ms.clone()),
        lines_moved: alignment.map_or(0, |alignment| alignment.lines_moved as u32),
        confidence: alignment.map_or(0.0, |alignment| alignment.confidence),
    };

    let Some(wanted) = database.song_to_look_up(song).await? else {
        return Err(AppError::Domain(melyxar_core::Error::not_found("the song no longer exists")));
    };
    if wanted.duration_ms.is_some_and(|ms| ms / 1000 > LONGEST_SONG_SECONDS) {
        database
            .keep_lyrics_timing(song, &kept(Conclusion::TooLong, None), melyxar_core::time::now())
            .await?;
        return Ok(());
    }
    let Some(file) = database.music_song_file(song).await? else {
        return Err(AppError::Domain(melyxar_core::Error::not_found("the song has no file on a disk")));
    };
    let Some(tools) = state.tools() else {
        return Err(AppError::Domain(melyxar_core::Error::dependency_missing("the media tools are missing")));
    };

    handle.at_step(JobStep::ListeningToSong).await;
    let name = file.path.file_name().map(|name| name.to_string_lossy().into_owned());
    handle.now_working_on(name.as_deref()).await;
    let directories = &state.config().directories;
    tokio::fs::create_dir_all(directories.speech_scratch()).await?;
    let threads = effort(state).await?.threads(std::thread::available_parallelism().map_or(2, usize::from));
    tracing::debug!(%song, file = %file.path.display(), lines = lines.len(), "lyrics: the song is listened to for the words in it");

    let heard = match listen_for_words(
        tools,
        Listener { tool: &listening.tool, model: &listening.model, threads },
        &file.path,
        &directories.speech_scratch(),
        AskedToStop::when(handle.cancelled_when()),
        |fraction| handle.element_at(fraction),
    )
    .await
    {
        Ok(heard) => heard,
        // Called off: nothing is kept, and it is not a fault.
        Err(FfmpegError::GivenUp) => return Ok(()),
        Err(error) => return Err(error.into()),
    };

    let words: Vec<HeardWord> =
        heard.iter().map(|word| HeardWord { text: word.text.clone(), at_ms: word.from_ms }).collect();
    let sent = lines.clone();
    let alignment = tokio::task::spawn_blocking(move || align(&sent, &words))
        .await
        .map_err(|error| AppError::Domain(melyxar_core::Error::internal(error.to_string())))?;
    tracing::info!(
        %song,
        heard_words = heard.len(),
        lines = lines.len(),
        lines_heard = alignment.lines_heard,
        lines_moved = alignment.lines_moved,
        shift_ms = alignment.shift_ms,
        verdict = alignment.verdict.as_word(),
        "lyrics: the song was listened to and its lines laid beside the words heard"
    );
    if cfg!(debug_assertions) {
        let said: Vec<String> =
            heard.iter().take(HEARD_SHOWN).map(|word| format!("{}@{}", word.text, word.from_ms)).collect();
        tracing::debug!(%song, heard = %said.join(" "), "lyrics: the first words heard");
        for (line, moved) in lines.iter().zip(&alignment.moves_ms) {
            tracing::debug!(
                %song,
                at_ms = line.at_ms,
                moved_ms = moved,
                text = %line.text.chars().take(30).collect::<String>(),
                "lyrics: a line"
            );
        }
    }

    database
        .keep_lyrics_timing(song, &kept(alignment.verdict.into(), Some(&alignment)), melyxar_core::time::now())
        .await?;
    Ok(())
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

    #[tokio::test]
    async fn only_an_administrator_lines_the_lyrics_up() {
        let (_directory, state) = crate::an_empty_server().await;
        let viewer = crate::a_viewer(&state).await;
        let song = WorkId::new();
        let refused = |outcome: Result<()>| {
            matches!(outcome, Err(AppError::Domain(error)) if error.to_string().contains("only an administrator"))
        };
        assert!(refused(start(&state, &viewer, song).await.map(|_| ())));
        assert!(refused(forget(&state, &viewer, song).await));
    }

    #[test]
    fn every_way_it_can_end_has_a_word_of_its_own_and_a_verdict_to_come_from() {
        let words: std::collections::BTreeSet<&str> = [
            Conclusion::Aligned,
            Conclusion::AlreadyFits,
            Conclusion::NotSure,
            Conclusion::TooFewLines,
            Conclusion::TooLong,
        ]
        .iter()
        .map(|conclusion| conclusion.as_str())
        .collect();
        assert_eq!(words.len(), 5);
        assert_eq!(Conclusion::from(Verdict::Aligned), Conclusion::Aligned);
        assert_eq!(Conclusion::from(Verdict::NotSure), Conclusion::NotSure);
    }
}
