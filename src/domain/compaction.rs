//! Cutting the run log down to what is still used, so it doesn't grow forever.

use std::collections::{HashMap, HashSet};

use super::{entry::name_of, history::Show};

/// The log cut down to what the rotation and a reader still use: its latest
/// `recent` lines as they are, and before them only the last show of each
/// entry in `names` (the entries that still exist). Older shows, shows of
/// entries that are gone, and the lines between them are dropped. Every name
/// in `names` keeps its last show, so the rotation's order doesn't change.
#[must_use]
pub fn compact(log: &str, names: &HashSet<&str>, recent: usize) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let cut = lines.len().saturating_sub(recent);
    let mut last: HashMap<String, usize> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(show) = Show::parse(line) {
            last.insert(name_of(&show.id).to_owned(), i);
        }
    }
    let mut kept: Vec<usize> = last
        .iter()
        .filter(|&(name, &i)| i < cut && names.contains(name.as_str()))
        .map(|(_, &i)| i)
        .collect();
    kept.sort_unstable();
    kept.extend(cut..lines.len());
    kept.into_iter().fold(String::new(), |mut out, i| {
        out.push_str(lines[i]);
        out.push('\n');
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::history::History;
    use proptest::prelude::*;
    use std::fmt::Write as _;

    const LOG: &str = "\
2026-10-01 10:00:00 showing /s/a.frag (fragcoord format)
2026-10-01 10:05:00 stopped after 300s: key press
2026-10-02 10:00:00 showing /s/gone.frag (fragcoord format)
2026-10-03 10:00:00 showing /s/b.frag (fragcoord format)
2026-10-04 10:00:00 showing /s/a.frag (lock format)
2026-10-05 10:00:00 showing koi (ascii format)
2026-10-05 10:01:00 stopped after 60s: mouse moved
";

    #[test]
    fn older_lines_keep_only_each_live_entrys_last_show() {
        let names = HashSet::from(["a", "b", "koi"]);
        assert_eq!(
            compact(LOG, &names, 2),
            "\
2026-10-03 10:00:00 showing /s/b.frag (fragcoord format)
2026-10-04 10:00:00 showing /s/a.frag (lock format)
2026-10-05 10:00:00 showing koi (ascii format)
2026-10-05 10:01:00 stopped after 60s: mouse moved
"
        );
    }

    #[test]
    fn a_short_log_is_kept_whole_and_an_empty_one_stays_empty() {
        let names = HashSet::from(["a"]);
        assert_eq!(compact(LOG, &names, 100), LOG);
        assert_eq!(compact("", &names, 0), "");
        assert_eq!(compact(LOG, &HashSet::new(), 0), "", "nothing left to keep");
    }

    proptest! {
        #[test]
        fn compaction_keeps_every_live_entrys_last_show_and_the_recent_tail(
            shows in proptest::collection::vec((0usize..8, any::<bool>()), 0..60),
            live in proptest::collection::vec(any::<bool>(), 8),
            recent in 0usize..20,
        ) {
            let mut log = String::new();
            for (n, (who, stopped)) in shows.iter().enumerate() {
                let (day, hour) = (n / 24 + 1, n % 24);
                writeln!(log, "2026-10-{day:02} {hour:02}:00:00 showing /s/e{who}.frag (x format)").unwrap();
                if *stopped {
                    writeln!(log, "2026-10-{day:02} {hour:02}:30:00 stopped after 9s: key press").unwrap();
                }
            }
            let all: Vec<String> = (0..8).map(|i| format!("e{i}")).collect();
            let names: HashSet<&str> = all
                .iter()
                .zip(&live)
                .filter(|(_, alive)| **alive)
                .map(|(name, _)| name.as_str())
                .collect();
            let compacted = compact(&log, &names, recent);

            let (before, after) = (History::parse(&log), History::parse(&compacted));
            for name in &names {
                prop_assert_eq!(before.last_shown(name), after.last_shown(name));
            }
            let lines: Vec<&str> = log.lines().collect();
            let kept: Vec<&str> = compacted.lines().collect();
            let tail = &lines[lines.len().saturating_sub(recent)..];
            prop_assert!(kept.ends_with(tail));
            // Only lines of the log, in its order.
            let mut rest = lines.iter();
            prop_assert!(kept.iter().all(|line| rest.any(|l| l == line)));
            prop_assert_eq!(compact(&compacted, &names, recent), compacted.clone(), "settled");
        }
    }
}
