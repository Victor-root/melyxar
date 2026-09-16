//! Answering "how does this film reach me, and where was I?".
//!
//! The decision itself is a pure rule living in its own crate, which knows
//! nothing of files or databases. This is where it meets them: the file is
//! read back with its tracks, the rule is asked, and the answer is handed over
//! with what a player needs to start.
//!
//! Nothing here opens the file. Serving bytes belongs to the HTTP layer, which
//! is the only part that knows what a range request is.

use std::path::PathBuf;
use std::sync::Arc;

use melyxar_core::id::{MediaSourceId, TrackId, UserId, WorkId};
use melyxar_core::media::Track;
use melyxar_core::privacy::MediaName;
use melyxar_core::thumbnails::Thumbnails;
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::user::DownmixMethod;
use melyxar_core::work::{state_for_position, PlaybackState, DEFAULT_WATCHED_THRESHOLD};
use melyxar_playback::decision::{decide, PlaybackRequest};

use crate::{AppError, AppState, Result};

/// What a client says it can open, and the shape of the answer.
///
/// Re-exported here so the layer above talks to one crate: the decision lives
/// where it can be tested without a database, and nothing outside has to know
/// that it does.
pub use melyxar_playback::decision::{
    PlaybackDecision, PlaybackMethod, Reason, StreamAction, SubtitleDelivery,
};
pub use melyxar_playback::profile::{ClientProfile, RebuiltCapability};

/// A film being converted as it is watched, as the layer above handles it.
///
/// Re-exported for the same reason as the decision: the HTTP layer talks to
/// one crate, and where a session really lives stays this crate's business.
pub use melyxar_streaming::session::{
    Preparation, PreparationStep, Producing, Recipe, Session, SessionId,
};
pub use melyxar_streaming::StreamingError;

/// What a viewer asked to play, in the words of a client.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayRequest {
    pub source_id: MediaSourceId,
    /// What the client says it can open. Absent means a cautious browser,
    /// which is what a client that says nothing turns out to be.
    pub profile: Option<ClientProfile>,
    /// Chosen soundtrack. Absent means the one the file marks as default.
    pub audio_track_id: Option<TrackId>,
    /// Chosen subtitle. Absent means none.
    pub subtitle_track_id: Option<TrackId>,
}

/// Everything a player needs to start.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayPlan {
    pub source_id: MediaSourceId,
    pub work_id: WorkId,
    /// Where the file is. Kept for the layer that serves the bytes and never
    /// sent to a client.
    pub path: PathBuf,
    pub size_bytes: i64,
    /// What the file is wrapped in, as the analyser named it.
    pub container: Option<String>,
    /// Everything in the file per second, streams and container alike.
    pub overall_bitrate: Option<i64>,
    pub duration: Option<Millis>,
    pub decision: PlaybackDecision,
    /// Where this viewer stopped last time, when they did.
    pub resume_from: Option<Millis>,
    pub tracks: Vec<Track>,
    /// How this viewer wants a multichannel soundtrack folded to stereo, and
    /// how much to lift it afterwards. Carried on the plan because the fold is
    /// only actually performed later, and a preference that changes the answer
    /// without changing what is produced is a setting that does nothing.
    pub downmix: DownmixMethod,
    pub downmix_gain: f64,
    /// How the picture will actually be rebuilt, when it is.
    ///
    /// Worked out here rather than where the tool is set going, because a page
    /// has to be able to say it: "the card is doing this, in this codec, at
    /// this size" is the answer to the only question anybody asks about a film
    /// that stutters.
    pub rebuild: Option<PictureRebuild>,
    /// The little pictures of the playback bar, when this film has been read
    /// for them. Absent is an ordinary answer: a film scanned before the pass
    /// ran shows a bare bar, which is what every film did before it existed.
    pub thumbnails: Option<Thumbnails>,
}

/// How the picture is rebuilt, when it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureRebuild {
    /// Codec it comes out in.
    pub codec: String,
    /// The card doing the work. Absent means the processor is.
    pub card: Option<melyxar_ffmpeg::Card>,
    /// Whether that card reads the film for itself as well.
    ///
    /// A film in a codec the card was never proved to read is decoded by the
    /// processor and handed up. That still moves the expensive half of the
    /// work, and it is what works whatever the film holds.
    pub reads_the_film: bool,
    /// Height it comes out at. Absent means the size is left alone.
    pub height: Option<i32>,
    /// Rate it is held to, in bits per second.
    pub bitrate: Option<i64>,
}

impl PictureRebuild {
    pub fn on_a_card(&self) -> bool {
        self.card.is_some()
    }

    /// What the card is called, for a log line and for a page.
    pub fn card_name(&self) -> Option<String> {
        self.card
            .as_ref()
            .map(|card| card.device.display().to_string())
    }
}

/// Works out how one file reaches one client.
pub async fn plan(state: &AppState, user_id: UserId, request: &PlayRequest) -> Result<PlayPlan> {
    let database = state.database();

    let source = database
        .playable_source(request.source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("media source")))?;

    if source.missing {
        // Refused with an explanation rather than opened and failing halfway
        // through, which is what a viewer would otherwise see.
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::RootUnavailable,
            "the file is not on the disk at the moment",
        )));
    }

    // Nothing ever managed to describe this file: no container, no streams.
    // There is nothing to decide with, and a conversion started anyway would
    // be refused by the tool after a wait. Saying so here puts the answer on
    // the screen that asked for it, and the report names the file and why.
    let tracks = database.tracks_of_source(request.source_id).await?;
    if source.container.is_none() && tracks.is_empty() {
        return Err(AppError::Domain(melyxar_core::Error::not_described(
            "nothing has managed to read this file, so there is nothing to play it with",
        )));
    }

    let media = melyxar_core::media::MediaSource {
        id: source.id,
        work_id: source.work_id,
        root_id: melyxar_core::id::LibraryRootId::new(),
        relative_path: source.path.clone(),
        container: source.container.clone(),
        duration: source.duration,
        overall_bitrate: source.overall_bitrate,
        identity: melyxar_core::media::FileIdentity {
            size_bytes: source.size_bytes,
            modified_at: melyxar_core::time::now(),
            content_fingerprint: None,
        },
        added_at: melyxar_core::time::now(),
        tracks: tracks.clone(),
    };

    let profile = request
        .profile
        .clone()
        .unwrap_or_else(ClientProfile::conservative_browser);

    let preferences = database.user(user_id).await?.map(|user| user.preferences);
    let remembered = database.playback_progress(user_id, source.work_id).await?;

    // What the viewer asked for now, otherwise what they chose last time for
    // this very film, otherwise a track in the language they prefer. Only then
    // does the file get to decide, which is what happens today for everyone.
    let audio = chosen_track(
        &tracks,
        request.audio_track_id,
        remembered.as_ref().and_then(|stored| stored.audio_track_id),
        preferences
            .as_ref()
            .and_then(|values| values.preferred_audio_language.as_deref()),
        TrackShape::Audio,
    );
    let subtitle = match request.subtitle_track_id {
        // A subtitle is only shown when someone asks for one: a film that
        // starts with subtitles nobody wanted is a film someone stops.
        Some(id) => tracks.iter().find(|track| track.id == id),
        None => chosen_track(
            &tracks,
            None,
            remembered
                .as_ref()
                .and_then(|stored| stored.subtitle_track_id),
            preferences
                .as_ref()
                .and_then(|values| values.preferred_subtitle_language.as_deref()),
            TrackShape::Subtitle,
        ),
    };
    let downmix = preferences
        .as_ref()
        .map(|values| values.downmix_method)
        .unwrap_or_default();
    let downmix_gain = preferences
        .as_ref()
        .map(|values| values.downmix_gain)
        .unwrap_or(melyxar_core::user::DEFAULT_DOWNMIX_GAIN);

    let decision = decide(
        &media,
        &PlaybackRequest {
            profile: &profile,
            audio_track: audio,
            subtitle_track: subtitle,
            downmix,
            // Nothing asks for levelling yet: no preference carries it, and
            // asking for it without a measurement to level by would turn a
            // copy into a rebuild for nothing. When the preference arrives it
            // goes here, and the answer will say it was asked for.
            level_loudness: false,
        },
    );

    let rebuild = how_to_rebuild(
        &decision,
        &tracks,
        &profile,
        state.capabilities(),
        subtitle_to_paint_on(&decision, &tracks).is_some(),
    );

    // The one line that explains a playback afterwards. The reasons are worked
    // out here, travel to the page, and used to go nowhere else: a viewer who
    // says "it would not play" left nothing behind to look at. What is
    // rebuilding the picture belongs on the same line for the same reason: it
    // is the first question anybody asks about a film that stutters, and the
    // answer used to be somewhere between a process listing and a guess.
    let shape = picture_shape(&tracks);
    tracing::info!(
        file = %MediaName::new(
            source
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        ),
        method = decision.method.as_str(),
        video = ?decision.video,
        audio = ?decision.audio,
        subtitles = ?decision.subtitles,
        tone_map = decision.tone_map,
        rebuilt_by = rebuild.as_ref().map(|rebuild| match rebuild.on_a_card() {
            true => "card",
            false => "processor",
        }),
        rebuilt_on = rebuild.as_ref().and_then(PictureRebuild::card_name),
        rebuilt_into = rebuild.as_ref().map(|rebuild| rebuild.codec.as_str()),
        read_by = rebuild.as_ref().map(|rebuild| match rebuild.reads_the_film {
            true => "card",
            false => "processor",
        }),
        rebuilt_height = rebuild.as_ref().and_then(|rebuild| rebuild.height),
        rebuilt_bitrate = rebuild.as_ref().and_then(|rebuild| rebuild.bitrate),
        picture_across = shape.as_ref().map(|(across, _, _)| *across),
        picture_down = shape.as_ref().map(|(_, down, _)| *down),
        picture_shown_as = shape.as_ref().and_then(|(_, _, shown_as)| shown_as.clone()),
        reasons = ?decision.reasons,
        "playback decided"
    );

    let resume_from = remembered
        .filter(|progress| progress.state == PlaybackState::InProgress)
        .map(|progress| progress.position);

    // Nothing here waits on them or makes them: a film that has none is a film
    // whose bar shows no pictures, and that is all.
    let thumbnails = database.thumbnails_of(source.id).await.unwrap_or_default();

    Ok(PlayPlan {
        source_id: source.id,
        work_id: source.work_id,
        path: source.path,
        size_bytes: source.size_bytes,
        container: source.container.clone(),
        overall_bitrate: source.overall_bitrate,
        duration: source.duration,
        decision,
        resume_from,
        tracks,
        downmix,
        downmix_gain,
        rebuild,
        thumbnails,
    })
}

