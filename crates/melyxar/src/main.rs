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

mod bench;
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
    /// Listen to one series again for the titles its episodes share.
    ///
    /// The upkeep does this on its own, once, and never again: a season
    /// written down as done is a season never read again, which is what stops
    /// a collection being read through every night for the same answer. So
    /// when the rules about what an opening is change, this is how one series
    /// is put through the new ones without a whole collection being read
    /// again for hours.
    ///
    /// What was found before is forgotten first. What the file says itself and
    /// what somebody set by hand both stay.
    Listen {
        /// The series, by name or by part of one. Case does not matter.
        #[arg(required_unless_present = "everything")]
        series: Option<String>,
        /// One season of it, by number. Every season by default.
        #[arg(long)]
        season: Option<i32>,
        /// Every season of every series instead of one, which takes hours.
        ///
        /// What to reach for once a rule has been tried on one series and is
        /// worth putting a whole collection through.
        #[arg(long, conflicts_with_all = ["series", "season"])]
        everything: bool,
    },
    /// Measure this server against the budgets, on a library large enough to
    /// mean something.
    ///
    /// Fifty films say nothing about a hundred thousand. This invents the
    /// hundred thousand, plays the pages somebody browsing would ask for, and
    /// puts what it measured next to what was promised.
    Bench {
        #[command(subcommand)]
        what: Bench,
    },
    /// The accounts on this server, from the machine it runs on.
    ///
    /// The way back in for somebody locked out of their own server. It asks
    /// for no password of its own, so it is reachable only here: whoever has a
    /// terminal on this machine can read the database anyway.
    Account {
        #[command(subcommand)]
        what: Account,
    },
    /// Print every folder the libraries look in, one a line, for the
    /// installer to open them for writing when asked to.
    Folders,
    /// Print a starting configuration, for the installer.
    PrintDefaultConfig,
}

#[derive(Subcommand)]
enum Account {
    /// Name every account, and say which of them are administrators.
    List,
    /// Make an account.
    ///
    /// Here rather than on a screen for now: the administration where it
    /// belongs does not exist yet.
    Add {
        /// What to call it.
        name: String,
        /// Let it manage this server: its libraries, its journal, its report.
        #[arg(long)]
        administrator: bool,
    },
    /// Take an account away, with everything of theirs.
    ///
    /// Where they were in every film, what they marked as liked, what they
    /// meant to watch, their preferences and every device they were signed in
    /// on. None of it can be had back.
    Remove {
        name: String,
    },
    /// Say which libraries an account may see.
    ///
    /// Without a library named, it sees every one there is, including the
    /// ones added later. With one or more, it sees those and nothing else.
    Libraries {
        name: String,
        /// A library by its name, as many times as there are to grant.
        #[arg(long = "library", value_name = "NAME")]
        libraries: Vec<String>,
    },
    /// Put a password on an account, and sign every device of it out.
    ///
    /// The password is read from the standard input rather than taken as an
    /// argument, so that it never lands in a shell's history or in the list of
    /// what is running on this machine.
    Password {
        /// Whose. Run `account list` if you are not sure how it is spelt.
        name: String,
    },
}

#[derive(Subcommand)]
enum Bench {
    /// Invent a library of this many works, in a library of its own.
    ///
    /// Writes no file and touches no disk: the works have no pictures and
    /// their files exist as rows. Nothing of a real library is touched, and
    /// the invented one is the only thing `empty` ever removes.
    Fill {
        /// How many works to invent. A series arrives whole, so the count
        /// lands near this rather than on it.
        #[arg(long, default_value_t = 100_000)]
        works: i64,
    },
    /// Take the invented library away, and nothing else.
    Empty,
    /// Say what the invented library holds right now.
    Show,
    /// Play the pages somebody browsing would ask for, and time them.
    ///
    /// Asks a server that is already running, so what is measured is the whole
    /// of an answer. Ends in failure when a budget is missed, so a script can
    /// tell without reading the table.
    Run {
        /// Which account to ask as.
        ///
        /// This server answers nothing to somebody who is not signed in, so a
        /// run signs in the way a browser does. Its password is read from the
        /// standard input, never taken as an argument.
        #[arg(long = "as", value_name = "NAME")]
        account: String,
        /// Where the server is answering. The port from the configuration on
        /// this machine, by default.
        #[arg(long)]
        address: Option<String>,
        /// How many times each page is asked for.
        #[arg(long, default_value_t = 200)]
        rounds: u32,
        /// How many are asked for at the same time. One person browsing is
        /// one, which is what the budgets are written about.
        #[arg(long, default_value_t = 1)]
        at_once: u32,
    },
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

    install_logging(&config, matches!(cli.command, Some(Command::Listen { .. })));

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
        // The flag does its whole work before this: it is what allows the
        // series to be left out, and no name is already every series there is.
        Command::Listen {
            series,
            season,
            everything: _,
        } => listen(config, series.as_deref(), season).await,
        Command::Bench { what } => bench(config, what).await,
        Command::Account { what } => account(config, what).await,
        Command::Folders => folders(config).await,
        Command::PrintDefaultConfig => unreachable!("handled above"),
    }
}

