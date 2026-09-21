//! The rows a home page is made of that belong to nobody in particular.
//!
//! What somebody left halfway and what their series are waiting on live next
//! to the progress they are read from. What is here is the other half: what
//! the server was told to put in front of everybody, and what it picks when
//! nobody told it anything.

use melyxar_core::id::{LibraryId, UserId, WorkId};
use melyxar_core::time::now;
use sqlx::{AssertSqlSafe, Row};

use std::collections::HashMap;

use crate::browse::{kept_inside, met_on_its_own, WorkCard, WHAT_A_CARD_IS};
use crate::convert::{parse_id, timestamp_to_text};
use crate::images::StoredImage;
use crate::{Database, Result};

/// How many genres of somebody's own count as what they watch.
///
/// Five rather than all of them: a person who has watched forty films has
/// touched nearly every genre there is, and weighing them all equally is the
/// same as weighing none of them.
const GENRES_THAT_COUNT: i64 = 5;

/// What a work needs to be shown large rather than as a card.
///
/// A card is a poster, a title and a year, which is everything a grid needs
/// and nothing the one place that shows a work full width does. This is the
/// rest: the wide picture behind it, the title as the film's own designers
/// drew it, and enough words to say what it is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dressed {
    pub backdrop: Vec<StoredImage>,
    pub logo: Vec<StoredImage>,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    /// How tall the picture of the best copy is, which is what a badge
    /// saying 4K is really saying.
    pub height: Option<i64>,
    /// hdr10, hlg or dolby_vision, when the picture carries one.
    pub hdr: Option<String>,
    /// What the fullest soundtrack is, as the file states it: the profile
    /// when there is one, since that is where Atmos is written, and the
    /// codec otherwise.
    pub sound: Option<String>,
}