/// Which of the two kinds of track is being looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackShape {
    Audio,
    Subtitle,
}

impl TrackShape {
    fn matches(self, track: &Track) -> bool {
        match self {
            Self::Audio => matches!(track.kind, melyxar_core::media::TrackKind::Audio(_)),
            Self::Subtitle => matches!(track.kind, melyxar_core::media::TrackKind::Subtitle(_)),
        }
    }
}

/// Picks a track, in the order a viewer would expect.
///
/// What they asked for now beats what they chose last time, which beats the
/// language they prefer in general. A choice pointing at a track the file no
/// longer holds falls through to the next rule rather than to nothing: an
/// analysis run again renumbers the tracks, and a viewer should not have to
/// notice.
fn chosen_track<'a>(
    tracks: &'a [Track],
    asked_for: Option<TrackId>,
    remembered: Option<TrackId>,
    preferred_language: Option<&str>,
    shape: TrackShape,
) -> Option<&'a Track> {
    let by_id = |wanted: TrackId| tracks.iter().find(|track| track.id == wanted);

    if let Some(track) = asked_for.and_then(by_id) {
        return Some(track);
    }
    if let Some(track) = remembered.and_then(by_id) {
        return Some(track);
    }

    let wanted = preferred_language.map(melyxar_core::media::normalise_language)?;
    tracks.iter().find(|track| {
        shape.matches(track)
            && track
                .language
                .as_deref()
                .map(melyxar_core::media::normalise_language)
                .is_some_and(|spoken| spoken == wanted)
    })
}

/// Remembers the tracks a viewer chose, and the languages they were in.
///
/// Both, and for different reasons. The tracks so this film starts the same
/// way next time, down to the track; the languages so the next film does too,
/// even though its tracks are numbered differently.
pub async fn remember_chosen_tracks(
    state: &AppState,
    user_id: UserId,
    work_id: WorkId,
    audio: Option<&Track>,
    subtitle: Option<&Track>,
) -> Result<()> {
    let database = state.database();
    database
        .record_chosen_tracks(
            user_id,
            work_id,
            audio.map(|track| track.id),
            subtitle.map(|track| track.id),
        )
        .await?;

    if let Some(user) = database.user(user_id).await? {
        let mut preferences = user.preferences;
        // A track with no language declared teaches nothing about what this
        // viewer prefers, so it leaves the preference alone.
        if let Some(language) = audio.and_then(|track| track.language.clone()) {
            preferences.preferred_audio_language = Some(language);
        }
        // Subtitles are different: turning them off is itself a preference,
        // and one a viewer expects to hold for the next film too.
        preferences.preferred_subtitle_language = subtitle.and_then(|track| track.language.clone());
        database.save_preferences(user_id, &preferences).await?;
    }
    Ok(())
}

/// Opens a session that produces this film in a form the client can play.
///
/// Everything about what to produce was already decided; this turns that
/// decision into the recipe a session carries out. A film the client could
/// play as it is never reaches here: it is served as a file, which costs
/// nothing at all.
pub async fn open_session(
    state: &AppState,
    plan: &PlayPlan,
    starting_at: Option<Millis>,
) -> Result<Arc<Session>> {
    let no_tools = || {
        AppError::Domain(melyxar_core::Error::dependency_missing(
            "this server has no media tools, so nothing can be converted",
        ))
    };
    let sessions = state.sessions().ok_or_else(no_tools)?;
    let capabilities = state.capabilities().ok_or_else(no_tools)?;

    let mut recipe = recipe_for(plan, capabilities)?;

    // What the client says, and what the server remembers of this viewer when
    // it says nothing. Either way the first segment produced is the one about
    // to be watched, rather than the beginning of a film nobody is at.
    recipe.where_the_viewer_starts = starting_at.or(plan.resume_from).unwrap_or(Millis::ZERO);

    // Both sides named, because they disagreeing is the whole failure mode:
    // the server producing one part of the film while the player asks for
    // another costs a restart at best, and the two are set from one number on
    // the page. A journal that shows only the answer cannot say which of them
    // was wrong.
    tracing::debug!(
        said_by_the_player = starting_at.map(Millis::as_seconds_f64),
        remembered = plan.resume_from.map(Millis::as_seconds_f64),
        beginning_at_second = recipe.where_the_viewer_starts.as_seconds_f64(),
        "a session was told where it begins"
    );

    // Only for a picture carried over untouched. One the server rebuilds gets
    // a key frame on every boundary, put there by the server itself, so the
    // grid is already true of it and reading the film for these would answer a
    // question nobody asked.
    if recipe.video == melyxar_ffmpeg::command::VideoOutput::Copy {
        recipe.where_it_can_be_started = where_this_film_can_be_started(state, plan).await;
    }

    let expensive = plan.decision.method.is_expensive();
    let session = sessions.open(recipe, expensive).await?;
    say_how_the_film_was_cut(&session);

    // Started now rather than when somebody asks for a subtitle. Pulling one
    // out of a film means reading the whole file through, because the words
    // are interleaved with the picture from end to end: measured at three
    // quarters of a minute on a 4K film. Asked for at the moment of the click,
    // that wait falls entirely on somebody who has already pressed the button
    // and sees nothing happen. Started here, it runs while the film plays.
    prepare_the_subtitles(state, plan);

    Ok(session)
}

/// Converts every subtitle made of words this film carries, in the background.
///
/// Nothing is waited on and nothing fails outwardly: this is a head start, not
/// a step. A track that will not convert says so in the journal under its own
/// tag, and a viewer who asks for it anyway gets the same answer they would
/// have got without this.
fn prepare_the_subtitles(state: &AppState, plan: &PlayPlan) {
    let state = state.clone();
    let source_id = plan.source_id;
    tokio::spawn(async move {
        let _ = crate::subtitles::pull_them_all_out(&state, source_id).await;
    });
}

/// Writes down how a film came out cut, before a single segment exists.
///
/// The playlist is the one thing a reading cannot recover from getting wrong:
/// it says which part of the film every segment holds, and a player believes
/// it. When the picture drifts behind the bar, or a film stops with minutes
/// still on the clock, this line is where the answer is, and without it the
/// only way to see any of this was to have the file in hand.
fn say_how_the_film_was_cut(session: &melyxar_streaming::session::Session) {
    let playlist = session.playlist();
    let segments = playlist.segment_count();
    let lengths = (0..segments).filter_map(|index| playlist.duration_of(index));
    let shortest = lengths.clone().map(Millis::get).min().unwrap_or_default();

    tracing::debug!(
        session = %session.id,
        film_lasts = playlist.total.as_seconds_f64(),
        segments,
        cut_where_the_film_allows = playlist.cut_where_the_film_allows(),
        last_segment_begins_at = playlist.start_of(segments.saturating_sub(1)).as_seconds_f64(),
        longest_segment = lengths.map(Millis::get).max().unwrap_or_default() as f64 / 1000.0,
        shortest_segment = shortest as f64 / 1000.0,
        "how this film was cut"
    );
}

