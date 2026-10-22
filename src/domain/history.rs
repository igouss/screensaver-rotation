//! What the run log says was shown, and when.
//!
//! Runners append `YYYY-MM-DD HH:MM:SS showing <id> (<kind> format)` when
//! they start showing an entry, and other lines (`stopped after …`) the
//! rotation ignores.

use std::collections::HashMap;

use super::entry::name_of;

/// A log time stamp, `YYYY-MM-DD HH:MM:SS`. Only that shape parses, so
/// stamps order as text in the order they happened.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp(String);

impl Stamp {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let shape = b"0000-00-00 00:00:00";
        let fits = s.len() == shape.len()
            && s.bytes().zip(shape).all(|(c, &want)| {
                if want == b'0' {
                    c.is_ascii_digit()
                } else {
                    c == want
                }
            });
        fits.then(|| Self(s.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One `showing` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Show {
    pub when: Stamp,
    /// What the runner was given, as it logged it.
    pub id: String,
    /// How the runner showed it: `fragcoord`, `ascii`, `lock`, …
    pub kind: String,
}

impl Show {
    fn parse(line: &str) -> Option<Self> {
        let (when, rest) = line.split_once(" showing ")?;
        let (id, kind) = rest.strip_suffix(')')?.rsplit_once(" (")?;
        Some(Self {
            when: Stamp::parse(when)?,
            id: id.to_owned(),
            kind: kind.strip_suffix(" format").unwrap_or(kind).to_owned(),
        })
    }
}

/// Every show in the log, oldest first, and when each name was last shown.
#[derive(Debug, Default)]
pub struct History {
    shows: Vec<Show>,
    last: HashMap<String, Stamp>,
}

impl History {
    #[must_use]
    pub fn parse(log: &str) -> Self {
        let shows: Vec<Show> = log.lines().filter_map(Show::parse).collect();
        let mut last = HashMap::new();
        for show in &shows {
            last.insert(name_of(&show.id).to_owned(), show.when.clone());
        }
        Self { shows, last }
    }

    /// When `name` was last shown, if ever.
    #[must_use]
    pub fn last_shown(&self, name: &str) -> Option<&Stamp> {
        self.last.get(name)
    }

    #[must_use]
    pub fn shows(&self) -> &[Show] {
        &self.shows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
2026-10-21 14:50:43 showing /c/shaders/yrden-circle.frag (fragcoord format)
2026-10-21 14:50:44 stopped after 2s: key press
2026-10-21 15:00:00 showing sharks (ascii format)
2026-10-21 15:48:02 showing /c/shaders/yrden-circle.frag (lock format)
2026-10-21 16:00:00 showing koi (ascii format)
garbage showing x (y format)
";

    #[test]
    fn shows_are_read_and_other_lines_skipped() {
        let history = History::parse(LOG);
        let kinds: Vec<&str> = history.shows().iter().map(|s| s.kind.as_str()).collect();
        assert_eq!(kinds, ["fragcoord", "ascii", "lock", "ascii"]);
        assert_eq!(history.shows()[3].id, "koi");
    }

    #[test]
    fn the_latest_show_of_a_name_wins() {
        let history = History::parse(LOG);
        let at = |name| history.last_shown(name).map(Stamp::as_str);
        assert_eq!(at("yrden-circle"), Some("2026-10-21 15:48:02"));
        assert_eq!(at("sharks"), Some("2026-10-21 15:00:00"));
        assert_eq!(at("koi"), Some("2026-10-21 16:00:00"));
        assert_eq!(at("plasma"), None);
    }

    #[test]
    fn an_empty_log_has_no_history() {
        let history = History::parse("");
        assert_eq!(history.shows(), &[] as &[Show]);
    }

    #[test]
    fn only_full_stamps_parse() {
        assert!(Stamp::parse("2026-10-21 15:48:02").is_some());
        for bad in [
            "2026-10-21 15:48",
            "2026-10-21T15:48:02",
            "2026-1x-21 15:48:02",
            "",
        ] {
            assert!(Stamp::parse(bad).is_none(), "{bad:?}");
        }
    }
}
