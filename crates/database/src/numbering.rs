//! Episodes numbered across the whole of their series, and the seasons they
//! are put back into.
//!
//! An anime release says `Series - 29` and never which season that is, while
//! a provider describes the series season by season. The number the file
//! carried stays with its episode, and how many episodes the provider counts
//! in each season stays with the series, so an episode can be placed whenever
//! either one changes.

use std::collections::HashSet;

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::time::now;
use melyxar_core::work::{SeasonLength, WorkKind};
use sqlx::Row;

use crate::catalogue::{child_at, insert_work, merge_within, Placed};
use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// An episode whose file numbered it across its series, and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberedAcross {
    pub id: WorkId,
    /// The number the file carried.
    pub absolute_number: i32,
    pub season: i32,
    pub ordinal: i32,
    pub title: String,
}

/// Where one of those episodes has to go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeMove {
    pub episode: WorkId,
    pub season: i32,
    /// What the season is called if it has to be written down.
    pub season_title: String,
    pub season_sort_title: String,
    pub ordinal: i32,
    /// A new name, for an episode whose name was only ever its number.
    pub renamed: Option<(String, String)>,
}

impl Database {
    /// How many episodes each season of a series holds, as its provider
    /// counted them the last time the series was described.
    pub async fn season_lengths(&self, series_id: WorkId) -> Result<Vec<SeasonLength>> {
        sqlx::query(
            "SELECT season, episodes FROM season_lengths WHERE series_id = ? ORDER BY season",
        )
        .bind(series_id.to_db_string())
        .fetch_all(self.reader())
        .await?
        .iter()
        .map(|row| {
            Ok(SeasonLength {
                season: row.try_get("season")?,
                episodes: row.try_get("episodes")?,
            })
        })
        .collect()
    }