/// Where this film can really be started, when it has been read for it.
///
/// Nothing when it has not, and nothing when reading it back went wrong: the
/// usual grid is what everything used before, it plays, and a film that cannot
/// be started is a far worse answer than a jump that lands early.
async fn where_this_film_can_be_started(state: &AppState, plan: &PlayPlan) -> Vec<Millis> {
    match state.database().key_frames_of(plan.source_id).await {
        Ok(Some(found)) if !found.is_empty() => {
            melyxar_ffmpeg::probe::where_the_film_can_be_cut(&found)
        }
        Ok(_) => Vec::new(),
        Err(error) => {
            tracing::warn!(
                source = %plan.source_id,
                error = %error,
                "where this film can be started could not be read, so it is cut on the usual grid"
            );
            Vec::new()
        }
    }
}

/// The subtitle to paint onto every frame, when there is one.
///
/// Only a subtitle made of pictures. The decision also asks for painting when
/// a client says it draws no subtitle at all, and that case is not served here:
/// the tool paints pictures onto pictures, so a subtitle of words would cost a
/// full rebuild and put nothing on the screen. The words are sent alongside
/// instead, which the one client there is draws anyway.
fn subtitle_to_paint_on(decision: &PlaybackDecision, tracks: &[Track]) -> Option<i32> {
    use melyxar_core::media::{SubtitleLayout, TrackKind};

    if decision.subtitles != SubtitleDelivery::BurnIn {
        return None;
    }
    let index = decision.subtitle_stream_index?;
    tracks
        .iter()
        .find(|track| match &track.kind {
            TrackKind::Subtitle(details) => {
                track.stream_index == index && details.layout == SubtitleLayout::Bitmap
            }
            _ => false,
        })
        .map(|track| track.stream_index)
}

/// The tallest picture a processor rebuilds while somebody watches it.
///
/// Measured on a four thread machine with a real wide gamut film: rebuilt at
/// its own size it ran at a third of real time, which is a slideshow with
/// sound. At this height the same film ran faster than real time, and a smooth
/// picture beats a larger one nobody can watch.
///
/// Only ever true of a rebuild done in software. A card does this without
/// noticing, and the ceiling is not applied to one.
const TALLEST_SOFTWARE_REBUILD: i32 = 1080;

/// The codecs a rebuilt picture is offered in, best first.
///
/// Best means the most picture for a given rate. A card produces all three at
/// much the same speed, so the newer ones cost nothing here and are worth a
/// great deal to a viewer on a thin connection. The last one is the one no
/// client has ever refused.
///
/// Which of them a viewer actually gets is settled by the height the client
/// said it decodes each of them at, never by a rule written here: the same
/// browser answers differently on two machines, and a film rebuilt above what
/// it answered is a picture that never appears.
const BEST_FIRST: &[&str] = &["av1", "hevc", melyxar_playback::profile::ALWAYS_READ];

/// How tall the picture of a film is, when it holds one.
fn height_of(tracks: &[Track]) -> Option<i32> {
    tracks.iter().find_map(|track| match &track.kind {
        melyxar_core::media::TrackKind::Video(details) => Some(details.visible_height()),
        _ => None,
    })
}

/// The shape of the picture as the film holds it: how many pixels across and
/// down, and what shape those pixels make once shown.
///
/// The two are not the same thing and the difference is the whole point. A
/// film can hold pixels that are not square, so a picture nineteen hundred and
/// twenty across is shown far wider than that; a film that loses that on the
/// way out is shown stretched, and nothing anywhere said what shape it started
/// as.
fn picture_shape(tracks: &[Track]) -> Option<(i32, i32, Option<String>)> {
    tracks.iter().find_map(|track| match &track.kind {
        melyxar_core::media::TrackKind::Video(details) => Some((
            details.visible_width(),
            details.visible_height(),
            details.aspect_ratio.clone(),
        )),
        _ => None,
    })
}

/// How tall a picture rebuilt by the processor should be.
///
/// What the client asked for, and never more than the ceiling above. Absent
/// when the picture is no taller than that already, since resizing a picture
/// to its own size is work for nothing.
fn height_to_rebuild_at(source_height: Option<i32>, asked_for: Option<i32>) -> Option<i32> {
    let source_height = source_height?;
    let ceiling = asked_for.unwrap_or(i32::MAX).min(TALLEST_SOFTWARE_REBUILD);
    (ceiling < source_height).then_some(ceiling)
}

/// What a rebuilt picture is given to work with when nobody asked for a rate.
///
/// A card has to be given one: it counts quality on a scale of its own for
/// each codec, where the same number means three different things, and a rate
/// means the same thing to all three. These are the usual streaming rates for
/// the codec every client reads, and the newer codecs are given less because
/// needing less is the whole point of them.
fn rate_for(height: Option<i32>, codec: &str) -> i64 {
    let as_h264 = match height.unwrap_or(1080) {
        height if height > 1440 => 25_000_000,
        height if height > 1080 => 16_000_000,
        height if height > 720 => 10_000_000,
        height if height > 480 => 5_000_000,
        _ => 2_500_000,
    };
    match codec {
        "av1" => as_h264 * 55 / 100,
        "hevc" => as_h264 * 65 / 100,
        _ => as_h264,
    }
}

/// Works out who rebuilds the picture, into what, and at what size.
///
/// Nothing when the picture is not being rebuilt at all, which is most films.
fn how_to_rebuild(
    decision: &PlaybackDecision,
    tracks: &[Track],
    profile: &ClientProfile,
    capabilities: Option<&melyxar_ffmpeg::Capabilities>,
    painting_subtitles: bool,
) -> Option<PictureRebuild> {
    if decision.video != melyxar_playback::decision::StreamAction::Transcode {
        return None;
    }
    let source_height = height_of(tracks);

    let card = capabilities
        .and_then(melyxar_ffmpeg::Capabilities::card)
        // Painting words into a picture is done where the words are, which is
        // on the processor. Getting them onto a card is a different piece of
        // work, and this is not it.
        .filter(|_| !painting_subtitles)
        // A card that cannot convert wide gamut colour would hand back a film
        // that is grey, which is worse than one that is merely smaller.
        .filter(|card| !decision.tone_map || card.can_tone_map);

    // The height the picture will really be, which is what the client answered
    // about. A card rebuilds at the film's own size unless the viewer asked for
    // less, so a codec is offered only where it was measured to keep up.
    let rebuilt_height = decision.scale_to_height.or(source_height);

    let on_a_card = card.and_then(|card| {
        let writes = |codec: &&&str| card.encoder_for(codec).is_some();

        // The best codec the client keeps up with at the size this is coming
        // out at, which is the ordinary answer.
        if let Some(codec) = BEST_FIRST
            .iter()
            .filter(writes)
            .find(|codec| profile.accepts_rebuilt(codec, rebuilt_height))
        {
            return Some((card, (*codec).to_string(), None));
        }

        // Nothing the card writes keeps up at this size. Then the picture is
        // made smaller rather than handed over in a codec that will not show:
        // a client that cannot follow this film in anything cannot follow it,
        // and a smaller picture is a picture. A card that cannot scale has
        // nothing to offer here, and the answer is found below.
        BEST_FIRST
            .iter()
            .filter(|_| card.can_scale)
            .filter(writes)
            .find_map(|codec| {
                profile
                    .tallest_rebuilt(codec)
                    .map(|tallest| (card, (*codec).to_string(), Some(tallest)))
            })
    });

    let Some((card, codec, hold_to)) = on_a_card else {
        return Some(PictureRebuild {
            codec: melyxar_playback::profile::ALWAYS_READ.to_string(),
            card: None,
            reads_the_film: false,
            height: height_to_rebuild_at(source_height, decision.scale_to_height),
            bitrate: decision.bitrate_ceiling,
        });
    };

    // What the viewer asked for, and never more than the client said it keeps
    // up with. The software ceiling above is a limit of the processor, and a
    // card does not have it.
    let asked = decision.scale_to_height.filter(|_| card.can_scale);
    let height = match hold_to {
        Some(tallest) => Some(asked.unwrap_or(tallest).min(tallest)),
        None => asked,
    };
    Some(PictureRebuild {
        bitrate: Some(
            decision
                .bitrate_ceiling
                .unwrap_or_else(|| rate_for(height.or(source_height), &codec)),
        ),
        // A film that says its picture sits inside a larger frame is read by
        // the processor, whatever the card can do with its codec. A card hands
        // its pictures on as whole frames and nothing in the chain that
        // follows knows how to cut the edges off one, so the film would be
        // rebuilt frame and all: a wide picture stretched to fill a shape it
        // never had. The processor cuts them as it reads, which is what every
        // still image pulled out of these files has always shown.
        reads_the_film: !says_it_is_cut(tracks)
            && codec_of(tracks).is_some_and(|codec| card.reads(&codec)),
        codec,
        card: Some(card.clone()),
        height,
    })
}

