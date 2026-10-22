//! The run log on disk: runners append to it, the rotation reads it.

use std::{fs, io, path::Path};

/// Past this size the log is cut back to its recent part.
const MAX_BYTES: u64 = 1 << 20;
/// How many of the latest lines a cut keeps; plenty of history for the
/// rotation, which only needs each entry's last show.
const KEEP_LINES: usize = 5000;

/// The log's text; empty if there is no log yet.
///
/// # Errors
///
/// If the log exists but can't be read.
pub fn read(path: &Path) -> io::Result<String> {
    match fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Cuts the log back to its last [`KEEP_LINES`] lines once it is over
/// [`MAX_BYTES`], replacing it whole so a runner appending meanwhile can't
/// land in a half-written file.
///
/// # Errors
///
/// If the log can't be read or replaced.
pub fn trim(path: &Path) -> io::Result<()> {
    match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_BYTES => {}
        Ok(_) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    }
    let text = read(path)?;
    let lines: Vec<&str> = text.lines().collect();
    let kept = lines[lines.len().saturating_sub(KEEP_LINES)..].join("\n") + "\n";
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, kept)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn a_long_log_keeps_its_latest_lines_and_a_short_one_is_left_alone() {
        let dir =
            std::env::temp_dir().join(format!("screensaver-rotation-log-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("log");
        let line = "2026-10-21 10:00:00 showing /s/x.frag (fragcoord format)";
        let mut long = String::new();
        for i in 0..30_000 {
            writeln!(long, "{line} {i}").unwrap();
        }
        fs::write(&path, &long).unwrap();
        trim(&path).unwrap();
        let kept = read(&path).unwrap();
        assert_eq!(kept.lines().count(), KEEP_LINES);
        assert!(kept.ends_with(" 29999\n"));
        trim(&path).unwrap();
        assert_eq!(read(&path).unwrap(), kept, "under the limit: untouched");
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(read(&path).unwrap(), "", "no log is an empty one");
    }
}
