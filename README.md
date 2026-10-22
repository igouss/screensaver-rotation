# screensaver-rotation

Picks which [Omarchy](https://omarchy.org) screensaver runs next. Screensavers
only know how to show things; this project owns the rotation across all of
them: the order things come up in, the history it is based on, and the Omarchy
launcher override that starts the winner on every monitor.

## Runners

A runner is any executable linked into `~/.local/lib/screensaver-rotation/runners/`;
its name in the rotation is the link's name. Each screensaver's own `install.sh`
links itself in. A runner answers two calls:

    RUNNER --list
        what it can show, one entry per line

    RUNNER --app-id ID --seed N --log FILE ENTRY...
        show the first ENTRY it can, fullscreen, in a window with Wayland app
        id ID; append `YYYY-MM-DD HH:MM:SS showing <entry> (<kind> format)` to
        FILE when it starts

Runners read their own settings. Today's runners:

- `shaders`: [shader-screensaver](https://github.com/igouss/shader-screensaver)'s
  `bin/rotation-runner`; its entries are the shader files in the collection,
  and it shows the first that compiles.
- `aquarium`: [ascii-aquarium](https://github.com/igouss/ascii-aquarium); its
  entries are `sharks` and `koi`.

A runner that fails or hangs on `--list` (5 s) is left out with a warning, so
one broken screensaver can't stop the others.

## The rotation

Every entry of every runner, best first: entries never shown (shuffled), then
the half of the rest shown longest ago (shuffled), then the others oldest
first, as fallbacks. An entry can't come back until about half the others have
played. Entries are known by name, the entry's file name without its
extension, so history follows a shader whichever runner lists it.

The history is the run log the runners append to,
`~/.local/state/screensaver-rotation/log`; `screensaver-rotation log-path` prints
it, for screensavers that show things outside a rotation run (a lock screen, a
viewer) and log them so the rotation moves past them.

The log doesn't grow forever: when a launch finds it over 256 KiB (a few
thousand shows), it is compacted to its latest 1000 lines and, before them,
only the last show of each entry that still exists. Older shows, shows of
removed entries and the stop lines between them go; every entry keeps its last
show, so compaction never changes the rotation's order.

When the idle timer fires, the launcher asks `screensaver-rotation launch` for
the command to run: the runner of the first entry, given that runner's entries
in rotation order, so a shader that doesn't compile falls back to the next
shader.

## Install

    ./install.sh

Builds `~/.local/bin/screensaver-rotation` and links
`omarchy/omarchy-launch-screensaver` into `~/.local/share/omarchy-overrides/bin/`.
Omarchy's idle service starts the launcher with `bash -lc`, so that directory
has to come first on PATH; `install.sh` prints the line for `~/.bashrc` if it
isn't there yet. Without the binary, or with nothing to show, the launcher
falls back to Omarchy's own screensaver.

## Use

    screensaver-rotation pick                   # the rotation, runner<TAB>entry
    screensaver-rotation pick --runner shaders  # one runner's part of it
    screensaver-rotation list                   # every entry, last shown or "new"
    screensaver-rotation last                   # what was that one?
    screensaver-rotation history 50             # the latest shows
    screensaver-rotation log-path               # where the run log is
    omarchy-launch-screensaver force            # run the next one now

## Develop

`src/domain/` is the rotation's rules with no I/O: entries and their names,
the run log's history, and the playlist order. `src/adapters/` reads the
runners directory and the log. `src/main.rs` is the command line.

    cargo test                                  # unit and property tests
    cargo clippy --all-targets -- -D warnings   # strict lints: pedantic and nursery
