//! The order screensavers come up in, and what to run for the next one.

use super::entry::Entry;
use super::history::{History, Stamp};

/// Every entry, best first: the ones never shown (shuffled), then the half of
/// the rest shown longest ago (shuffled), then the others oldest first, as
/// fallbacks. An entry can't come back until about half the others have
/// played.
#[must_use]
pub fn playlist(entries: Vec<Entry>, history: &History, rng: &mut fastrand::Rng) -> Vec<Entry> {
    let (mut unseen, mut seen): (Vec<Entry>, Vec<(Stamp, Entry)>) = (Vec::new(), Vec::new());
    for entry in entries {
        match history.last_shown(entry.name()) {
            Some(when) => seen.push((when.clone(), entry)),
            None => unseen.push(entry),
        }
    }
    rng.shuffle(&mut unseen);
    seen.sort_by(|a, b| (&a.0, &a.1.id).cmp(&(&b.0, &b.1.id)));
    let mut seen: Vec<Entry> = seen.into_iter().map(|(_, entry)| entry).collect();
    let half = seen.len().div_ceil(2);
    rng.shuffle(&mut seen[..half]);
    unseen.extend(seen);
    unseen
}

/// What runs next: the runner of the first entry in `playlist`, and that
/// runner's entries in playlist order, so a runner that can't show the first
/// one falls back to its next.
#[must_use]
pub fn next(playlist: &[Entry]) -> Option<(&str, Vec<&str>)> {
    let runner = playlist.first()?.runner.as_str();
    let ids = playlist
        .iter()
        .filter(|entry| entry.runner == runner)
        .map(|entry| entry.id.as_str())
        .collect();
    Some((runner, ids))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fmt::Write as _;

    fn entry(runner: &str, id: &str) -> Entry {
        Entry {
            runner: runner.to_owned(),
            id: id.to_owned(),
        }
    }

    fn names(list: &[Entry]) -> Vec<&str> {
        list.iter().map(Entry::name).collect()
    }

    /// a, b, c, d, e shown in that order; f and g never.
    fn history() -> History {
        History::parse(
            "\
2026-10-01 10:00:00 showing /s/a.frag (fragcoord format)
2026-10-02 10:00:00 showing /s/b.frag (fragcoord format)
2026-10-03 10:00:00 showing /s/c.frag (fragcoord format)
2026-10-04 10:00:00 showing /s/d.frag (fragcoord format)
2026-10-05 10:00:00 showing e (ascii format)
",
        )
    }

    fn entries() -> Vec<Entry> {
        vec![
            entry("shaders", "/s/e.frag"),
            entry("shaders", "/s/d.frag"),
            entry("aquarium", "f"),
            entry("shaders", "/s/c.frag"),
            entry("shaders", "/s/b.frag"),
            entry("aquarium", "g"),
            entry("shaders", "/s/a.frag"),
        ]
    }

    #[test]
    fn never_shown_first_then_the_older_half_then_the_rest_oldest_first() {
        let list = playlist(entries(), &history(), &mut fastrand::Rng::with_seed(1));
        let names = names(&list);
        let mut unseen = names[..2].to_vec();
        unseen.sort_unstable();
        assert_eq!(unseen, ["f", "g"]);
        let mut older = names[2..5].to_vec();
        older.sort_unstable();
        assert_eq!(older, ["a", "b", "c"], "the oldest half, rounded up");
        assert_eq!(names[5..], ["d", "e"], "fallbacks, oldest first");
    }

    #[test]
    fn showing_an_entry_sends_it_to_the_back() {
        let mut log = String::from("2026-10-01 10:00:00 showing /s/a.frag (fragcoord format)\n");
        log.push_str("2026-10-02 10:00:00 showing /s/b.frag (fragcoord format)\n");
        log.push_str("2026-10-03 10:00:00 showing /s/a.frag (lock format)\n");
        let two = vec![entry("shaders", "/s/a.frag"), entry("shaders", "/s/b.frag")];
        let list = playlist(two, &History::parse(&log), &mut fastrand::Rng::with_seed(1));
        assert_eq!(names(&list), ["b", "a"]);
    }

    #[test]
    fn nothing_to_show_runs_nothing() {
        assert_eq!(
            playlist(Vec::new(), &history(), &mut fastrand::Rng::new()),
            Vec::new()
        );
        assert!(next(&[]).is_none());
    }

    #[test]
    fn the_next_runner_gets_its_own_entries_in_order() {
        let list = vec![
            entry("aquarium", "koi"),
            entry("shaders", "/s/a.frag"),
            entry("aquarium", "sharks"),
        ];
        assert_eq!(next(&list), Some(("aquarium", vec!["koi", "sharks"])));
        assert_eq!(next(&list[1..]), Some(("shaders", vec!["/s/a.frag"])));
    }

    proptest! {
        #[test]
        fn the_playlist_is_a_reordering_with_unseen_entries_first(
            shown in proptest::collection::vec(any::<bool>(), 0..40),
            seed in any::<u64>(),
        ) {
            let entries: Vec<Entry> = (0..shown.len())
                .map(|i| entry("shaders", &format!("/s/e{i}.frag")))
                .collect();
            let mut log = String::new();
            for (i, _) in shown.iter().enumerate().filter(|(_, was)| **was) {
                writeln!(log, "2026-10-01 10:00:{:02} showing /s/e{i}.frag (x format)", i % 60).unwrap();
            }
            let list = playlist(entries.clone(), &History::parse(&log), &mut fastrand::Rng::with_seed(seed));
            let mut got: Vec<String> = list.iter().map(|e| e.id.clone()).collect();
            let mut want: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
            got.sort();
            want.sort();
            prop_assert_eq!(got, want);
            let unseen = shown.iter().filter(|&&was| !was).count();
            let history = History::parse(&log);
            prop_assert!(list[..unseen].iter().all(|e| history.last_shown(e.name()).is_none()));
        }
    }
}
