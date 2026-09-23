//! What every use case needs to hand.
//!
//! Shared by reference and cheap to clone, so a request handler carries a copy
//! rather than reaching for a global.

use std::sync::Arc;

use melyxar_config::Config;
use melyxar_database::Database;
use melyxar_ffmpeg::{Capabilities, ToolPaths};
use melyxar_jobs::JobRunner;
use melyxar_metadata::TmdbProvider;
use melyxar_streaming::registry::Sessions;

/// The server, assembled.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    config: Config,
    database: Database,
    /// The one place background work is started from, so that what is running
    /// can be listed, followed and stopped.
    jobs: JobRunner,
    /// Where the media tools are, when they were found.
    ///
    /// Optional on purpose: the server has to start and say what is wrong when
    /// they are missing, rather than refusing to come up at all. A library
    /// still browses without them; only playback needs them.
    tools: Option<ToolPaths>,
    capabilities: Option<Capabilities>,
    /// The metadata provider, built once and kept, so the connection to it
    /// stays open instead of being made again for every film. Absent when no
    /// key was configured: the library still scans and browses without one.
    provider: Option<Arc<TmdbProvider>>,
    /// The films being watched right now. Absent when the media tools are, for
    /// the same reason: without them nothing can be converted, and a session
    /// that could never produce a segment is worse than none.
    sessions: Option<Arc<Sessions>>,
    /// What each library has been counted for, so a page never counts a
    /// hundred thousand works to draw a menu.
    counts: crate::counted::Counts,
    /// Wrong passwords, so that guessing one costs time rather than a
    /// processor. Held in memory: a server that has just restarted has
    /// forgotten them, which is the right way round for somebody who locked
    /// themselves out and rebooted it.
    wrong_answers: crate::accounts::WrongAnswers,
    /// When this server came up, for how long it has been running.
    started_at: melyxar_core::time::Timestamp,
    /// What the machine spent lately, read on a steady beat.
    measuring: crate::measures::Measuring,
    /// What is being watched right now, device by device.
    watching: crate::watching::Watching,
}

