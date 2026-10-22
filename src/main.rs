//! Picks which Omarchy screensaver runs next. Runners are executables linked
//! into one directory; each names what it can show with `--list`, and the run
//! log they all append to is the rotation's history.

mod adapters;
mod domain;

use std::{
    env,
    io::{self, Write as _},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Parser, Subcommand};

use crate::{
    adapters::{log, runners},
    domain::{
        entry::Entry,
        history::History,
        rotation::{next, playlist},
    },
};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// The run log runners append to [default: $XDG_STATE_HOME/screensaver-rotation/log]
    #[arg(long, global = true, value_name = "FILE")]
    log: Option<PathBuf>,

    /// Where runners are linked in [default: ~/.local/lib/screensaver-rotation/runners]
    #[arg(long, global = true, value_name = "DIR")]
    runners: Option<PathBuf>,

    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// The rotation, best first, as `runner<TAB>entry` lines
    Pick {
        /// Only this runner's entries
        #[arg(long, value_name = "NAME")]
        runner: Option<String>,
    },
    /// The command line of the screensaver to run next, NUL-separated: its
    /// runner, the window and log options, then its entries in rotation order
    Launch {
        /// Seed for the runner, so every monitor shows the same thing
        #[arg(long, value_name = "N")]
        seed: u64,

        /// Wayland app id of the window
        #[arg(long, value_name = "ID", default_value = "org.omarchy.screensaver")]
        app_id: String,
    },
    /// Every entry with when it was last shown, or "new"
    List,
    /// The name of the last entry shown
    Last,
    /// Where the run log is, for a screensaver that shows things outside a
    /// rotation run (a lock screen, a viewer) and logs them so the rotation
    /// moves past them
    LogPath,
    /// The latest shows, oldest first
    History {
        /// How many
        #[arg(default_value_t = 20)]
        count: usize,
    },
}

/// Why a run ended early.
#[derive(Debug)]
enum Failure {
    /// Whatever read the output stopped reading, as `head` does: not an error.
    Closed,
    Message(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::Message(message.to_owned())
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::BrokenPipe {
            Self::Closed
        } else {
            Self::Message(error.to_string())
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) | Err(Failure::Closed) => ExitCode::SUCCESS,
        Err(Failure::Message(message)) => {
            let _ = writeln!(io::stderr(), "screensaver-rotation: {message}");
            ExitCode::FAILURE
        }
    }
}

fn home(rest: &str) -> PathBuf {
    PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(rest)
}

/// The XDG state directory's `rest`.
fn state(rest: &str) -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| home(".local/state"), PathBuf::from)
        .join(rest)
}

fn run(cli: Cli) -> Result<(), Failure> {
    let log_path = cli.log.unwrap_or_else(|| state("screensaver-rotation/log"));
    let runners_dir = cli
        .runners
        .unwrap_or_else(|| home(".local/lib/screensaver-rotation/runners"));
    let history = || -> Result<History, Failure> {
        let text = log::read(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
        Ok(History::parse(&text))
    };
    let mut out = io::stdout().lock();
    let mut print = |line: &str| writeln!(out, "{line}").map_err(Failure::from);

    match cli.action {
        Action::Pick { runner } => {
            let mut entries = entries(&runners_dir)?;
            if let Some(only) = runner {
                entries.retain(|entry| entry.runner == only);
            }
            for entry in playlist(entries, &history()?, &mut fastrand::Rng::new()) {
                print(&format!("{}\t{}", entry.runner, entry.id))?;
            }
        }
        Action::Launch { seed, app_id } => {
            log::trim(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
            let list = playlist(
                entries(&runners_dir)?,
                &history()?,
                &mut fastrand::Rng::new(),
            );
            let (runner, ids) = next(&list).ok_or("no runner has anything to show")?;
            let mut argv = vec![
                runners_dir.join(runner).display().to_string(),
                "--app-id".to_owned(),
                app_id,
                "--seed".to_owned(),
                seed.to_string(),
                "--log".to_owned(),
                log_path.display().to_string(),
            ];
            argv.extend(ids.into_iter().map(str::to_owned));
            let mut out = io::stdout().lock();
            for arg in argv {
                out.write_all(arg.as_bytes())?;
                out.write_all(b"\0")?;
            }
        }
        Action::List => {
            let history = history()?;
            for entry in entries(&runners_dir)? {
                let when = history
                    .last_shown(entry.name())
                    .map_or("new", |stamp| stamp.as_str());
                print(&format!("{:<24} {:<10} {when}", entry.name(), entry.runner))?;
            }
        }
        Action::LogPath => print(&log_path.display().to_string())?,
        Action::Last => {
            let history = history()?;
            let show = history.shows().last().ok_or("nothing has been shown yet")?;
            print(domain::entry::name_of(&show.id))?;
        }
        Action::History { count } => {
            let history = history()?;
            let shows = history.shows();
            for show in &shows[shows.len().saturating_sub(count)..] {
                print(&format!(
                    "{}  {}  ({})",
                    show.when.as_str(),
                    domain::entry::name_of(&show.id),
                    show.kind
                ))?;
            }
        }
    }
    Ok(())
}

/// Everything the installed runners can show. A runner that fails to list is
/// left out, with a warning, so one broken runner can't stop the rest.
fn entries(dir: &std::path::Path) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for runner in runners::installed(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        match runners::list(&runner) {
            Ok(listed) => entries.extend(listed),
            Err(e) => {
                let _ = writeln!(
                    io::stderr(),
                    "screensaver-rotation: skipping {}: {e}",
                    runner.name
                );
            }
        }
    }
    Ok(entries)
}