/// The accounts on this server, from a terminal on the machine itself.
async fn folders(config: Config) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config).await?;
    for library in state.database().list_libraries().await? {
        for root in library.roots {
            println!("{}", root.path.display());
        }
    }
    Ok(())
}

async fn account(config: Config, what: Account) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config).await?;

    match what {
        Account::List => {
            let accounts = state.database().list_users().await?;
            if accounts.is_empty() {
                println!(
                    "no account yet: open this server in a browser to set it up"
                );
                return Ok(());
            }
            for account in accounts {
                match account.permissions.is_administrator {
                    true => println!("{} (administrator)", account.name),
                    false => println!("{}", account.name),
                }
            }
        }
        Account::Add {
            name,
            administrator,
        } => {
            let password = read_a_password()?;
            let permissions = match administrator {
                true => melyxar_core::user::Permissions::administrator(),
                false => melyxar_core::user::Permissions::viewer(),
            };
            let made = melyxar_app::accounts::create_account(
                &state,
                &name,
                &password,
                &permissions,
            )
            .await?;
            match administrator {
                true => println!("{} was made, as an administrator", made.name),
                false => println!("{} was made", made.name),
            }
        }
        Account::Remove { name } => {
            anyhow::ensure!(
                confirmed(&name)?,
                "nothing was taken away: what was typed is not {name}"
            );
            match melyxar_app::accounts::remove_account(&state, &name).await? {
                true => println!("{name} is gone, with everything of theirs"),
                false => anyhow::bail!(
                    "no account is called {name}: run `account list` to see the names"
                ),
            }
        }
        Account::Libraries { name, libraries } => {
            let mut granted = Vec::with_capacity(libraries.len());
            for wanted in &libraries {
                let library = state
                    .database()
                    .library_by_name(wanted)
                    .await?
                    .with_context(|| format!("no library is called {wanted}"))?;
                granted.push(library.id);
            }

            let found = melyxar_app::accounts::set_what_an_account_may_see(
                &state,
                &name,
                granted.is_empty(),
                &granted,
            )
            .await?;
            anyhow::ensure!(
                found,
                "no account is called {name}: run `account list` to see the names"
            );
            match libraries.is_empty() {
                true => println!("{name} sees every library, including the ones added later"),
                false => println!("{name} sees only: {}", libraries.join(", ")),
            }
        }
        Account::Password { name } => {
            let password = read_a_password()?;
            match melyxar_app::accounts::set_a_password(&state, &name, &password).await? {
                true => println!(
                    "the password of {name} was changed, and every device of it signed out"
                ),
                false => anyhow::bail!(
                    "no account is called {name}: run `account list` to see the names"
                ),
            }
        }
    }
    Ok(())
}

/// Asks for a name to be typed again before something cannot be had back.
///
/// Read the same way a password is, so it works whether somebody is at the
/// terminal or a script is feeding it.
fn confirmed(name: &str) -> anyhow::Result<bool> {
    use std::io::{BufRead, Write};

    print!("Type {name} again to take it away, with everything of theirs: ");
    std::io::stdout().flush()?;

    let mut typed = String::new();
    std::io::stdin().lock().read_line(&mut typed)?;
    Ok(typed.trim() == name)
}