/// Whether the film says its picture sits inside a larger frame.
fn says_it_is_cut(tracks: &[Track]) -> bool {
    tracks.iter().any(|track| match &track.kind {
        melyxar_core::media::TrackKind::Video(details) => details.margins.is_some(),
        _ => false,
    })
}

/// What the picture of a film is written in, when it holds one.
fn codec_of(tracks: &[Track]) -> Option<String> {
    tracks.iter().find_map(|track| match &track.kind {
        melyxar_core::media::TrackKind::Video(details) => Some(details.codec.to_lowercase()),
        _ => None,
    })
}

/// The same answer with the card left out, which is what a session falls back
/// to if the card will not have the film after all.
fn without_the_card(plan: &PlayPlan) -> PictureRebuild {
    PictureRebuild {
        codec: melyxar_playback::profile::ALWAYS_READ.to_string(),
        card: None,
        reads_the_film: false,
        height: height_to_rebuild_at(height_of(&plan.tracks), plan.decision.scale_to_height),
        bitrate: plan.decision.bitrate_ceiling,
    }
}

/// Turns a decision into what the tool is asked to do.
fn recipe_for(plan: &PlayPlan, capabilities: &melyxar_ffmpeg::Capabilities) -> Result<Recipe> {
    use melyxar_ffmpeg::command::{AudioOutput, StreamSelection, VideoOutput};
    use melyxar_playback::decision::StreamAction;

    let duration = plan.duration.ok_or_else(|| {
        // Without a duration there is no playlist to write: a film nobody has
        // looked inside cannot be cut into segments.
        AppError::Domain(melyxar_core::Error::invalid_input(
            "this file has not been analysed yet",
        ))
    })?;

    let painted_on = subtitle_to_paint_on(&plan.decision, &plan.tracks);

    let video = match plan.decision.video {
        StreamAction::Drop => VideoOutput::None,
        StreamAction::Copy => VideoOutput::Copy,
        StreamAction::Transcode => {
            let rebuild = plan.rebuild.as_ref().ok_or_else(|| {
                AppError::Domain(melyxar_core::Error::invalid_input(
                    "this picture is to be rebuilt and nothing says how",
                ))
            })?;
            VideoOutput::Encode(encode_for(rebuild, plan, painted_on)?)
        }
    };

    // Only when a card is doing the work. A card is proved at start-up on a
    // generated picture, which does not prove every film, and a refusal must
    // not reach a viewer as a black screen.
    //
    // Two rungs where the card was also reading the film, because the rungs
    // are not equal: a card that will not read one film will still rebuild its
    // picture perfectly well, and giving up on it entirely at the first
    // refusal would throw away most of what it was doing.
    let mut if_the_card_refuses = Vec::new();
    if let Some(rebuild) = plan.rebuild.as_ref().filter(|rebuild| rebuild.on_a_card()) {
        if rebuild.reads_the_film {
            let handed_up = PictureRebuild {
                reads_the_film: false,
                ..rebuild.clone()
            };
            if_the_card_refuses.push(VideoOutput::Encode(encode_for(
                &handed_up, plan, painted_on,
            )?));
        }
        if_the_card_refuses.push(VideoOutput::Encode(encode_for(
            &without_the_card(plan),
            plan,
            painted_on,
        )?));
    }

    let audio = match plan.decision.audio {
        StreamAction::Drop => AudioOutput::None,
        StreamAction::Copy => AudioOutput::Copy,
        StreamAction::Transcode => {
            let encoder = capabilities.audio_encoder().ok_or_else(|| {
                AppError::Domain(melyxar_core::Error::dependency_missing(
                    "these media tools cannot build a soundtrack a browser reads",
                ))
            })?;
            let mut encode = melyxar_ffmpeg::command::AudioEncode::browser_stereo(encoder);
            // The fold this viewer asked for, rather than the one the default
            // would apply. Their choice is already what made this a rebuild in
            // the first place, so producing a different fold would be the
            // worst of both: the cost of the work without the point of it.
            encode.downmix = plan.downmix;
            encode.downmix_gain = plan.downmix_gain;
            AudioOutput::Encode(encode)
        }
    };

    Ok(Recipe {
        source: plan.path.clone(),
        duration,
        streams: StreamSelection {
            video_index: plan.decision.video_stream_index,
            audio_index: plan.decision.audio_stream_index,
            subtitle_index: None,
        },
        video,
        audio,
        // Both filled in by whoever opens the session: one is the film's own
        // answer read back, the other is what the viewer asked for, and
        // neither belongs to the decision this turns into a command.
        where_the_viewer_starts: Millis::ZERO,
        where_it_can_be_started: Vec::new(),
        if_the_card_refuses,
    })
}

/// Turns one answer about the picture into what the tool is handed.
fn encode_for(
    rebuild: &PictureRebuild,
    plan: &PlayPlan,
    painted_on: Option<i32>,
) -> Result<melyxar_ffmpeg::command::VideoEncode> {
    let mut encode = match &rebuild.card {
        Some(card) => melyxar_ffmpeg::command::VideoEncode::on_a_card(
            card,
            &rebuild.codec,
            rebuild.reads_the_film,
        )
        .ok_or_else(|| {
            AppError::Domain(melyxar_core::Error::invalid_input(
                "this card was never proved to produce that codec",
            ))
        })?,
        None => melyxar_ffmpeg::command::VideoEncode::software_h264(),
    };
    encode.scale_to_height = rebuild.height;
    encode.max_bitrate = rebuild.bitrate;
    encode.tone_map = plan.decision.tone_map;
    encode.burn_in_subtitle = painted_on;
    // Key frames on the segment boundaries, which is what lets any segment be
    // produced on its own rather than only after the one before it.
    encode.keyframe_interval = Some(melyxar_streaming::playlist::SEGMENT_DURATION);
    Ok(encode)
}

/// How often sessions nobody is watching are looked for.
///
/// Often enough that a closed tab is forgotten within a minute or so of the
/// idle limit, rarely enough that a server doing nothing is doing nothing.
const SWEEP_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// Removes what a previous run left on the disk.
///
/// Only ever called by the server on its way up, never by a command run
/// alongside a live server: the folders it removes belong to sessions nobody
/// will come back for, and telling them apart is knowing that nothing of this
/// run exists yet.
pub async fn tidy_up_after_a_previous_run(state: &AppState) {
    if let Some(sessions) = state.sessions() {
        sessions.sweep_what_a_previous_run_left().await;
    }
}

/// Keeps sweeping away sessions nobody is watching, for as long as it runs.
///
/// A viewer never says goodbye: they close a tab, lose a connection, put a
/// telephone in a pocket. Without this, the media tool started for them would
/// keep producing a film nobody will ever see.
pub fn keep_sessions_swept(state: &AppState) -> tokio::task::JoinHandle<()> {
    sweep_repeatedly(
        state,
        SWEEP_EVERY,
        melyxar_streaming::registry::KEPT_WHILE_IDLE,
    )
}

/// The loop itself, with both delays given rather than assumed.
///
/// Written this way so a test can watch a real session be swept by the real
/// loop in a moment, instead of waiting two minutes or trusting that the loop
/// says what it means.
fn sweep_repeatedly(
    state: &AppState,
    every: std::time::Duration,
    idle_for: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(every);
        loop {
            ticks.tick().await;
            if let Some(sessions) = state.sessions() {
                sessions.sweep(idle_for).await;
            }
        }
    })
}

/// Closes every session, which is what a server does on its way out.
///
/// This is the point of a clean stop: a session owns an external process, and
/// leaving one behind is exactly the failure this project set out to avoid.
pub async fn close_every_session(state: &AppState) {
    if let Some(sessions) = state.sessions() {
        sessions.close_all().await;
    }
}

/// Records where a viewer got to.
///
/// Answers whether the report was kept: one arriving after a fresher one is
/// refused, so a client that reconnects cannot make the resume point go
/// backwards.
pub async fn record_position(
    state: &AppState,
    user_id: UserId,
    work_id: WorkId,
    position: Millis,
    reported_at: Timestamp,
) -> Result<bool> {
    let database = state.database();
    let duration = longest_version(state, work_id).await?;

    let marked_manually = database
        .playback_progress(user_id, work_id)
        .await?
        .is_some_and(|progress| progress.marked_manually);

    let state_now = state_for_position(
        position,
        duration,
        DEFAULT_WATCHED_THRESHOLD,
        marked_manually,
    );

    Ok(database
        .record_playback_progress(user_id, work_id, position, state_now, reported_at)
        .await?)
}

