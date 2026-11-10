# GLib security backport

This directory retains the published `glib 0.18.5` package under its original MIT license.
The source package SHA-256 is `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`.

Tauri 2's Linux/BSD backend requires GTK 0.18 and its GLib 0.18 dependency.
[GHSA-wrw7-89jp-8q8g](https://github.com/advisories/GHSA-wrw7-89jp-8q8g)
affects optimized string-variant iteration in the published package.
The upstream correction is
[`b5a4071e439bef2b5eea76c3aa25e5ae84839e34`](https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34).

The application uses a Cargo path patch so this source is selected throughout the
GTK dependency graph. The package version remains 0.18.5; it does not claim to be
an upstream release. Retain [LICENSE](glib/LICENSE), [COPYRIGHT](glib/COPYRIGHT),
and the original package metadata.

The regression harness in `tests/glib-regression` executes the affected iterator
operations in release mode. Remove this backport when the supported Tauri
dependency graph resolves an upstream package containing the fix, and rerun the
regression and native Linux checks.