/// A password off the standard input, typed or piped in.
///
/// Read here rather than taken as an argument, so it never lands in a shell's
/// history or in the list of what is running on this machine. What is typed is
/// shown as it is typed: hiding it would mean speaking to the terminal itself,
/// and this is a command run by one person on their own machine.
fn read_a_password() -> anyhow::Result<String> {
    use std::io::{BufRead, Write};

    print!("Password: ");
    std::io::stdout().flush()?;

    let mut typed = String::new();
    std::io::stdin().lock().read_line(&mut typed)?;
    let typed = typed.trim_end_matches(['\r', '\n']).to_string();
    if typed.is_empty() {
        anyhow::bail!("nothing was typed, so nothing was changed");
    }
    Ok(typed)
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
/// A command run to find something out asks for the detail on its own. What
/// the listening decided about each file is written down as it goes, and a
/// server set to say only what matters keeps none of it: somebody at a
/// terminal asking one series why it has no button would be answered with a
/// single line saying that it does not. `RUST_LOG` still wins where it is set,
/// since somebody who named a level meant that level.
fn install_logging(config: &Config, wants_the_detail: bool) {
    let asked_for = match wants_the_detail {
        true => format!("{},melyxar_app=debug", config.logging.level),
        false => config.logging.level.clone(),
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(asked_for));

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

    // The first line of every log says which build wrote it. Without it, a
    // journal and a question about a fix cannot be put together.
    tracing::info!(build = melyxar_core::BUILD, "melyxar starting");
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
    // Said once here rather than on every page asked for: the server runs
    // without it, but nobody can open a page until the installer puts it back.
    let interface = &state.config().directories.interface;
    if !interface.join("index.html").is_file() {
        tracing::warn!(
            folder = %interface.display(),
            "the interface is not installed in its folder: the pages will not open until it is"
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

    // What the machine spends, for the curves of the administration. Only a
    // server has anybody to show them to.
    let measuring = melyxar_app::measures::keep_measuring(&state);

    // A scan of a whole collection runs for hours, so an update in the middle
    // of one must not mean starting it over by hand, or worse, forgetting to.
    let cut_short = melyxar_app::startup::close_what_a_previous_run_left(&state)
        .await
        .context("closing what a previous run left")?;
    melyxar_app::startup::take_up_again_what_a_restart_cut_short(&state, &cut_short).await;

    melyxar_server::serve(address, state.clone(), shutdown_signal())
        .await
        .context("serving")?;

    // The listener has stopped accepting: nothing new can open a session, so
    // closing them all is the last thing left to do.
    sweeper.abort();
    upkeep.abort();
    measuring.abort();
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

async fn listen(config: Config, series: Option<&str>, season: Option<i32>) -> anyhow::Result<()> {
    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up for the listening")?;

    let seasons = state
        .database()
        .seasons_of_series(series, season)
        .await
        .context("looking for what to listen to")?;
    if seasons.is_empty() {
        state.database().close().await;
        match series {
            Some(series) => anyhow::bail!(
                "no season of a series whose name holds '{series}' is here to be listened to; \
                 the name is matched on part of it, and a season whose files have not been \
                 analysed yet has nothing to listen to"
            ),
            None => anyhow::bail!("there is no season here to listen to"),
        }
    }

    let files: i64 = seasons.iter().map(|season| season.waiting).sum();
    println!(
        "listening again to {} season(s) of {}, {files} file(s) in all",
        seasons.len(),
        match series {
            Some(_) => seasons[0].series.clone(),
            None => "every series here".to_string(),
        }
    );

    let job = melyxar_app::openings::start_listening_again(&state, series, seasons)
        .await
        .context("starting the listening")?;
    let (job_state, went) = job.wait().await;

    for one in &went {
        let name = match one.number {
            Some(number) => format!("{} S{number:02}", one.series),
            None => one.series.clone(),
        };
        println!(
            "{name}: {} episode(s), {} with an opening, {} with closing titles",
            one.episodes, one.with_an_opening, one.with_a_closing
        );
    }
    if went.is_empty() {
        println!("nothing was listened to; the log above says why");
    }

    state.database().close().await;
    // A run that did not reach its end has to be visible to whatever started
    // it, not only readable in the lines above.
    anyhow::ensure!(
        job_state == melyxar_core::job::JobState::Succeeded,
        "the listening ended as {}",
        job_state.as_str()
    );
    Ok(())
}

async fn bench(config: Config, what: Bench) -> anyhow::Result<()> {
    // Measuring never opens the database: the server under measurement is the
    // one that holds it, and a second reader of the same file is one more
    // thing between the question and the answer.
    if let Bench::Run {
        account,
        address,
        rounds,
        at_once,
    } = what
    {
        let password = read_a_password()?;
        let address =
            address.unwrap_or_else(|| format!("http://127.0.0.1:{}", config.port));
        let held = bench::run(bench::Asked {
            address,
            account,
            password,
            rounds,
            at_once,
        })
        .await?;
        anyhow::ensure!(held, "a budget was missed; the table above names which");
        return Ok(());
    }

    let state = melyxar_app::startup::bring_up(config)
        .await
        .context("bringing the server up for the bench")?;

    let outcome = match what {
        Bench::Fill { works } => fill_the_bench(&state, works).await,
        Bench::Empty => {
            let emptied = melyxar_app::bench::empty(&state)
                .await
                .context("emptying the invented library")?;
            match emptied.works {
                0 => println!("there was no invented library to empty"),
                works => println!(
                    "the invented library is gone: {works} work(s), {} file(s) as rows",
                    emptied.files
                ),
            }
            Ok(())
        }
        Bench::Show => {
            match melyxar_app::bench::what_is_there(&state)
                .await
                .context("reading the invented library")?
            {
                Some(held) => println!(
                    "the invented library holds {} work(s) and {} file(s) as rows",
                    held.works, held.files
                ),
                None => println!("there is no invented library here"),
            }
            Ok(())
        }
        Bench::Run { .. } => unreachable!("handled above"),
    };

    state.database().close().await;
    outcome
}

/// Fills the invented library, saying how far along it is as it goes.
///
/// Said out loud because it runs for minutes: a command that prints nothing
/// for four minutes is indistinguishable from a command that has hung, and
/// somebody watching it presses the keys that stop it.
async fn fill_the_bench(state: &melyxar_app::AppState, works: i64) -> anyhow::Result<()> {
    anyhow::ensure!(works > 0, "a library of no works measures nothing");
    println!("inventing about {works} works; this takes a few minutes");

    let filled = melyxar_app::bench::fill(state, works, |written, wanted| {
        println!("  {written} / {wanted}");
    })
    .await
    .context("inventing the library")?;

    println!(
        "invented {} work(s) and {} file(s) as rows in {:.1} s, under the library '{}'",
        filled.works,
        filled.files,
        filled.took.as_secs_f64(),
        melyxar_app::bench::LIBRARY_NAME
    );
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