/// How long the work runs, from the longest copy of it that was analysed.
///
/// The runtime a provider gave describes the film; what decides whether
/// someone reached the end is the file they are actually watching.
async fn longest_version(state: &AppState, work_id: WorkId) -> Result<Option<Millis>> {
    let database = state.database();
    let mut longest = None;
    for source in database.sources_of_work(work_id).await? {
        if let Some((analysis, _)) = database.source_details(source.id).await? {
            longest = match (longest, analysis.duration) {
                (Some(known), Some(found)) if found > known => Some(found),
                (None, found) => found,
                (known, _) => known,
            };
        }
    }
    Ok(longest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::MediaSourceId;
    use melyxar_core::media::{
        AudioDetails, ColorInfo, Loudness, SubtitleDetails, SubtitleLayout, TrackKind, VideoDetails,
    };
    use melyxar_core::user::Permissions;
    use melyxar_database::catalogue::SourceAnalysis;
    use melyxar_database::Database;
    use melyxar_playback::decision::{PlaybackMethod, SubtitleDelivery};
    use time::macros::datetime;

    fn video(source_id: MediaSourceId, codec: &str, height: i32) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: codec.to_string(),
                profile: Some("High".into()),
                level: Some(40),
                width: height * 16 / 9,
                height,
                margins: None,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: Some(24.0),
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr: None,
            }),
        }
    }

    fn audio(source_id: MediaSourceId, codec: &str, channels: i32, is_default: bool) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 1,
            language: Some("fre".into()),
            title: None,
            is_default,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: codec.to_string(),
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

    fn subtitle(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 2,
            language: Some("fre".into()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".into(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: false,
                external_relative_path: None,
            }),
        }
    }

    /// A subtitle made of pictures, the kind a disc carries.
    fn picture_subtitle(source_id: MediaSourceId) -> Track {
        let mut track = subtitle(source_id);
        track.kind = TrackKind::Subtitle(SubtitleDetails {
            codec: "dvd_subtitle".into(),
            layout: SubtitleLayout::Bitmap,
            is_hearing_impaired: false,
            is_external: false,
            external_relative_path: None,
        });
        track
    }

    /// One film in one library, with the file named as the client will ask
    /// for it and the tracks a browser has to judge.
    async fn state_with_film(
        file_name: &str,
        container: &str,
        tracks: impl FnOnce(MediaSourceId) -> Vec<Track>,
    ) -> (tempfile::TempDir, AppState, UserId, MediaSourceId) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("media folder");
        std::fs::write(media.join(file_name), b"not a real film").expect("file written");

        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            libraries: vec![melyxar_config::LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![melyxar_config::RootConfig {
                    label: "disk-one".into(),
                    path: media,
                }],
            }],
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        let library = database
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("declared");
        let work = database
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let source_id = database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new(file_name),
                12_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some(container.to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &tracks(source_id),
                &[],
            )
            .await
            .expect("analysis stored");

        let user = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        let state = AppState::new(config, database, None, None);
        (directory, state, user.id, source_id)
    }

    #[tokio::test]
    async fn a_film_a_browser_can_open_is_handed_over_untouched() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        assert_eq!(plan.decision.method, PlaybackMethod::DirectPlay);
        assert!(plan.path.ends_with("Quiet.Harbour.2019.mp4"));
        assert_eq!(plan.size_bytes, 12_000);
        assert_eq!(plan.duration, Some(Millis::new(7_200_000)));
        assert!(plan.resume_from.is_none(), "nobody has watched it yet");
    }

    #[tokio::test]
    async fn a_soundtrack_no_browser_decodes_is_rebuilt_and_the_picture_is_not() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "h264", 1080), audio(id, "eac3", 6, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        assert_eq!(
            plan.decision.method,
            PlaybackMethod::TranscodeAudio,
            "the common case here: the picture is fine and the sound is not"
        );
        assert_eq!(
            plan.decision.video,
            melyxar_playback::decision::StreamAction::Copy,
            "rebuilding a picture that plays perfectly well costs an hour of a graphics card"
        );
        assert!(!plan.decision.reasons.is_empty(), "and it says why");
    }

    #[tokio::test]
    async fn a_chosen_subtitle_travels_beside_the_picture() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![
                    video(id, "h264", 1080),
                    audio(id, "aac", 2, true),
                    subtitle(id),
                ]
            })
            .await;

        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let chosen = tracks
            .iter()
            .find(|track| matches!(track.kind, TrackKind::Subtitle(_)))
            .expect("a subtitle");

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: Some(chosen.id),
            },
        )
        .await
        .expect("a plan");

        assert_eq!(plan.decision.subtitles, SubtitleDelivery::External);
        assert_eq!(plan.decision.subtitle_stream_index, Some(2));
    }

    /// Two soundtracks, French second, so the file's own order and the
    /// viewer's preference disagree.
    fn two_soundtracks(source_id: MediaSourceId) -> Vec<Track> {
        let mut english = audio(source_id, "aac", 2, true);
        english.language = Some("eng".to_string());
        english.stream_index = 1;
        let mut french = audio(source_id, "aac", 2, false);
        french.language = Some("fre".to_string());
        french.stream_index = 2;
        vec![video(source_id, "h264", 1080), english, french]
    }

    async fn soundtrack_of(
        state: &AppState,
        user: UserId,
        source: MediaSourceId,
    ) -> Option<String> {
        let plan = plan(
            state,
            user,
            &PlayRequest {
                source_id: source,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        let index = plan.decision.audio_stream_index?;
        plan.tracks
            .iter()
            .find(|track| track.stream_index == index)
            .and_then(|track| track.language.clone())
    }

    #[tokio::test]
    async fn without_a_preference_the_file_decides_which_soundtrack_plays() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", two_soundtracks).await;
        assert_eq!(
            soundtrack_of(&state, user_id, source_id).await.as_deref(),
            Some("eng"),
            "the one the file marks as default"
        );
    }

    #[tokio::test]
    async fn the_language_a_viewer_prefers_is_picked_without_being_asked_again() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", two_soundtracks).await;

        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let french = tracks
            .iter()
            .find(|track| track.language.as_deref() == Some("fre"))
            .expect("a French soundtrack");
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        remember_chosen_tracks(&state, user_id, work_id, Some(french), None)
            .await
            .expect("choice remembered");

        assert_eq!(
            soundtrack_of(&state, user_id, source_id).await.as_deref(),
            Some("fre"),
            "the same film starts the way it was left"
        );

        // Another film altogether, whose tracks are numbered differently: the
        // language is what carries over, not the identifier.
        let (_elsewhere, other_state, other_user, other_source) =
            state_with_film("Amber.Field.2020.mp4", "mov,mp4,m4a", two_soundtracks).await;
        let mut preferences = other_state
            .database()
            .user(other_user)
            .await
            .expect("read")
            .expect("present")
            .preferences;
        preferences.preferred_audio_language = Some("fr".to_string());
        other_state
            .database()
            .save_preferences(other_user, &preferences)
            .await
            .expect("preferences saved");

        assert_eq!(
            soundtrack_of(&other_state, other_user, other_source)
                .await
                .as_deref(),
            Some("fre"),
            "written as two letters or three, it is the same language"
        );
    }

    #[tokio::test]
    async fn what_was_chosen_for_this_film_beats_the_language_preferred_in_general() {
        // A viewer who watches everything in French but this one film in its
        // own language gets that film in its own language.
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", two_soundtracks).await;
        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let english = tracks
            .iter()
            .find(|track| track.language.as_deref() == Some("eng"))
            .expect("an English soundtrack");
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        state
            .database()
            .record_chosen_tracks(user_id, work_id, Some(english.id), None)
            .await
            .expect("choice recorded");
        let mut preferences = state
            .database()
            .user(user_id)
            .await
            .expect("read")
            .expect("present")
            .preferences;
        preferences.preferred_audio_language = Some("fre".to_string());
        state
            .database()
            .save_preferences(user_id, &preferences)
            .await
            .expect("preferences saved");

        assert_eq!(
            soundtrack_of(&state, user_id, source_id).await.as_deref(),
            Some("eng"),
            "the choice made about this film is more particular than a habit"
        );
    }

    #[tokio::test]
    async fn a_film_never_starts_with_subtitles_nobody_asked_for() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![
                    video(id, "h264", 1080),
                    audio(id, "aac", 2, true),
                    subtitle(id),
                ]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.decision.subtitles, SubtitleDelivery::None);
        assert_eq!(plan.decision.subtitle_stream_index, None);
    }

    #[tokio::test]
    async fn a_fold_to_stereo_someone_asked_for_is_a_reason_of_its_own() {
        // The preference has to reach the decision, or it quietly does nothing
        // on every film a client could have played as it is. The client here
        // accepts six channels, so nothing else explains the rebuild.
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 6, true)]
            })
            .await;

        let mut profile = ClientProfile::conservative_browser();
        profile.max_audio_channels = Some(6);
        let request = PlayRequest {
            source_id,
            profile: Some(profile),
            audio_track_id: None,
            subtitle_track_id: None,
        };

        // A server nobody has configured already folds the sound its own way
        // rather than leaving it to the browser, whose fold buries the
        // dialogue under the effects. That costs a rebuild of the sound, and
        // the answer says as much.
        let out_of_the_box = plan(&state, user_id, &request).await.expect("a plan");
        assert_eq!(
            out_of_the_box.decision.method,
            PlaybackMethod::TranscodeAudio
        );

        let mut preferences = state
            .database()
            .user(user_id)
            .await
            .expect("read")
            .expect("present")
            .preferences;
        preferences.downmix_method = melyxar_core::user::DownmixMethod::None;
        state
            .database()
            .save_preferences(user_id, &preferences)
            .await
            .expect("preferences saved");
        assert_eq!(
            plan(&state, user_id, &request)
                .await
                .expect("a plan")
                .decision
                .method,
            PlaybackMethod::DirectPlay,
            "a viewer who leaves the fold to the browser gets the file untouched"
        );

        preferences.downmix_method = melyxar_core::user::DownmixMethod::NightDialogue;
        state
            .database()
            .save_preferences(user_id, &preferences)
            .await
            .expect("preferences saved");

        let folded = plan(&state, user_id, &request).await.expect("a plan");
        assert_eq!(
            folded.decision.method,
            PlaybackMethod::TranscodeAudio,
            "a way of folding to stereo can only be applied by rebuilding the sound"
        );
        assert!(
            folded.decision.reasons.iter().any(|reason| matches!(
                reason,
                melyxar_playback::decision::Reason::DownmixRequested { .. }
            )),
            "and the answer says it was asked for: {:?}",
            folded.decision.reasons
        );
    }

    fn capabilities_of_a_usual_tool() -> melyxar_ffmpeg::Capabilities {
        melyxar_ffmpeg::Capabilities {
            version: "ffmpeg version invented".to_string(),
            encoders: ["libx264", "aac"].iter().map(|v| v.to_string()).collect(),
            decoders: Default::default(),
            filters: Default::default(),
            hardware: Default::default(),
            card_search: Default::default(),
        }
    }

    #[tokio::test]
    async fn a_rebuilt_picture_is_cut_where_the_segments_are() {
        // Without key frames on the boundaries, a segment can only be produced
        // after the one before it, and jumping stops working: the whole reason
        // the server owns the playlist would be lost.
        use melyxar_ffmpeg::command::VideoOutput;

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "hevc", 2160), audio(id, "eac3", 6, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.decision.method, PlaybackMethod::FullTranscode);

        let recipe = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        let VideoOutput::Encode(encode) = recipe.video else {
            panic!("a picture no browser reads is rebuilt");
        };
        assert_eq!(
            encode.keyframe_interval,
            Some(melyxar_streaming::playlist::SEGMENT_DURATION),
            "a key frame on every segment boundary"
        );
        assert!(matches!(
            recipe.audio,
            melyxar_ffmpeg::command::AudioOutput::Encode(_)
        ));
    }

    #[tokio::test]
    async fn a_picture_a_browser_reads_is_copied_and_only_the_sound_is_rebuilt() {
        use melyxar_ffmpeg::command::{AudioOutput, VideoOutput};

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "h264", 1080), audio(id, "eac3", 6, true)]
            })
            .await;

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        let recipe = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        assert_eq!(
            recipe.video,
            VideoOutput::Copy,
            "rebuilding a picture that plays perfectly well costs an hour of a machine"
        );
        assert!(matches!(recipe.audio, AudioOutput::Encode(_)));
        assert_eq!(recipe.duration, Millis::new(7_200_000));
    }

    #[tokio::test]
    async fn the_fold_a_viewer_asked_for_is_the_one_actually_performed() {
        // Asking for it is what turned a copy into a rebuild. Producing a
        // different fold would be the worst of both: the cost of the work
        // without the point of it.
        use melyxar_core::user::DownmixMethod;
        use melyxar_ffmpeg::command::AudioOutput;

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "h264", 1080), audio(id, "eac3", 6, true)]
            })
            .await;

        let mut preferences = state
            .database()
            .user(user_id)
            .await
            .expect("read")
            .expect("the account")
            .preferences;
        preferences.downmix_method = DownmixMethod::NightDialogue;
        preferences.downmix_gain = 2.5;
        state
            .database()
            .save_preferences(user_id, &preferences)
            .await
            .expect("preferences kept");

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.downmix, DownmixMethod::NightDialogue);

        let recipe = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        let AudioOutput::Encode(encode) = recipe.audio else {
            panic!("a soundtrack no browser reads is rebuilt");
        };
        assert_eq!(encode.downmix, DownmixMethod::NightDialogue);
        assert_eq!(encode.downmix_gain, 2.5);
        assert!(
            encode.limiter,
            "the compensation gain is followed by a limiter, or a loud passage clips"
        );
    }

    #[tokio::test]
    async fn a_subtitle_made_of_pictures_is_painted_onto_every_frame() {
        // There is no text in one to hand a browser, so painting it on is the
        // only way to show it, and that is what makes it cost a full rebuild.
        use melyxar_ffmpeg::command::VideoOutput;

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![
                    video(id, "h264", 1080),
                    audio(id, "aac", 2, true),
                    picture_subtitle(id),
                ]
            })
            .await;

        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let words = tracks
            .iter()
            .find(|track| matches!(track.kind, TrackKind::Subtitle(_)))
            .expect("the film carries one");

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: Some(words.id),
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.decision.subtitles, SubtitleDelivery::BurnIn);

        let recipe = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        let VideoOutput::Encode(encode) = recipe.video else {
            panic!("painting a subtitle on means rebuilding the picture");
        };
        assert_eq!(encode.burn_in_subtitle, Some(words.stream_index));
    }

    #[tokio::test]
    async fn a_subtitle_made_of_words_is_never_painted_onto_the_picture() {
        // The tool paints pictures onto pictures. Asking it to paint words on
        // would cost a full rebuild and put nothing on the screen, which is
        // the worst of both.
        use melyxar_ffmpeg::command::VideoOutput;

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![
                    video(id, "hevc", 2160),
                    audio(id, "aac", 2, true),
                    subtitle(id),
                ]
            })
            .await;

        let tracks = state
            .database()
            .tracks_of_source(source_id)
            .await
            .expect("read");
        let words = tracks
            .iter()
            .find(|track| matches!(track.kind, TrackKind::Subtitle(_)))
            .expect("the film carries one");

        let mut plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: Some(words.id),
            },
        )
        .await
        .expect("a plan");
        // Said outright rather than found: the one client there is draws these
        // itself, so the decision never asks for this today.
        plan.decision.subtitles = SubtitleDelivery::BurnIn;
        plan.decision.subtitle_stream_index = Some(words.stream_index);

        let recipe = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        let VideoOutput::Encode(encode) = recipe.video else {
            panic!("this picture is rebuilt for its own reasons");
        };
        assert_eq!(encode.burn_in_subtitle, None);
    }

    #[tokio::test]
    async fn a_film_nobody_has_looked_inside_cannot_be_cut_into_segments() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "h264", 1080), audio(id, "eac3", 6, true)]
            })
            .await;

        let mut plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        plan.duration = None;

        assert!(
            recipe_for(&plan, &capabilities_of_a_usual_tool()).is_err(),
            "without a length there is no playlist to write, and a player would \
             be handed a film of no duration"
        );
    }

    /// The same server, but able to convert: the sessions only exist when the
    /// media tools do.
    async fn state_that_can_convert() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        assert!(tools.is_some(), "the tools are installed here");

        let database = Database::open_in_memory().await.expect("database opens");
        let state = AppState::new(config, database, tools, capabilities);
        (directory, state)
    }

    #[tokio::test]
    async fn folders_of_a_run_that_is_over_are_gone_before_this_one_serves() {
        // A crash, a power cut, a container killed outright: nothing else ever
        // comes back for these, and the disk fills up quietly.
        let (directory, state) = state_that_can_convert().await;
        let left_behind = directory
            .path()
            .join("cache/transcodes")
            .join("01a0-left-behind");
        std::fs::create_dir_all(&left_behind).expect("an old folder");

        tidy_up_after_a_previous_run(&state).await;

        assert!(!left_behind.exists());
    }

    #[tokio::test]
    async fn stopping_closes_every_session_rather_than_leaving_a_tool_running() {
        let (_directory, state) = state_that_can_convert().await;
        let sessions = state.sessions().expect("this server converts").clone();
        sessions
            .open(
                Recipe {
                    source: std::path::PathBuf::from("Quiet.Harbour.2019.mkv"),
                    duration: Millis::new(20_000),
                    streams: melyxar_ffmpeg::command::StreamSelection::default(),
                    video: melyxar_ffmpeg::command::VideoOutput::Copy,
                    audio: melyxar_ffmpeg::command::AudioOutput::Copy,
                    where_the_viewer_starts: Millis::ZERO,
                    where_it_can_be_started: Vec::new(),
                    if_the_card_refuses: Vec::new(),
                },
                false,
            )
            .await
            .expect("a session");
        assert_eq!(sessions.live_count().await, 1);

        close_every_session(&state).await;

        assert_eq!(
            sessions.live_count().await,
            0,
            "a session outliving the server is the failure this project set out to avoid"
        );
    }

    #[tokio::test]
    async fn a_session_nobody_came_back_for_is_swept_while_the_server_runs() {
        // The viewer closed the tab, so nothing will ever ask this session for
        // anything again. Nobody is coming to clean up but this loop.
        let (_directory, state) = state_that_can_convert().await;
        let sessions = state.sessions().expect("this server converts").clone();
        sessions
            .open(
                Recipe {
                    source: std::path::PathBuf::from("Quiet.Harbour.2019.mkv"),
                    duration: Millis::new(20_000),
                    streams: melyxar_ffmpeg::command::StreamSelection::default(),
                    video: melyxar_ffmpeg::command::VideoOutput::Copy,
                    audio: melyxar_ffmpeg::command::AudioOutput::Copy,
                    where_the_viewer_starts: Millis::ZERO,
                    where_it_can_be_started: Vec::new(),
                    if_the_card_refuses: Vec::new(),
                },
                false,
            )
            .await
            .expect("a session");

        let sweeper = sweep_repeatedly(
            &state,
            std::time::Duration::from_millis(10),
            std::time::Duration::from_millis(20),
        );
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        sweeper.abort();

        assert_eq!(
            sessions.live_count().await,
            0,
            "a session nobody came back for has to go on its own"
        );
    }

    #[tokio::test]
    async fn a_server_that_converts_nothing_stops_and_tidies_without_trouble() {
        // No media tools, so no sessions at all. Both must be a quiet no-op
        // rather than something the binary has to guard against.
        let (_directory, state, _user_id, _source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        assert!(state.sessions().is_none());

        tidy_up_after_a_previous_run(&state).await;
        close_every_session(&state).await;
    }

    #[tokio::test]
    async fn a_file_that_is_not_on_the_disk_is_refused_rather_than_opened() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        state
            .database()
            .mark_source_missing(source_id)
            .await
            .expect("marked");

        let outcome = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await;
        assert!(
            outcome.is_err(),
            "a viewer is told before pressing play, not halfway through"
        );
    }

    #[test]
    fn a_picture_rebuilt_in_software_is_never_rebuilt_larger_than_one_can_be() {
        assert_eq!(
            height_to_rebuild_at(Some(2160), None),
            Some(1080),
            "a processor cannot rebuild a picture that size while somebody watches it"
        );
        assert_eq!(
            height_to_rebuild_at(Some(2160), Some(720)),
            Some(720),
            "a client asking for less is asking for less, not for the ceiling"
        );
        assert_eq!(
            height_to_rebuild_at(Some(1080), None),
            None,
            "resizing a picture to its own size is work for nothing"
        );
        assert_eq!(height_to_rebuild_at(Some(720), None), None);
        assert_eq!(height_to_rebuild_at(None, Some(720)), None);
    }

    /// A decision to rebuild the picture, as the engine would produce one.
    fn rebuilding(
        height_asked: Option<i32>,
        tone_map: bool,
        rate: Option<i64>,
    ) -> PlaybackDecision {
        PlaybackDecision {
            method: PlaybackMethod::FullTranscode,
            video: StreamAction::Transcode,
            audio: StreamAction::Transcode,
            subtitles: SubtitleDelivery::None,
            audio_stream_index: None,
            subtitle_stream_index: None,
            video_stream_index: Some(0),
            scale_to_height: height_asked,
            bitrate_ceiling: rate,
            tone_map,
            reasons: Vec::new(),
        }
    }

    /// A card, with a say in what it was proved able to do. It reads every
    /// codec it writes here; the tests that care say otherwise themselves.
    fn a_card(codecs: &[&str], can_tone_map: bool) -> melyxar_ffmpeg::Card {
        melyxar_ffmpeg::Card {
            way: melyxar_ffmpeg::HardwareAcceleration::Vaapi,
            device: PathBuf::from("/dev/dri/renderD128"),
            encoders: codecs
                .iter()
                .map(|codec| ((*codec).to_string(), format!("{codec}_vaapi")))
                .collect(),
            decoders: codecs.iter().map(|codec| (*codec).to_string()).collect(),
            can_scale: true,
            can_tone_map,
        }
    }

    fn capabilities_with(card: Option<melyxar_ffmpeg::Card>) -> melyxar_ffmpeg::Capabilities {
        melyxar_ffmpeg::Capabilities {
            card_search: melyxar_ffmpeg::CardSearch {
                card,
                ..Default::default()
            },
            ..capabilities_of_a_usual_tool()
        }
    }

    #[test]
    fn a_card_rebuilds_the_picture_in_the_best_codec_the_client_keeps_up_with() {
        // A card produces all three at much the same speed, so the newer ones
        // cost nothing here. What settles which one a viewer gets is how far
        // the client said it decodes each of them, never a rule written here.
        let tracks = vec![video(MediaSourceId::new(), "hevc", 2160)];
        let card = capabilities_with(Some(a_card(&["h264", "hevc", "av1"], true)));

        let keeps_up_with_everything = ClientProfile {
            rebuilt_video: vec![
                RebuiltCapability::any("h264"),
                RebuiltCapability::any("hevc"),
                RebuiltCapability::any("av1"),
            ],
            ..ClientProfile::conservative_browser()
        };
        let rebuild = how_to_rebuild(
            &rebuilding(None, true, None),
            &tracks,
            &keeps_up_with_everything,
            Some(&card),
            false,
        )
        .expect("this picture is rebuilt");
        assert!(rebuild.on_a_card());
        assert_eq!(rebuild.codec, "av1");
        assert_eq!(
            rebuild.height, None,
            "a card rebuilds a picture at its own size, and the ceiling is the processor's"
        );

        // The same client, and the newer codec only up to half this film's
        // size. That is the ordinary answer of a machine without the part that
        // decodes it, and a film rebuilt past it never shows a frame.
        let only_so_far = ClientProfile {
            rebuilt_video: vec![
                RebuiltCapability::any("h264"),
                RebuiltCapability {
                    codec: "av1".into(),
                    max_height: Some(1080),
                },
            ],
            ..ClientProfile::conservative_browser()
        };
        let at_its_own_size = how_to_rebuild(
            &rebuilding(None, true, None),
            &tracks,
            &only_so_far,
            Some(&card),
            false,
        )
        .expect("this picture is rebuilt");
        assert_eq!(
            at_its_own_size.codec, "h264",
            "past what the client answered, the codec it never refuses"
        );

        let made_smaller = how_to_rebuild(
            &rebuilding(Some(1080), true, None),
            &tracks,
            &only_so_far,
            Some(&card),
            false,
        )
        .expect("this picture is rebuilt");
        assert_eq!(
            made_smaller.codec, "av1",
            "and within it, the newer codec, which is what the viewer gains by asking for less"
        );

        // Nothing the card writes keeps up at this size: the picture is made
        // smaller rather than handed over in a codec that will not show.
        let only_small_av1 = ClientProfile {
            rebuilt_video: vec![RebuiltCapability {
                codec: "av1".into(),
                max_height: Some(1080),
            }],
            ..ClientProfile::conservative_browser()
        };
        let made_to_fit = how_to_rebuild(
            &rebuilding(None, true, None),
            &tracks,
            &only_small_av1,
            Some(&card),
            false,
        )
        .expect("this picture is rebuilt");
        assert_eq!(made_to_fit.codec, "av1");
        assert_eq!(
            made_to_fit.height,
            Some(1080),
            "a client that cannot follow this film in anything is given a smaller one"
        );
        assert!(
            made_to_fit.on_a_card(),
            "and the card keeps the work: the picture is smaller, not the processor's"
        );

        // A client that measured nothing: what every client reads, because a
        // guess wrong here is a black screen.
        let rebuild = how_to_rebuild(
            &rebuilding(None, true, None),
            &tracks,
            &ClientProfile::conservative_browser(),
            Some(&card),
            false,
        )
        .expect("this picture is rebuilt");
        assert_eq!(rebuild.codec, "h264");
        assert!(rebuild.on_a_card());
    }

    #[test]
    fn a_rate_a_viewer_asked_for_is_the_rate_the_card_is_given() {
        let tracks = vec![video(MediaSourceId::new(), "hevc", 2160)];
        let card = capabilities_with(Some(a_card(&["h264", "av1"], true)));
        let profile = ClientProfile {
            rebuilt_video: vec![
                RebuiltCapability::any("h264"),
                RebuiltCapability::any("av1"),
            ],
            max_bitrate: Some(4_000_000),
            max_height: Some(720),
            ..ClientProfile::conservative_browser()
        };

        let rebuild = how_to_rebuild(
            &rebuilding(Some(720), false, Some(4_000_000)),
            &tracks,
            &profile,
            Some(&card),
            false,
        )
        .expect("this picture is rebuilt");
        assert_eq!(rebuild.height, Some(720));
        assert_eq!(rebuild.bitrate, Some(4_000_000));

        // No rate asked for, only a size: the card is still given one,
        // because it counts quality on a scale of its own for each codec, and
        // it is the usual rate for that size and that codec.
        let only_a_size = how_to_rebuild(
            &rebuilding(Some(720), false, None),
            &tracks,
            &profile,
            Some(&card),
            false,
        )
        .expect("this picture is rebuilt");
        assert_eq!(only_a_size.codec, "av1");
        assert_eq!(only_a_size.bitrate, Some(rate_for(Some(720), "av1")));
        assert!(
            only_a_size.bitrate < Some(rate_for(Some(720), "h264")),
            "needing less is the whole point of the newer codec"
        );
    }

    #[tokio::test]
    async fn a_card_reads_the_film_itself_only_in_a_codec_it_was_proved_to_read() {
        // Reading and writing are separate abilities and separate lists. A
        // film in a codec the card never proved it reads is decoded by the
        // processor and handed up, which works whatever the film holds.
        use melyxar_ffmpeg::command::VideoOutput;

        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                vec![video(id, "hevc", 2160), audio(id, "eac3", 6, true)]
            })
            .await;
        let mut plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        let tracks = plan.tracks.clone();
        let profile = ClientProfile::conservative_browser();
        let reads_hevc = capabilities_with(Some(a_card(&["h264", "hevc"], true)));
        let reads_nothing_useful = capabilities_with(Some(melyxar_ffmpeg::Card {
            decoders: Default::default(),
            ..a_card(&["h264", "hevc"], true)
        }));

        let on_the_card =
            how_to_rebuild(&plan.decision, &tracks, &profile, Some(&reads_hevc), false)
                .expect("this picture is rebuilt");
        assert!(on_the_card.reads_the_film, "the card was proved to read it");

        let handed_up = how_to_rebuild(
            &plan.decision,
            &tracks,
            &profile,
            Some(&reads_nothing_useful),
            false,
        )
        .expect("this picture is rebuilt");
        assert!(handed_up.on_a_card(), "it still rebuilds the picture");
        assert!(!handed_up.reads_the_film);

        // A card reading the film has one more way to step down than a card
        // that is only rebuilding the picture: a card that will not read one
        // film will still rebuild it, and giving up entirely at the first
        // refusal would throw away most of what it was doing.
        plan.rebuild = Some(on_the_card);
        let reading = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        assert_eq!(reading.if_the_card_refuses.len(), 2);
        assert!(matches!(
            &reading.if_the_card_refuses[0],
            VideoOutput::Encode(encode) if encode.card().is_some()
        ));
        assert!(matches!(
            &reading.if_the_card_refuses[1],
            VideoOutput::Encode(encode) if encode.card().is_none()
        ));

        plan.rebuild = Some(handed_up);
        let handed = recipe_for(&plan, &capabilities_of_a_usual_tool()).expect("a recipe");
        assert_eq!(handed.if_the_card_refuses.len(), 1);
    }

    #[tokio::test]
    async fn a_film_that_says_part_of_its_frame_is_not_the_picture_is_read_by_the_processor() {
        // A card hands its pictures on as whole frames, and nothing in the
        // chain that follows knows how to cut the edges off one: the film
        // would be rebuilt frame and all, a wide picture stretched to fill a
        // shape it never had. The processor cuts them as it reads, which is
        // what every still image pulled out of these files has always shown.
        // The card still rebuilds the picture; it just does not read the film.
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mkv", "matroska,webm", |id| {
                let mut picture = video(id, "hevc", 2160);
                if let TrackKind::Video(details) = &mut picture.kind {
                    details.margins = Some(melyxar_core::media::Margins {
                        top: 276,
                        bottom: 276,
                        left: 0,
                        right: 0,
                    });
                }
                vec![picture, audio(id, "eac3", 6, true)]
            })
            .await;
        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");

        let reads_hevc = capabilities_with(Some(a_card(&["h264", "hevc"], true)));
        let rebuild = how_to_rebuild(
            &plan.decision,
            &plan.tracks,
            &ClientProfile::conservative_browser(),
            Some(&reads_hevc),
            false,
        )
        .expect("this picture is rebuilt");

        assert!(
            rebuild.on_a_card(),
            "the card is still what rebuilds the picture"
        );
        assert!(
            !rebuild.reads_the_film,
            "the card was proved to read this codec, and still must not read this film"
        );
    }

    #[test]
    fn a_picture_nobody_is_rebuilding_has_nothing_to_say_about_how() {
        let tracks = vec![video(MediaSourceId::new(), "h264", 1080)];
        let mut untouched = rebuilding(None, false, None);
        untouched.video = StreamAction::Copy;
        assert!(how_to_rebuild(
            &untouched,
            &tracks,
            &ClientProfile::conservative_browser(),
            Some(&capabilities_with(Some(a_card(&["h264"], true)))),
            false,
        )
        .is_none());
    }

    #[tokio::test]
    async fn a_file_nothing_could_read_is_refused_with_a_reason_of_its_own() {
        // Such a file has no container and no streams recorded, so there is
        // nothing to decide with. Left to go ahead it was described as a
        // repackaging, for no reason anybody could read, and the tool then
        // refused it after a wait.
        let (_directory, state, user_id, source_id) =
            state_with_film("Le.Cirque.mkv", "matroska,webm", |_| Vec::new()).await;
        state
            .database()
            .forget_analysis(state.database().list_libraries().await.expect("read")[0].id)
            .await
            .expect("the analysis is forgotten");

        let outcome = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await;

        match outcome {
            Err(AppError::Domain(error)) => assert_eq!(
                error.code,
                melyxar_core::error::ErrorCode::NotDescribed,
                "a viewer has to be told it is the file, not the server"
            ),
            other => panic!("a file nothing could read is not playable: {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_film_someone_left_halfway_offers_to_carry_on() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        assert!(record_position(
            &state,
            user_id,
            work_id,
            Millis::new(1_800_000),
            datetime!(2026-01-01 12:00 UTC),
        )
        .await
        .expect("recorded"));

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert_eq!(plan.resume_from, Some(Millis::new(1_800_000)));
    }

    #[tokio::test]
    async fn a_film_watched_to_the_end_starts_again_from_the_beginning() {
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        // Two hours in, of a two hour film.
        record_position(
            &state,
            user_id,
            work_id,
            Millis::new(7_100_000),
            datetime!(2026-01-01 12:00 UTC),
        )
        .await
        .expect("recorded");

        assert_eq!(
            state
                .database()
                .playback_progress(user_id, work_id)
                .await
                .expect("read")
                .expect("present")
                .state,
            PlaybackState::Watched
        );

        let plan = plan(
            &state,
            user_id,
            &PlayRequest {
                source_id,
                profile: None,
                audio_track_id: None,
                subtitle_track_id: None,
            },
        )
        .await
        .expect("a plan");
        assert!(
            plan.resume_from.is_none(),
            "offering to carry on ten seconds before the credits helps nobody"
        );
    }

    #[tokio::test]
    async fn where_someone_is_counts_against_the_copy_they_are_watching() {
        // The runtime a provider gave describes the film; the file is what
        // decides whether someone reached the end of it.
        let (_directory, state, user_id, source_id) =
            state_with_film("Quiet.Harbour.2019.mp4", "mov,mp4,m4a", |id| {
                vec![video(id, "h264", 1080), audio(id, "aac", 2, true)]
            })
            .await;
        let work_id = state
            .database()
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present")
            .work_id;

        record_position(
            &state,
            user_id,
            work_id,
            Millis::new(60_000),
            datetime!(2026-01-01 12:00 UTC),
        )
        .await
        .expect("recorded");

        assert_eq!(
            state
                .database()
                .playback_progress(user_id, work_id)
                .await
                .expect("read")
                .expect("present")
                .state,
            PlaybackState::InProgress,
            "a minute into a two hour film is not the end of it"
        );
    }
}
