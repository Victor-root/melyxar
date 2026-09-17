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

mod journal;

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
        /// How much to go over: new_and_updated_files, what_is_missing or
        /// everything.
        ///
        /// The middle one by default, which is what a scan does on its own.
        /// `everything` reads every file again, even the ones already
        /// described, and asks about every film again: what the analyser is
        /// asked to read grows, and a collection analysed by an older build
        /// keeps the gaps that build left. It touches no file on disk.
        #[arg(long)]
        mode: Option<String>,
    },
    /// Look up the works that are still waiting to be identified.
    ///
    /// Separate from a scan on purpose: it talks to someone else's server, and
    /// can be run again on its own whenever that server is up.
    Identify {
        /// Look up only this library, by name. Every library by default.
        #[arg(long)]
        library: Option<String>,
        /// How much to go over, as for the scan.
        #[arg(long)]
        mode: Option<String>,
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
            mode,
        } => {
            let mode = chosen_mode(mode.as_deref())?;
            scan(config, library, !without_identification, mode).await
        }
        Command::Identify { library, mode } => {
            let mode = chosen_mode(mode.as_deref())?;
            identify(config, library, mode).await
        }
        Command::PrintDefaultConfig => unreachable!("handled above"),
    }
}

/// Reads the mode a command was given, or the usual one when it was given
/// none.
///
/// A word nobody knows is refused by name rather than read as the usual one:
/// whoever typed a mode meant that mode, and a run that quietly does something
/// else is a run somebody believes has happened.
fn chosen_mode(asked: Option<&str>) -> anyhow::Result<melyxar_core::refresh::RefreshMode> {
    let Some(word) = asked else {
        return Ok(melyxar_core::refresh::RefreshMode::default());
    };
    melyxar_core::refresh::RefreshMode::parse(word).ok_or_else(|| {
        anyhow::anyhow!(
            "'{word}' is not a way of refreshing; the three are {}",
            melyxar_core::refresh::RefreshMode::ALL
                .map(|mode| mode.as_str())
                .join(", ")
        )
    })
}

/// Sets up logging from the configuration.
///
/// The redaction switch is applied here rather than later, so that a media
/// name cannot slip into the log during start-up.
fn install_logging(config: &Config) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.logging.level.clone()));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                // Every line carries when it happened, which is what makes a
                // pasted log usable.
                .with_timer(tracing_subscriber::fmt::time::uptime())
                // The level belongs to the console copy alone. What the screen
                // keeps is decided next, and deliberately not by this.
                .with_filter(filter),
        )
        .with(
            journal::KeepWhatWasSaid.with_filter(tracing_subscriber::filter::filter_fn(
                journal::worth_keeping,
            )),
        )
        .init();

    melyxar_core::privacy::set_reveal_media_names(config.logging.reveal_media_names);
    // The first line of every log says which build wrote it. Without it, a
    // journal and a question about a fix cannot be put together.
    tracing::info!(build = melyxar_core::BUILD, "melyxar starting");

    if config.logging.reveal_media_names {
        tracing::warn!(
            "media names appear in full in the logs; turn this off once the problem is found"
        );
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let address = SocketAddr::new(config.bind_address, config.port);
    let brought_up = melyxar_app::startup::bring_up_and_say_what_is_waiting(config)
        .await
        .context("bringing the server up")?;
    let state = brought_up.state;

    if !state.can_play_media() {
        tracing::warn!(
            "the media tools are missing or incomplete: browsing works, playback does not. \
             Run the doctor command for details"
        );
    }

    // Only the server does these, and only before it serves anything: what
    // they close belongs to a run that is over, and nothing of this run exists
    // yet to be confused with it. A report or a scan asked for from a terminal
    // runs alongside a server that may be busy, and closing its rows from
    // there marks work that is happening right now as finished.
    melyxar_app::playback::tidy_up_after_a_previous_run(&state).await;
    let sweeper = melyxar_app::playback::keep_sessions_swept(&state);

    // The two readings that go through every film belong to the night, unless
    // a library has asked its own scan to do them. Started here because only a
    // server runs long enough to reach an hour of the morning: a report or a
    // scan from a terminal is over in minutes.
    let upkeep = melyxar_app::upkeep::keep_the_upkeep_running(&state);

    // A scan of a whole collection runs for hours, so an update in the middle
    // of one must not mean starting it over by hand, or worse, forgetting to.
    let cut_short = melyxar_app::startup::close_what_a_previous_run_left(&state)
        .await
        .context("closing what a previous run left")?;
    melyxar_app::startup::take_up_again_what_a_restart_cut_short(&state, &cut_short).await;

    // A language changed in the configuration puts every film of that library
    // back in the queue. Asking about them now is what makes the change
    // something somebody watches happen, rather than something they discover
    // months later.
    melyxar_app::startup::ask_again_about(&state, &brought_up.waiting_on_a_new_language).await;

    melyxar_server::serve(address, state.clone(), shutdown_signal())
        .await
        .context("serving")?;

    // The listener has stopped accepting: nothing new can open a session, so
    // closing them all is the last thing left to do.
    sweeper.abort();
    upkeep.abort();
    melyxar_app::playback::close_every_session(&state).await;
    state.database().close().await;

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

async fn scan(
    config: Config,
    only: Option<String>,
    then_identify: bool,
    mode: melyxar_core::refresh::RefreshMode,
) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up for the scan")?;
    let chosen = chosen_libraries(&state, only).await?;

    for library in chosen {
        let name = library.name.clone();
        // Somebody is at a terminal watching this one.
        let job = melyxar_app::scan::start_scan(
            &state,
            library,
            melyxar_core::job::JobPriority::REQUESTED,
            mode,
        )
        .await
        .with_context(|| format!("starting the scan of {name}"))?;
        let (job_state, report) = job.wait().await;

        match report {
            Some(report) => println!(
                "{name}: {} added, {} changed, {} absent, {} back, {} unchanged, {} renamed, \
                 {} found to be copies of a film already here, {} analysed, {} unreadable, \
                 {} extra videos, {} subtitle files",
                report.added,
                report.changed,
                report.missing,
                report.restored,
                report.unchanged,
                report.renamed,
                report.merged,
                report.analysed,
                report.unreadable_files,
                report.extras,
                report.external_subtitles
            ),
            None => println!("{name}: the scan ended as {}", job_state.as_str()),
        }

        if then_identify {
            identify_one_library(&state, library_again(&state, &name).await?, mode).await?;
        }
    }

    state.database().close().await;
    Ok(())
}

async fn identify(
    config: Config,
    only: Option<String>,
    mode: melyxar_core::refresh::RefreshMode,
) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up for the identification")?;

    let mut all_ran = true;
    for library in chosen_libraries(&state, only).await? {
        all_ran &= identify_one_library(&state, library, mode).await?;
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
    mode: melyxar_core::refresh::RefreshMode,
) -> anyhow::Result<bool> {
    let Some(provider) = state.metadata_provider() else {
        println!(
            "{}: nothing was looked up, no provider key is configured",
            library.name
        );
        return Ok(true);
    };

    let name = library.name.clone();
    let job = melyxar_app::identify::start_identification(state, provider, library, mode)
        .await
        .with_context(|| format!("starting the identification of {name}"))?;
    let (job_state, report) = job.wait().await;

    match report {
        Some(report) => {
            println!(
                "{name}: {} identified, {} not recognised, {} put off until the provider \
                 answers, {} renamed after their file, {} found to be copies of a film \
                 already here, {} given the pictures they were missing",
                report.identified,
                report.unidentified,
                report.postponed,
                report.renamed,
                report.merged,
                report.pictures_filled
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