impl AppState {
    pub fn new(
        config: Config,
        database: Database,
        tools: Option<ToolPaths>,
        capabilities: Option<Capabilities>,
    ) -> Self {
        let provider = melyxar_metadata::defaults::provider_key().and_then(|key| {
            match TmdbProvider::new(key) {
                Ok(provider) => Some(Arc::new(provider)),
                Err(error) => {
                    tracing::error!(error = %error, "the metadata provider could not be prepared");
                    None
                }
            }
        });

        let sessions = tools.clone().map(|tools| {
            Arc::new(Sessions::new(
                config.directories.transcodes.clone(),
                tools,
                config.limits.max_transcoding_sessions,
            ))
        });

        Self {
            inner: Arc::new(Inner {
                jobs: JobRunner::new(database.clone()).telling({
                    // Every task over is a line of the activity journal.
                    let database = database.clone();
                    move |finished| {
                        let database = database.clone();
                        tokio::spawn(async move {
                            let ended = crate::activity::TaskEnded {
                                kind: finished.kind,
                                state: finished.state,
                                reason: finished.reason,
                                target: finished.target_id,
                                took: finished.took,
                            };
                            crate::activity::write(&database, crate::activity::Event::TaskEnded(ended))
                                .await;
                        });
                    }
                }),
                provider,
                sessions,
                config,
                database,
                tools,
                capabilities,
                counts: crate::counted::Counts::default(),
                wrong_answers: crate::accounts::WrongAnswers::default(),
                started_at: melyxar_core::time::now(),
                measuring: crate::measures::Measuring::new(),
                watching: crate::watching::Watching::default(),
            }),
        }
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn database(&self) -> &Database {
        &self.inner.database
    }

    pub fn started_at(&self) -> melyxar_core::time::Timestamp {
        self.inner.started_at
    }

    pub(crate) fn measuring(&self) -> &crate::measures::Measuring {
        &self.inner.measuring
    }

    pub(crate) fn watching(&self) -> &crate::watching::Watching {
        &self.inner.watching
    }

    pub fn jobs(&self) -> &JobRunner {
        &self.inner.jobs
    }

    /// What each library has been counted for.
    pub(crate) fn wrong_answers(&self) -> &crate::accounts::WrongAnswers {
        &self.inner.wrong_answers
    }

    pub(crate) fn counts(&self) -> &crate::counted::Counts {
        &self.inner.counts
    }

    /// The metadata provider, when there is one to use.
    pub fn metadata_provider(&self) -> Option<Arc<TmdbProvider>> {
        self.inner.provider.clone()
    }

    pub fn tools(&self) -> Option<&ToolPaths> {
        self.inner.tools.as_ref()
    }

    pub fn capabilities(&self) -> Option<&Capabilities> {
        self.inner.capabilities.as_ref()
    }

    /// Whether playback can work at all.
    pub fn can_play_media(&self) -> bool {
        self.inner
            .capabilities
            .as_ref()
            .is_some_and(Capabilities::supports_minimum_targets)
    }

    /// Hardware paths the media tool was built with, by name.
    ///
    /// Exposed as plain names so the HTTP layer can answer without knowing
    /// anything about the media tool crate, which it has no business
    /// depending on.
    pub fn hardware_acceleration_names(&self) -> Vec<String> {
        self.inner
            .capabilities
            .as_ref()
            .map(|capabilities| {
                capabilities
                    .hardware
                    .iter()
                    .map(|value| value.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The films being watched right now, when this server can convert at all.
    pub fn sessions(&self) -> Option<&Arc<Sessions>> {
        self.inner.sessions.as_ref()
    }

    /// Whether wide gamut colour can be converted without a card.
    ///
    /// Without it such films cannot be shown with correct colours at all, so
    /// it is worth telling a client about.
    pub fn can_convert_wide_gamut(&self) -> bool {
        self.inner
            .capabilities
            .as_ref()
            .is_some_and(Capabilities::can_tone_map_in_software)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_ffmpeg::capabilities::HardwareAcceleration;
    use std::collections::BTreeSet;

    fn capabilities(
        encoders: &[&str],
        filters: &[&str],
        hardware: &[HardwareAcceleration],
    ) -> Capabilities {
        Capabilities {
            version: "ffmpeg version invented".to_string(),
            encoders: encoders.iter().map(|value| value.to_string()).collect(),
            decoders: BTreeSet::new(),
            filters: filters.iter().map(|value| value.to_string()).collect(),
            hardware: hardware.iter().copied().collect(),
            card_search: Default::default(),
        }
    }

    async fn state_with(capabilities: Option<Capabilities>) -> AppState {
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        AppState::new(Config::default(), database, None, capabilities)
    }

    #[tokio::test]
    async fn a_server_without_the_media_tools_says_playback_is_out_rather_than_failing_later() {
        let state = state_with(None).await;
        assert!(
            !state.can_play_media(),
            "a library still browses, and a client is told before it presses play"
        );
        assert!(!state.can_convert_wide_gamut());
        assert!(state.hardware_acceleration_names().is_empty());
    }

    #[tokio::test]
    async fn a_tool_that_cannot_build_what_a_browser_plays_is_not_good_enough() {
        // The two the browser target rests on. Without them, what would be
        // built is something no browser opens.
        let state = state_with(Some(capabilities(&["libx264"], &[], &[]))).await;
        assert!(!state.can_play_media(), "a picture without a soundtrack");

        let state = state_with(Some(capabilities(&["aac"], &[], &[]))).await;
        assert!(!state.can_play_media(), "a soundtrack without a picture");

        let state = state_with(Some(capabilities(&["libx264", "aac"], &[], &[]))).await;
        assert!(state.can_play_media());
    }

    #[tokio::test]
    async fn wide_gamut_conversion_is_announced_only_when_a_filter_can_do_it() {
        let without = state_with(Some(capabilities(&["libx264", "aac"], &[], &[]))).await;
        assert!(
            !without.can_convert_wide_gamut(),
            "without it such films cannot be shown with correct colours at all"
        );

        let with = state_with(Some(capabilities(&["libx264", "aac"], &["zscale"], &[]))).await;
        assert!(with.can_convert_wide_gamut());
    }

    #[tokio::test]
    async fn the_hardware_paths_are_named_the_way_a_client_reads_them() {
        let state = state_with(Some(capabilities(
            &["libx264", "aac"],
            &[],
            &[HardwareAcceleration::Vaapi, HardwareAcceleration::QuickSync],
        )))
        .await;

        let mut names = state.hardware_acceleration_names();
        names.sort();
        assert_eq!(names, vec!["qsv".to_string(), "vaapi".to_string()]);
    }
}
