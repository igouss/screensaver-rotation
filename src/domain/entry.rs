//! One thing a screensaver can show, and which runner shows it.

/// Something a runner listed: `id` is what the runner takes back to show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub runner: String,
    pub id: String,
}

impl Entry {
    /// The name the rotation knows the entry by.
    #[must_use]
    pub fn name(&self) -> &str {
        name_of(&self.id)
    }
}

/// An id's file name without its extension: a shader at `…/plasma.frag` is
/// `plasma`, and the aquarium's `sharks` is `sharks` whether it was logged as
/// that or, before runners listed themselves, as `…/sharks.ascii`.
#[must_use]
pub fn name_of(id: &str) -> &str {
    let base = id.rsplit('/').next().unwrap_or(id);
    base.rsplit_once('.').map_or(base, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_drop_the_directory_and_the_extension() {
        assert_eq!(name_of("/home/x/shaders/plasma.frag"), "plasma");
        assert_eq!(name_of("/home/x/shaders/sharks.ascii"), "sharks");
        assert_eq!(name_of("sharks"), "sharks");
        assert_eq!(name_of("a.b.glsl"), "a.b", "only the last extension");
        assert_eq!(name_of(""), "");
    }
}
