# Unsigned preview packages

The preview helper creates portable ZIPs on native Windows or macOS. It does not publish a release, sign an application, or notarize a Mac app.

Install the normal build prerequisites from the README and Python 3.12 or newer. Commit the source you intend to package, then run from the repository root:

```sh
python3 -m tools.preview_package --output .task-files/preview-1 --label 0.1.0-preview.1
```

On Windows, use `python` instead of `python3`. Choose a new output directory for each attempt; existing archives are never overwritten.

The helper exports only the committed Git objects into a fresh source snapshot; local public assets, ignored environment files and other untracked inputs are excluded. It installs the locked npm dependencies, inspects Cargo's target-filtered normal dependency closure, assembles notices, remaps compiler source paths, and builds with Tauri's explicit `--no-sign` option. Existing custom Rust flags are rejected so they cannot silently change the build. The archive name records the native target's architecture; the application version and preview label are recorded separately in its manifest.

Each ZIP contains the app, an opening guide, the project license, a file/hash manifest, and notices. The Mac app carries notices inside `Contents/Resources/notices`, so they remain with it when moved. On Windows, keep the extracted executable and its notices directory together.

Notice materials include:

- Resolved Cargo/npm component versions and license text, including nested or supplemental notices.
- The Windows WebView2 wrapper MIT notice and separate Microsoft SDK license/supplemental notices, verified against the linked native loader.
- Matching Rust standard-library copyright and embedded license texts from the actual toolchain.
- CMU attribution, the wordfreq acknowledgements and licensed ranking-data adaptation, and retained palette notices.
- Complete published source archives for selected MPL components and for the retained objc2 notice overrides, with registry-lockfile checksum verification and a readable source index.
- [Pinned upstream notice evidence and qualifications](../packaging/UPSTREAM-NOTICES.md), including objc2's unresolved historical-attribution question and Gruvbox's published declaration.

The metadata closure may conservatively retain extra components where Cargo unifies features. It excludes development dependencies, build-only edges and proc-macro packages. Notice inventory is packaging evidence, not an assertion that every upstream attribution question has been settled.

## Verify and test

Compare the ZIP's SHA-256 with its adjacent `.zip.sha256` file. On Windows, use `Get-FileHash -Algorithm SHA256`; on macOS, use `shasum -a 256`. A checksum detects corruption; it is not a publisher signature.

Extract into a fresh directory, then check the payload:

```sh
python3 -m tools.preview_package --verify path/to/extracted/cryptography-playground-preview-folder
```

Launch the extracted application on its advertised platform. Check startup, rendering, native recipe commands and file open/process/save. Check the executable architecture, imported native libraries, unsigned status, and the Mac minimum OS recorded by its load commands. A passing browser mock or a successful compiler run alone is insufficient.

Windows requires Microsoft's [Evergreen WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution). The ZIP does not install system runtimes. Actual PE imports determine any additional redistribution requirement.

The Mac app is unsigned and unnotarized. Apple's [per-app opening guidance](https://support.apple.com/en-us/102445) explains the operating-system approval path. Package verification does not bypass Gatekeeper or change system-wide security settings.

Public distribution is a separate owner decision under [AGENTS.md](../AGENTS.md). Prepare archives and release notes for review before publishing; retain exact-source acceptance and the [binary-release checklist](public-readiness.md).

## Helper regression checks

```sh
python3 -m unittest discover -s tests -p "test_preview*.py"
```

These standard-library-only tests run in the existing application CI job when application checks apply. Documentation-only changes keep the inexpensive classifier path.
