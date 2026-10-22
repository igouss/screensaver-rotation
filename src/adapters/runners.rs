//! The installed runners: executables linked into one directory, each of
//! which names what it can show when run with `--list`.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::domain::entry::Entry;

/// How long a runner gets to list its entries before it is skipped, so a
/// hung runner can't keep the screensaver from starting.
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

/// A runner: its name in the rotation (the link's file name) and its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runner {
    pub name: String,
    pub path: PathBuf,
}

/// The runners in `dir`, by name. A missing directory has none.
///
/// # Errors
///
/// If `dir` exists but can't be read.
pub fn installed(dir: &Path) -> io::Result<Vec<Runner>> {
    let read = match fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut runners = Vec::new();
    for item in read {
        let item = item?;
        let Some(name) = item.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        runners.push(Runner {
            name,
            path: item.path(),
        });
    }
    runners.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(runners)
}

/// What `runner` lists, one entry per non-empty line of its `--list`.
///
/// # Errors
///
/// If the runner can't start, fails, or takes longer than [`LIST_TIMEOUT`].
pub fn list(runner: &Runner) -> io::Result<Vec<Entry>> {
    let mut child = Command::new(&runner.path)
        .arg("--list")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + LIST_TIMEOUT;
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("--list took over {}s", LIST_TIMEOUT.as_secs()),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "--list failed: {}",
            output.status
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "--list printed non-UTF-8"))?;
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|id| Entry {
            runner: runner.name.clone(),
            id: id.to_owned(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    /// A fresh directory of runner scripts, removed when dropped.
    struct Dir(PathBuf);

    impl Dir {
        fn new(test: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "screensaver-rotation-{test}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn script(&self, name: &str, body: &str) {
            let path = self.0.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn runners_are_found_by_name_and_list_their_entries() {
        let dir = Dir::new("list");
        dir.script("shaders", "printf '/s/a.frag\\n\\n/s/b.frag\\n'");
        dir.script("aquarium", "echo sharks; echo koi");
        dir.script(".hidden", "echo nope");
        let runners = installed(&dir.0).unwrap();
        let names: Vec<&str> = runners.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["aquarium", "shaders"]);
        let ids: Vec<String> = runners
            .iter()
            .flat_map(|r| list(r).unwrap())
            .map(|e| format!("{}:{}", e.runner, e.id))
            .collect();
        assert_eq!(
            ids,
            [
                "aquarium:sharks",
                "aquarium:koi",
                "shaders:/s/a.frag",
                "shaders:/s/b.frag"
            ]
        );
    }

    #[test]
    fn a_failing_runner_is_an_error() {
        let dir = Dir::new("fail");
        dir.script("broken", "exit 3");
        let runner = &installed(&dir.0).unwrap()[0];
        assert!(list(runner).is_err());
    }

    #[test]
    fn no_directory_means_no_runners() {
        let missing = std::env::temp_dir().join("screensaver-rotation-no-such-dir");
        assert_eq!(installed(&missing).unwrap(), Vec::new());
    }
}