    /// Replaces what a series was counted as holding.
    pub async fn set_season_lengths(
        &self,
        series_id: WorkId,
        lengths: &[SeasonLength],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query("DELETE FROM season_lengths WHERE series_id = ?")
            .bind(series_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        for length in lengths {
            sqlx::query(
                "INSERT OR REPLACE INTO season_lengths (series_id, season, episodes)
                 VALUES (?, ?, ?)",
            )
            .bind(series_id.to_db_string())
            .bind(length.season)
            .bind(length.episodes)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Every episode of a series whose file numbered it across the series.
    pub async fn episodes_numbered_across(&self, series_id: WorkId) -> Result<Vec<NumberedAcross>> {
        sqlx::query(
            "SELECT episode.id, episode.absolute_number, season.ordinal AS season,
                    episode.ordinal, episode.title
             FROM works season
             JOIN works episode ON episode.parent_id = season.id
             WHERE season.parent_id = ? AND episode.absolute_number IS NOT NULL
               AND season.ordinal IS NOT NULL AND episode.ordinal IS NOT NULL
             ORDER BY episode.absolute_number",
        )
        .bind(series_id.to_db_string())
        .fetch_all(self.reader())
        .await?
        .iter()
        .map(|row| {
            Ok(NumberedAcross {
                id: parse_id(&row.try_get::<String, _>("id")?)?,
                absolute_number: row.try_get("absolute_number")?,
                season: row.try_get("season")?,
                ordinal: row.try_get("ordinal")?,
                title: row.try_get("title")?,
            })
        })
        .collect()
    }

    /// Puts episodes of a series where they belong, in one go.
    ///
    /// Every episode is moved before anything is compared, because two of
    /// them can be swapping places: looked at one by one, the first would
    /// find the second still in its way and be joined to it. Once all are
    /// where they go, an episode that lands where another already stood is
    /// the same episode twice, one file named across the series and one
    /// named inside its season, and the two become one with both files.
    ///
    /// A season left with nothing in it goes, since a season that holds no
    /// episode is only a heading over nothing.
    ///
    /// The paths of the pictures that went with what was dropped come back,
    /// for the caller to take out of the cache.
    pub async fn place_episodes(
        &self,
        library_id: LibraryId,
        series_id: WorkId,
        moves: &[EpisodeMove],
    ) -> Result<Vec<String>> {
        if moves.is_empty() {
            return Ok(Vec::new());
        }
        let mut transaction = self.begin().await?;
        let moment = timestamp_to_text(now());

        for placed in moves {
            let season = match child_at(&mut *transaction, series_id, placed.season).await? {
                Some(season) => season,
                None => {
                    insert_work(
                        &mut *transaction,
                        Placed::under(library_id, series_id, placed.season),
                        WorkKind::Season,
                        &placed.season_title,
                        &placed.season_sort_title,
                        None,
                    )
                    .await?
                    .id
                }
            };
            let (title, sort_title) = placed.renamed.clone().unzip();
            sqlx::query(
                "UPDATE works SET parent_id = ?, ordinal = ?,
                        title = coalesce(?, title), sort_title = coalesce(?, sort_title),
                        updated_at = ?
                 WHERE id = ?",
            )
            .bind(season.to_db_string())
            .bind(placed.ordinal)
            .bind(title)
            .bind(sort_title)
            .bind(&moment)
            .bind(placed.episode.to_db_string())
            .execute(&mut *transaction)
            .await?;
        }

        let mut no_longer_used = Vec::new();
        let mut gone = HashSet::new();
        for placed in moves {
            if gone.contains(&placed.episode) {
                continue;
            }
            let met = sqlx::query(
                "SELECT other.id FROM works episode
                 JOIN works other ON other.parent_id = episode.parent_id
                                 AND other.ordinal = episode.ordinal
                                 AND other.id <> episode.id
                 WHERE episode.id = ?
                 LIMIT 1",
            )
            .bind(placed.episode.to_db_string())
            .fetch_optional(&mut *transaction)
            .await?;
            let Some(met) = met else {
                continue;
            };
            let met: WorkId = parse_id(&met.try_get::<String, _>("id")?)?;
            no_longer_used.extend(merge_within(&mut transaction, placed.episode, met).await?);
            gone.insert(placed.episode);
        }

        let emptied: Vec<WorkId> = sqlx::query(
            "SELECT id FROM works season
             WHERE season.parent_id = ?
               AND NOT EXISTS (SELECT 1 FROM works child WHERE child.parent_id = season.id)",
        )
        .bind(series_id.to_db_string())
        .fetch_all(&mut *transaction)
        .await?
        .iter()
        .map(|row| parse_id(&row.try_get::<String, _>("id")?))
        .collect::<Result<_>>()?;
        for season in emptied {
            let pictures: Vec<String> = sqlx::query(
                "SELECT relative_path FROM images WHERE owner_kind = 'work' AND owner_id = ?",
            )
            .bind(season.to_db_string())
            .fetch_all(&mut *transaction)
            .await?
            .iter()
            .map(|row| row.try_get::<String, _>("relative_path"))
            .collect::<std::result::Result<_, _>>()?;
            no_longer_used.extend(pictures);
            // Pictures are found by owner rather than by a key the engine
            // knows about, so dropping the season does not drop them.
            sqlx::query("DELETE FROM images WHERE owner_kind = 'work' AND owner_id = ?")
                .bind(season.to_db_string())
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM works WHERE id = ?")
                .bind(season.to_db_string())
                .execute(&mut *transaction)
                .await?;
        }

        sqlx::query(
            "UPDATE works
                SET child_count = (SELECT count(*) FROM works AS child WHERE child.parent_id = works.id)
              WHERE id = ? OR parent_id = ?",
        )
        .bind(series_id.to_db_string())
        .bind(series_id.to_db_string())
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(no_longer_used)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use melyxar_core::id::LibraryRootId;
    use melyxar_core::library::LibraryKind;

    use super::*;

    async fn library() -> (Database, LibraryId, LibraryRootId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Anime",
                LibraryKind::Anime,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Anime"))],
            )
            .await
            .expect("library created");
        let root_id = library.roots[0].id;
        (database, library.id, root_id)
    }

    /// A series whose first season holds these episodes, each numbered across
    /// the series and each with a file behind it.
    async fn numbered_across(
        database: &Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        numbers: &[i32],
    ) -> (WorkId, WorkId) {
        let series = database
            .create_work(
                library_id,
                WorkKind::Series,
                "Amber Field",
                "amber field",
                None,
            )
            .await
            .expect("series written");
        let season = database
            .create_child_work(
                library_id,
                series.id,
                1,
                WorkKind::Season,
                "Season 1",
                "season 1",
            )
            .await
            .expect("season written");
        for number in numbers {
            let episode = database
                .create_episode_numbered_across(
                    library_id,
                    season.id,
                    *number,
                    *number,
                    &format!("Episode {number}"),
                    &format!("episode {number}"),
                )
                .await
                .expect("episode written");
            database
                .insert_source(
                    episode.id,
                    root_id,
                    &PathBuf::from(format!("Amber Field - {number}.mkv")),
                    12,
                    now(),
                )
                .await
                .expect("file written down");
        }
        (series.id, season.id)
    }

    fn a_move(episode: WorkId, season: i32, ordinal: i32) -> EpisodeMove {
        EpisodeMove {
            episode,
            season,
            season_title: format!("Season {season}"),
            season_sort_title: format!("season {season}"),
            ordinal,
            renamed: Some((format!("Episode {ordinal}"), format!("episode {ordinal}"))),
        }
    }

    #[tokio::test]
    async fn what_a_series_was_counted_as_holding_is_kept_and_replaced() {
        let (database, library_id, root_id) = library().await;
        let (series, _) = numbered_across(&database, library_id, root_id, &[]).await;
        let counted = |pairs: &[(i32, i32)]| -> Vec<SeasonLength> {
            pairs
                .iter()
                .map(|&(season, episodes)| SeasonLength { season, episodes })
                .collect()
        };

        database
            .set_season_lengths(series, &counted(&[(1, 28), (2, 12)]))
            .await
            .expect("written");
        database
            .set_season_lengths(series, &counted(&[(1, 12), (2, 16)]))
            .await
            .expect("written again");

        assert_eq!(
            database.season_lengths(series).await.expect("read"),
            counted(&[(1, 12), (2, 16)])
        );
    }

    #[tokio::test]
    async fn episodes_numbered_across_are_found_with_where_they_sit() {
        let (database, library_id, root_id) = library().await;
        let (series, season) = numbered_across(&database, library_id, root_id, &[2, 1]).await;
        // One named inside its season is not one of them.
        database
            .create_child_work(
                library_id,
                season,
                3,
                WorkKind::Episode,
                "Episode 3",
                "episode 3",
            )
            .await
            .expect("episode written");

        let found = database
            .episodes_numbered_across(series)
            .await
            .expect("read");
        assert_eq!(
            found
                .iter()
                .map(|episode| (episode.absolute_number, episode.season, episode.ordinal))
                .collect::<Vec<_>>(),
            [(1, 1, 1), (2, 1, 2)]
        );
    }

    #[tokio::test]
    async fn an_episode_moves_to_its_season_which_is_written_down_if_missing() {
        let (database, library_id, root_id) = library().await;
        let (series, season_one) =
            numbered_across(&database, library_id, root_id, &[1, 2, 3]).await;
        let found = database
            .episodes_numbered_across(series)
            .await
            .expect("read");

        database
            .place_episodes(library_id, series, &[a_move(found[2].id, 2, 1)])
            .await
            .expect("placed");

        let season_two = database
            .child_by_ordinal(series, 2)
            .await
            .expect("read")
            .expect("the second season is written down");
        let moved = database
            .child_by_ordinal(season_two.id, 1)
            .await
            .expect("read")
            .expect("the episode sits there");
        assert_eq!(moved.id, found[2].id);
        assert_eq!(moved.title, "Episode 1");
        let one = database
            .work(season_one)
            .await
            .expect("read")
            .expect("kept");
        let two = database
            .work(season_two.id)
            .await
            .expect("read")
            .expect("kept");
        let series_row = database.work(series).await.expect("read").expect("kept");
        assert_eq!(
            (
                database.children_ranked(one.id).await.expect("read").len(),
                database.children_ranked(two.id).await.expect("read").len(),
                database
                    .children_ranked(series_row.id)
                    .await
                    .expect("read")
                    .len(),
            ),
            (2, 1, 2)
        );
    }

    #[tokio::test]
    async fn two_episodes_swapping_places_are_not_taken_for_one() {
        let (database, library_id, root_id) = library().await;
        let (series, _) = numbered_across(&database, library_id, root_id, &[1, 2]).await;
        let found = database
            .episodes_numbered_across(series)
            .await
            .expect("read");

        database
            .place_episodes(
                library_id,
                series,
                &[a_move(found[0].id, 1, 2), a_move(found[1].id, 1, 1)],
            )
            .await
            .expect("placed");

        assert!(database.work(found[0].id).await.expect("read").is_some());
        assert!(database.work(found[1].id).await.expect("read").is_some());
    }

    #[tokio::test]
    async fn an_episode_landing_on_the_same_episode_joins_it_with_its_file() {
        let (database, library_id, root_id) = library().await;
        let (series, season_one) = numbered_across(&database, library_id, root_id, &[13]).await;
        let season_two = database
            .create_child_work(
                library_id,
                series,
                2,
                WorkKind::Season,
                "Season 2",
                "season 2",
            )
            .await
            .expect("season written");
        let named_inside = database
            .create_child_work(
                library_id,
                season_two.id,
                1,
                WorkKind::Episode,
                "Episode 1",
                "episode 1",
            )
            .await
            .expect("episode written");
        let found = database
            .episodes_numbered_across(series)
            .await
            .expect("read");

        database
            .place_episodes(library_id, series, &[a_move(found[0].id, 2, 1)])
            .await
            .expect("placed");

        assert!(database.work(found[0].id).await.expect("read").is_none());
        assert_eq!(
            database
                .sources_of_work(named_inside.id)
                .await
                .expect("read")
                .len(),
            1,
            "the file comes along"
        );
        assert!(
            database.work(season_one).await.expect("read").is_none(),
            "a season left with nothing in it goes"
        );
    }
}