impl Database {
    /// Dresses a handful of works for the one place that shows them large.
    ///
    /// Two questions for the lot rather than a detail page each: a detail page
    /// reads versions, tracks, credits and collections, which is a great deal
    /// of work to print a sentence and hang one picture.
    ///
    /// The words are taken in the language asked for and fall back to whatever
    /// the work has, which is the same rule the detail page follows: a page
    /// half empty is worse than a page with a paragraph somebody can read.
    pub async fn dressed_large(
        &self,
        works: &[WorkId],
        language: &str,
    ) -> Result<HashMap<WorkId, Dressed>> {
        let mut dressed: HashMap<WorkId, Dressed> = HashMap::new();
        if works.is_empty() {
            return Ok(dressed);
        }
        let owners: Vec<String> = works.iter().map(|id| id.to_db_string()).collect();
        // A row of question marks, one per identifier the caller just read
        // back out of this crate's own tables.
        let places = vec!["?"; owners.len()].join(", ");

        let mut pictures = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM images
              WHERE owner_kind = 'work'
                AND image_kind IN ('backdrop', 'logo')
                AND owner_id IN ({places})
              ORDER BY width DESC",
            crate::images::WHAT_A_PICTURE_IS
        )));
        for owner in &owners {
            pictures = pictures.bind(owner);
        }
        for row in pictures.fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("owner_id")?)?;
            let kind: String = row.try_get("image_kind")?;
            let image = crate::images::image_from_row(&row)?;
            let entry = dressed.entry(owner).or_default();
            match kind.as_str() {
                "logo" => entry.logo.push(image),
                _ => entry.backdrop.push(image),
            }
        }

        // The language asked for first, then anything the work has: ordered
        // here so the first row read for a work is the one that wins.
        let mut words = sqlx::query(AssertSqlSafe(format!(
            "SELECT work_id, tagline, overview
               FROM work_translations
              WHERE work_id IN ({places})
              ORDER BY (language = ?) DESC"
        )));
        for owner in &owners {
            words = words.bind(owner);
        }
        for row in words.bind(language).fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("work_id")?)?;
            let entry = dressed.entry(owner).or_default();
            if entry.overview.is_none() {
                entry.overview = row.try_get("overview")?;
            }
            if entry.tagline.is_none() {
                entry.tagline = row.try_get("tagline")?;
            }
        }

        let mut genres = sqlx::query(AssertSqlSafe(format!(
            "SELECT wg.work_id, g.name
               FROM work_genres wg
               JOIN genres g ON g.id = wg.genre_id
              WHERE wg.work_id IN ({places})"
        )));
        for owner in &owners {
            genres = genres.bind(owner);
        }
        for row in genres.fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("work_id")?)?;
            dressed
                .entry(owner)
                .or_default()
                .genres
                .push(row.try_get("name")?);
        }

        // What the file itself says about its picture and its sound, which is
        // what the badges beside a title are: read off the best copy on disk
        // rather than promised by the catalogue.
        let mut facts = sqlx::query(AssertSqlSafe(format!(
            "SELECT w.id AS work_id,
                    (SELECT t.height FROM tracks t
                       JOIN media_sources s ON s.id = t.source_id
                      WHERE s.work_id = w.id AND s.missing_since IS NULL
                        AND t.kind = 'video' AND t.height IS NOT NULL
                      ORDER BY t.height DESC LIMIT 1) AS height,
                    (SELECT t.hdr_format FROM tracks t
                       JOIN media_sources s ON s.id = t.source_id
                      WHERE s.work_id = w.id AND s.missing_since IS NULL
                        AND t.kind = 'video' AND t.hdr_format IS NOT NULL
                      LIMIT 1) AS hdr,
                    (SELECT coalesce(t.profile, t.codec) FROM tracks t
                       JOIN media_sources s ON s.id = t.source_id
                      WHERE s.work_id = w.id AND s.missing_since IS NULL
                        AND t.kind = 'audio'
                      ORDER BY t.channels DESC LIMIT 1) AS sound
               FROM works w
              WHERE w.id IN ({places})"
        )));
        for owner in &owners {
            facts = facts.bind(owner);
        }
        for row in facts.fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("work_id")?)?;
            let entry = dressed.entry(owner).or_default();
            entry.height = row.try_get("height")?;
            entry.hdr = row.try_get("hdr")?;
            entry.sound = row.try_get("sound")?;
        }
        Ok(dressed)
    }

    /// What an administrator put in front of everybody, in the order they
    /// chose.
    pub async fn pinned_works(
        &self,
        viewer: UserId,
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Vec<WorkCard>> {
        let Some(inside) = kept_inside(within, "w.library_id") else {
            return Ok(Vec::new());
        };
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS}
               FROM pinned_works p
               JOIN works w ON w.id = p.work_id
              WHERE {}{inside}
              ORDER BY p.rank, p.pinned_at
              LIMIT ?",
            met_on_its_own("w.")
        )));
        for granted in within.iter().copied().flatten() {
            query = query.bind(granted.to_db_string());
        }
        let rows = query.bind(limit).fetch_all(self.reader()).await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(cards)
    }

    /// Puts a work in front of everybody, at the end of what is already there.
    ///
    /// Pinning what is already pinned leaves it where it is rather than moving
    /// it to the end: an administrator who presses twice meant to pin it once,
    /// and the order of this row is arranged by hand.
    pub async fn pin_work(&self, work_id: WorkId) -> Result<()> {
        sqlx::query(
            "INSERT INTO pinned_works (work_id, rank, pinned_at)
             VALUES (?, coalesce((SELECT max(rank) FROM pinned_works), 0) + 1, ?)
             ON CONFLICT (work_id) DO NOTHING",
        )
        .bind(work_id.to_db_string())
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Takes a work back off the front page. Answers whether one was there.
    pub async fn unpin_work(&self, work_id: WorkId) -> Result<bool> {
        let gone = sqlx::query("DELETE FROM pinned_works WHERE work_id = ?")
            .bind(work_id.to_db_string())
            .execute(self.writer())
            .await?
            .rows_affected();
        Ok(gone > 0)
    }

    /// Whether this work is one of the ones put in front of everybody.
    pub async fn is_pinned(&self, work_id: WorkId) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM pinned_works WHERE work_id = ?")
            .bind(work_id.to_db_string())
            .fetch_optional(self.reader())
            .await?;
        Ok(row.is_some())
    }

    /// What this account might want to watch, said as honestly as the server
    /// can say it today.
    ///
    /// Works nobody here has started, well thought of elsewhere, kept to the
    /// genres this account watches most. There is no recommendation engine
    /// behind this and the name promises none: what has been watched is the
    /// only matter there is, and its genres are enough not to offer a horror
    /// film to somebody who only watches comedies.
    ///
    /// An account that has watched nothing yet gets the best rated things it
    /// has not started, which is the only honest answer to a question nobody
    /// has given any material for.
    ///
    /// Ordered by rating alone rather than by rating inside each genre, which
    /// is what lets it walk the index of well rated works and stop at the
    /// dozen it was asked for instead of reading and sorting every work that
    /// carries a rating.
    pub async fn suggestions(
        &self,
        viewer: UserId,
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Vec<WorkCard>> {
        // An account granted nothing has nothing to be suggested.
        if within.is_some_and(<[LibraryId]>::is_empty) {
            return Ok(Vec::new());
        }
        // Numbered throughout, and the libraries numbered on from four: the
        // viewer is named three times over, and one plain question mark among
        // them would shift every place after it.
        let granted = within.unwrap_or_default();
        let inside = match granted.is_empty() {
            true => String::new(),
            false => format!(
                " AND w.library_id IN ({})",
                (4..=granted.len() + 3)
                    .map(|place| format!("?{place}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };

        let mut query = sqlx::query(AssertSqlSafe(format!(
            "WITH watched_genres AS (
                 SELECT wg.genre_id
                   FROM playback_progress p
                   JOIN works e ON e.id = p.work_id
                   JOIN work_genres wg ON wg.work_id = e.id
                  WHERE p.user_id = ?1 AND p.state = 'watched'
                  GROUP BY wg.genre_id
                  ORDER BY count(*) DESC
                  LIMIT ?2
             )
             SELECT {WHAT_A_CARD_IS}
               FROM works w
               LEFT JOIN playback_progress p ON p.work_id = w.id AND p.user_id = ?1
              WHERE {}
                AND w.community_rating IS NOT NULL
                AND coalesce(p.state, 'not_started') = 'not_started'
                -- A series is not started by playing the series: it is started
                -- by playing an episode of it, so the row above says nothing
                -- about one and its episodes have to be asked.
                AND (w.kind <> 'series'
                     OR NOT EXISTS (
                         SELECT 1 FROM playback_progress q
                           JOIN works e ON e.id = q.work_id AND e.kind = 'episode'
                          WHERE q.user_id = ?1
                            AND q.state <> 'not_started'
                            AND (e.parent_id = w.id
                                 OR e.parent_id IN (SELECT id FROM works
                                                     WHERE parent_id = w.id))))
                -- Nothing watched yet leaves every genre open, which is the
                -- only honest answer before there is anything to go on.
                AND (NOT EXISTS (SELECT 1 FROM watched_genres)
                     OR EXISTS (SELECT 1 FROM work_genres wg
                                  JOIN watched_genres ON watched_genres.genre_id = wg.genre_id
                                 WHERE wg.work_id = w.id)){inside}
              ORDER BY w.community_rating DESC
              LIMIT ?3",
            met_on_its_own("w.")
        )))
        .bind(viewer.to_db_string())
        .bind(GENRES_THAT_COUNT)
        .bind(limit);
        for library in granted {
            query = query.bind(library.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::time::Millis;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::{PlaybackState, WorkKind};
    use std::path::PathBuf;

    /// A library of films, each with a rating and a genre, and two accounts.
    async fn a_shelf(films: &[(&str, f64, &str)]) -> (Database, LibraryId, UserId, Vec<WorkId>) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");

        let mut written = Vec::new();
        for (title, rating, genre) in films {
            let work = database
                .create_work(
                    library.id,
                    WorkKind::Movie,
                    title,
                    &title.to_lowercase(),
                    Some(2019),
                )
                .await
                .expect("work created");
            sqlx::query(
                "UPDATE works SET community_rating = ?, identification = 'identified'
                 WHERE id = ?",
            )
            .bind(rating)
            .bind(work.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("film completed");

            let genre_id = format!("genre-{genre}");
            sqlx::query("INSERT INTO genres (id, name) VALUES (?, ?) ON CONFLICT DO NOTHING")
                .bind(&genre_id)
                .bind(*genre)
                .execute(database.writer())
                .await
                .expect("genre written");
            sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, ?)")
                .bind(work.id.to_db_string())
                .bind(&genre_id)
                .execute(database.writer())
                .await
                .expect("genre attached");
            written.push(work.id);
        }

        let who = database
            .create_user("vera", None, &Permissions::viewer())
            .await
            .expect("account created")
            .id;
        (database, library.id, who, written)
    }

    #[tokio::test]
    async fn what_is_pinned_comes_back_in_the_order_it_was_pinned_in() {
        let (database, _, who, films) =
            a_shelf(&[("One", 7.0, "Drame"), ("Two", 8.0, "Drame")]).await;

        database.pin_work(films[1]).await.expect("pinned");
        database.pin_work(films[0]).await.expect("pinned");
        // Pressing twice on the same one leaves it where it is.
        database.pin_work(films[1]).await.expect("pinned again");

        let front = database
            .pinned_works(who, None, 10)
            .await
            .expect("pinned read");
        assert_eq!(
            front.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[1], films[0]]
        );

        assert!(database.is_pinned(films[0]).await.expect("read"));
        assert!(database.unpin_work(films[0]).await.expect("unpinned"));
        assert!(!database.is_pinned(films[0]).await.expect("read"));
        assert!(
            !database.unpin_work(films[0]).await.expect("unpinned"),
            "taking off what is not there says so"
        );

        assert!(
            database
                .pinned_works(who, Some(&[]), 10)
                .await
                .expect("pinned read")
                .is_empty(),
            "an account granted nothing sees nothing, front page included"
        );
    }

    #[tokio::test]
    async fn suggestions_lean_on_the_genres_this_account_watches() {
        let (database, _, who, films) = a_shelf(&[
            ("Watched Comedy", 6.0, "Comedie"),
            ("Great Horror", 9.5, "Horreur"),
            ("Good Comedy", 8.0, "Comedie"),
        ])
        .await;

        // Nothing watched yet: the best rated thing comes first, whatever it
        // is, because there is nothing to go on.
        let blind = database
            .suggestions(who, None, 10)
            .await
            .expect("suggestions read");
        assert_eq!(blind[0].id, films[1], "the best rated, with nothing to go on");

        database
            .mark_watched(who, films[0], true)
            .await
            .expect("marked");

        let leaning = database
            .suggestions(who, None, 10)
            .await
            .expect("suggestions read");
        assert_eq!(
            leaning.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[2]],
            "a comedy watcher is offered the comedy, and never what they watched"
        );
    }

    #[tokio::test]
    async fn a_work_shown_large_carries_its_genres_and_what_its_file_holds() {
        use melyxar_core::id::{MediaSourceId, TrackId};
        use melyxar_core::media::{
            AudioDetails, ColorInfo, HdrFormat, Loudness, Track, TrackKind, VideoDetails,
        };
        use melyxar_core::time::now;
        use std::path::Path;

        let (database, library, _, films) = a_shelf(&[("One", 7.0, "Drame")]).await;
        let root = database
            .library_roots(library)
            .await
            .expect("roots read")[0]
            .id;

        let video = |source_id: MediaSourceId, height: i32, hdr: Option<HdrFormat>| Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: "hevc".to_string(),
                profile: Some("Main 10".to_string()),
                level: None,
                width: height * 16 / 9,
                height,
                margins: None,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: None,
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr,
            }),
        };
        let audio = |source_id: MediaSourceId, channels: i32, profile: Option<&str>| Track {
            id: TrackId::new(),
            source_id,
            stream_index: 1,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "eac3".to_string(),
                profile: profile.map(str::to_string),
                channels,
                channel_layout: None,
                sample_rate: None,
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        };

        // Two copies of the same film, one of them gone from the disk. What
        // the badges say has to come from the copy that can really be played.
        let good = database
            .insert_source(films[0], root, Path::new("one-4k.mkv"), 1_000, now())
            .await
            .expect("copy recorded");
        database
            .store_analysis(
                good,
                &Default::default(),
                &[
                    video(good, 2160, Some(HdrFormat::Hdr10)),
                    audio(good, 8, Some("Dolby Atmos")),
                    audio(good, 2, None),
                ],
                &[],
            )
            .await
            .expect("copy analysed");

        let gone = database
            .insert_source(films[0], root, Path::new("one-8k.mkv"), 1_000, now())
            .await
            .expect("copy recorded");
        database
            .store_analysis(gone, &Default::default(), &[video(gone, 4320, None)], &[])
            .await
            .expect("copy analysed");
        sqlx::query("UPDATE media_sources SET missing_since = ? WHERE id = ?")
            .bind(timestamp_to_text(now()))
            .bind(gone.to_db_string())
            .execute(database.writer())
            .await
            .expect("copy lost");

        let dressed = database
            .dressed_large(&films, "fr")
            .await
            .expect("dressed read");
        let one = dressed.get(&films[0]).expect("the film is dressed");
        assert_eq!(one.genres, vec!["Drame".to_string()]);
        assert_eq!(one.height, Some(2160), "the copy still on disk decides");
        assert_eq!(one.hdr.as_deref(), Some("hdr10"));
        assert_eq!(
            one.sound.as_deref(),
            Some("Dolby Atmos"),
            "the fullest soundtrack, named as the file names it"
        );
    }

    #[tokio::test]
    async fn a_work_already_started_is_never_suggested() {
        let (database, _, who, films) =
            a_shelf(&[("One", 9.0, "Drame"), ("Two", 8.0, "Drame")]).await;

        database
            .record_playback_progress(
                who,
                films[0],
                Millis::new(600_000),
                PlaybackState::InProgress,
                melyxar_core::time::now(),
            )
            .await
            .expect("position recorded");

        let offered = database
            .suggestions(who, None, 10)
            .await
            .expect("suggestions read");
        assert_eq!(
            offered.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[1]],
            "what somebody is in the middle of belongs to carrying on"
        );
    }
}
