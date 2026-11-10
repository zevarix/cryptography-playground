# Third-party materials

Project-original code and documentation use the [MIT License](LICENSE). Bundled data, adapted palettes, and dependencies retain the separate terms and notices below; the project's MIT license does not relicense them.

| Material | Provenance | Terms and notices |
| --- | --- | --- |
| CMU Pronouncing Dictionary | [Pinned source and checksums](src-tauri/data/PROVENANCE.md) | [Bundled CMU license](src-tauri/data/LICENSE-cmudict) |
| English ranking vocabulary | 20,000-word adaptation from wordfreq 3.1.1, by Robyn Speer | CC BY-SA 4.0; [adaptation/attribution notice](src-tauri/data/NOTICE-english-frequency.md) and [source acknowledgements](src-tauri/data/wordfreq-notice.txt) |
| Adapted theme palettes | Catppuccin, Dracula, Gruvbox, Ayu, GitHub Primer, Everforest | [Sources](docs/theme-attributions.md) and [retained notices](docs/theme-license-notices.md) |

The CMU license requires retained notices for source and binary redistribution. The ranking vocabulary notice identifies the filtering/selection and credits Robyn Speer; its source acknowledgements also name the SUBTLEX authors and other contributing datasets. Preserve those notices, the CC BY-SA 4.0 license link, and the adaptation's share-alike terms when distributing that data.

`src-tauri/data/wordfreq-license.txt` concerns upstream wordfreq **code**. It is not the license of the ranked vocabulary data. The app does not need Python or execute wordfreq at runtime.

## Dependencies and binary distribution

Cargo.lock and package-lock.json pin resolved Rust and JavaScript packages, which retain their own terms. Most implementations are resolved from registries. The Linux/BSD GLib dependency is retained as a [documented security backport](src-tauri/vendor/README.md), with its original [MIT license](src-tauri/vendor/glib/LICENSE) and [copyright notice](src-tauri/vendor/glib/COPYRIGHT).

The classical adapter uses **ciphers 0.1.0** under MIT, Copyright (c) 2019 Stephen Radley. Its complete [license notice](licenses/ciphers-0.1.0-MIT.txt) is retained from the published package and displayed in the app's About page. The notice is embedded in the compiled frontend and native application. [The adapter documentation](docs/classical-cipher-adoption.md) records package provenance and formatting compatibility. The previous cifers dependency has been removed; [issue #7](https://github.com/zevarix/cryptography-playground/issues/7) records that notice-gap resolution.

Before distributing installers or executables, assemble notices for the exact resolved runtime dependencies and include the bundled-data and palette notices in the distribution materials. A successful source build does not establish that binary-distribution review.

Remaining binary-release checks are recorded in [docs/public-readiness.md](docs/public-readiness.md).
