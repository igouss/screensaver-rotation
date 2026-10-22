//! The run log on disk: runners append to it, the rotation reads it.

use std::{collections::HashSet, fs, io, path::Path};

use crate::domain::compaction;

/// Past this size the log is compacted. Each show is a line of about 100
/// bytes, so this is a few thousand shows, months of idle periods.
const MAX_BYTES: u64 = 256 << 10;
/// How many of the latest lines a compaction keeps as they are, for
/// `history` and for reading what happened lately.
const RECENT_LINES: usize = 1000;

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

/// Compacts the log once it is over [`MAX_BYTES`]: the latest
/// [`RECENT_LINES`] stay, and before them only the last show of each of the
/// entries in `names` (the ones that still exist). The log is replaced whole,
/// so a runner appending meanwhile can't land in a half-written file.
///
/// # Errors
///
/// If the log can't be read or replaced.
pub fn compact(path: &Path, names: &HashSet<&str>) -> io::Result<()> {
    match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_BYTES => {}
        Ok(_) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    }
    let kept = compaction::compact(&read(path)?, names, RECENT_LINES);
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, kept)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn a_big_log_is_compacted_and_a_small_one_left_alone() {
        let dir =
            std::env::temp_dir().join(format!("screensaver-rotation-log-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("log");
        let mut long = String::new();
        writeln!(
            long,
            "2026-01-01 00:00:00 showing /s/old.frag (fragcoord format)"
        )
        .unwrap();
        for i in 0..5000 {
            let line = format!("2026-10-21 10:00:00 showing /s/x{i}.frag (fragcoord format)");
            writeln!(long, "{line}").unwrap();
        }
        fs::write(&path, &long).unwrap();
        compact(&path, &HashSet::from(["old", "x0"])).unwrap();
        let kept = read(&path).unwrap();
        assert_eq!(kept.lines().count(), 2 + RECENT_LINES);
        assert!(kept.starts_with("2026-01-01 00:00:00 showing /s/old.frag"));
        assert!(kept.ends_with("/s/x4999.frag (fragcoord format)\n"));
        compact(&path, &HashSet::new()).unwrap();
        assert_eq!(read(&path).unwrap(), kept, "under the limit: untouched");
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(read(&path).unwrap(), "", "no log is an empty one");
    }
}
