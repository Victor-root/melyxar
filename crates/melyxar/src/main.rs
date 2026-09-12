//! The Melyxar server.
//!
//! Reads the configuration, opens everything, and either serves or reports.
//! Two commands rather than one: the diagnostic has to work on a server that
//! will not start, which is exactly when it is most useful.

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use melyxar_config::Config;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "melyxar", version, about = "The Melyxar media server")]
struct Cli {
    /// Configuration file. Holds the real paths and keys, and never lives in
    /// the repository.
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the server. The default when no command is given.
    Serve,
    /// Report on this installation: tools, folders, database, libraries.
    ///
    /// Written for someone who does not read code, and meant to be pasted
    /// straight into a conversation when something is wrong.
    Doctor {
        /// Print as data rather than as text, for a script.
        #[arg(long)]
        json: bool,
    },
    /// Scan the libraries: find what is new, gone or replaced, and analyse it.
    ///
    /// Runs the same work the server runs on its own, and waits for it, so a
    /// first scan can be watched from a terminal. Identification follows on
    /// its own once a provider key is configured.
    Scan {
        /// Scan only this library, by name. Every library by default.
        #[arg(long)]
        library: Option<String>,
        /// Stop after the scan, without looking anything up.
        #[arg(long)]
        without_identification: bool,
    },
    /// Look up the works that are still waiting to be identified.
    ///
    /// Separate from a scan on purpose: it talks to someone else's server, and
    /// can be run again on its own whenever that server is up.
    Identify {
        /// Look up only this library, by name. Every library by default.
        #[arg(long)]
        library: Option<String>,
    },
    /// Print a starting configuration, for the installer.
    PrintDefaultConfig,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Printing a starting configuration must work before anything exists, so
    // it is handled before any file is read.
    if matches!(cli.command, Some(Command::PrintDefaultConfig)) {
        print!("{}", Config::default().to_toml());
        return Ok(());
    }

    let config_path = cli.config.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path)
        .with_context(|| format!("reading the configuration at {}", config_path.display()))?;

    install_logging(&config);

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(config).await,
        Command::Doctor { json } => doctor(config, json).await,
        Command::Scan {
            library,
            without_identification,
        } => scan(config, library, !without_identification).await,
        Command::Identify { library } => identify(config, library).await,
        Command::PrintDefaultConfig => unreachable!("handled above"),
    }
}

/// Sets up logging from the configuration.
///
/// The redaction switch is applied here rather than later, so that a media
/// name cannot slip into the log during start-up.
fn install_logging(config: &Config) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.logging.level.clone()));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                // Every line carries when it happened, which is what makes a
                // pasted log usable.
                .with_timer(tracing_subscriber::fmt::time::uptime()),
        )
        .init();

    melyxar_core::privacy::set_reveal_media_names(config.logging.reveal_media_names);
    if config.logging.reveal_media_names {
        tracing::warn!(
            "media names appear in full in the logs; turn this off once the problem is found"
        );
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let address = SocketAddr::new(config.bind_address, config.port);
    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up")?;

    if !state.can_play_media() {
        tracing::warn!(
            "the media tools are missing or incomplete: browsing works, playback does not. \
             Run the doctor command for details"
        );
    }

    melyxar_server::serve(address, state, shutdown_signal())
        .await
        .context("serving")?;

    tracing::info!("stopped");
    Ok(())
}

async fn doctor(config: Config, as_json: bool) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up for the report")?;
    let report = melyxar_app::diagnostics::collect(&state)
        .await
        .context("collecting the report")?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", melyxar_app::diagnostics::render_text(&report));
    }

    state.database().close().await;
    Ok(())
}

/// The libraries a command was pointed at.
async fn chosen_libraries(
    state: &melyxar_app::AppState,
    only: Option<String>,
) -> anyhow::Result<Vec<melyxar_core::library::Library>> {
    let libraries = state
        .database()
        .list_libraries()
        .await
        .context("reading the libraries")?;

    let chosen: Vec<_> = match &only {
        Some(name) => libraries
            .into_iter()
            .filter(|library| library.name.eq_ignore_ascii_case(name))
            .collect(),
        None => libraries,
    };
    if chosen.is_empty() {
        anyhow::bail!("no library to work on; declare one in the configuration first");
    }
    Ok(chosen)
}

async fn scan(config: Config, only: Option<String>, then_identify: bool) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up for the scan")?;
    let chosen = chosen_libraries(&state, only).await?;

    for library in chosen {
        let name = library.name.clone();
        let job = melyxar_app::scan::start_scan(&state, library)
            .await
            .with_context(|| format!("starting the scan of {name}"))?;
        let (job_state, report) = job.wait().await;

        match report {
            Some(report) => println!(
                "{name}: {} added, {} changed, {} absent, {} back, {} unchanged, {} analysed, \
                 {} unreadable, {} extra videos, {} subtitle files",
                report.added,
                report.changed,
                report.missing,
                report.restored,
                report.unchanged,
                report.analysed,
                report.unreadable_files,
                report.extras,
                report.external_subtitles
            ),
            None => println!("{name}: the scan ended as {}", job_state.as_str()),
        }

        if then_identify {
            identify_one_library(&state, library_again(&state, &name).await?).await?;
        }
    }

    state.database().close().await;
    Ok(())
}

async fn identify(config: Config, only: Option<String>) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up for the identification")?;

    let mut all_ran = true;
    for library in chosen_libraries(&state, only).await? {
        all_ran &= identify_one_library(&state, library).await?;
    }

    state.database().close().await;
    // A run that did not finish has to be visible to whatever started this,
    // not only readable in the lines above.
    anyhow::ensure!(all_ran, "an identification run did not finish");
    Ok(())
}

/// Reads a library again by name, since a scan consumed the one it was given.
async fn library_again(
    state: &melyxar_app::AppState,
    name: &str,
) -> anyhow::Result<melyxar_core::library::Library> {
    state
        .database()
        .library_by_name(name)
        .await
        .context("reading the library")?
        .ok_or_else(|| anyhow::anyhow!("the library {name} is gone"))
}

/// Answers whether the run reached its end.
async fn identify_one_library(
    state: &melyxar_app::AppState,
    library: melyxar_core::library::Library,
) -> anyhow::Result<bool> {
    let Some(provider) = state.metadata_provider() else {
        println!(
            "{}: nothing was looked up, no provider key is configured",
            library.name
        );
        return Ok(true);
    };

    let name = library.name.clone();
    let job = melyxar_app::identify::start_identification(state, provider, library)
        .await
        .with_context(|| format!("starting the identification of {name}"))?;
    let (job_state, report) = job.wait().await;

    match report {
        Some(report) => {
            println!(
                "{name}: {} identified, {} not recognised, {} put off until the provider answers",
                report.identified, report.unidentified, report.postponed
            );
            Ok(true)
        }
        None => {
            println!(
                "{name}: the identification ended as {}, see the log above for why",
                job_state.as_str()
            );
            Ok(false)
        }
    }
}

/// Waits for the signals a service manager sends.
///
/// Stopping cleanly matters more here than in an ordinary server: playback
/// sessions own external processes, and leaving those behind is exactly the
/// failure this project set out to avoid.
async fn shutdown_signal() {
    let interrupt = async {
        tokio::signal::ctrl_c()
            .await
            .expect("the interrupt handler installs");
    };

    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("the termination handler installs")
            .recv()
            .await;
    };

    tokio::select! {
        () = interrupt => tracing::info!("interrupted, stopping"),
        () = terminate => tracing::info!("asked to stop by the service manager"),
    }
}
