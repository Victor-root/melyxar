//! What every use case needs to hand.
//!
//! Shared by reference and cheap to clone, so a request handler carries a copy
//! rather than reaching for a global.

use std::sync::Arc;

use melyxar_config::Config;
use melyxar_database::Database;
use melyxar_ffmpeg::{Capabilities, ToolPaths};

/// The server, assembled.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    config: Config,
    database: Database,
    /// Where the media tools are, when they were found.
    ///
    /// Optional on purpose: the server has to start and say what is wrong when
    /// they are missing, rather than refusing to come up at all. A library
    /// still browses without them; only playback needs them.
    tools: Option<ToolPaths>,
    capabilities: Option<Capabilities>,
}

impl AppState {
    pub fn new(
        config: Config,
        database: Database,
        tools: Option<ToolPaths>,
        capabilities: Option<Capabilities>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                database,
                tools,
                capabilities,
            }),
        }
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn database(&self) -> &Database {
        &self.inner.database
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
