# Unsigned preview packages

The preview helpers create portable ZIPs on Windows or macOS and a [Debian package on native Linux](#linux-debian-preview). The macOS helper also cross-compiles between Intel and Apple Silicon. They do not publish a release, use publisher signing credentials, or notarize a Mac app.

To try the application, [download the published Preview 2 packages](https://github.com/zevarix/cryptography-playground/releases/tag/v0.1.0-preview.2). Windows, Intel Mac and Ubuntu packages have native acceptance evidence; the Apple Silicon package remains an experimental download awaiting hardware testing. The commands below are for preparing new packages from source.

Install the normal build prerequisites from the README and Python 3.12 or newer. Commit the source you intend to package, then run from the repository root:

```sh
python3 -m tools.preview_package --output .task-files/preview-1 --label 0.1.0-preview.1
```

On Windows, use `python` instead of `python3`. Choose a new output directory for each attempt; existing archives are never overwritten.

The helper exports only the committed Git objects into a fresh source snapshot; local public assets, ignored environment files and other untracked inputs are excluded. It installs the locked npm dependencies, inspects Cargo's target-filtered normal dependency closure, assembles notices, remaps compiler source paths, and builds with Tauri's explicit `--no-sign` option. Existing custom Rust flags are rejected so they cannot silently change the build. The archive name records the selected target's architecture; the application version and preview label are recorded separately in its manifest.

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

The Intel Mac app is unsigned and unnotarized; Apple Silicon signing details are below. Apple's [per-app opening guidance](https://support.apple.com/en-us/102445) explains the operating-system approval path. Package verification does not bypass Gatekeeper or change system-wide security settings.

## Apple Silicon experimental preview

On an Intel or Apple Silicon Mac with the existing build prerequisites:

```sh
rustup target add aarch64-apple-darwin
python3 -m tools.preview_package --output .task-files/apple-silicon-preview-2 --label 0.1.0-preview.2 --target aarch64-apple-darwin
```

This creates an `aarch64` ZIP and checksum. It uses a macOS 11.0 deployment target and retains the compiler-generated ad-hoc executable signature required for ARM code. Tauri binary patching is disabled to preserve those signed bytes. The helper checks the bundled executable's ARM64 architecture, equality with the standalone compiler output, and the standalone output's native signature verification. The linker signature does not seal the app bundle's resources or identify a publisher; the package manifest supplies a separate file/hash inventory. No publisher signing credentials or notarization are used.

The README and manifest mark Apple Silicon runtime acceptance as pending. Cross-compilation and archive checks do not verify startup, rendering, file operations or saved experiments on the target hardware. The recorded macOS 11.0 minimum is a build setting, not tested compatibility with that OS version. Preview 2 includes native-tested Windows, Intel Mac and Ubuntu packages alongside the separately qualified experimental Apple Silicon download.

`--target` defaults to the compiler's native target. macOS additionally accepts `x86_64-apple-darwin` or `aarch64-apple-darwin`; other cross-platform requests are rejected. Existing Intel and Windows preview behavior is retained.

Public distribution is a separate owner decision under [AGENTS.md](../AGENTS.md). Prepare archives and release notes for review before publishing; retain exact-source acceptance and the [binary-release checklist](public-readiness.md).

## Helper regression checks

```sh
python3 -m unittest discover -s tests -p "test_preview*.py"
```

These standard-library-only tests run in the existing application CI job when application checks apply. Documentation-only changes keep the inexpensive classifier path.

## Linux Debian preview

On native Linux, prepare a Debian package using the same committed-source and notice checks:

```sh
python3 -m tools.preview_linux --output .task-files/linux-preview --label 0.1.0-preview.1
```

This produces an unsigned `.deb`, its SHA-256 sidecar and a separate `.deb.manifest.json` inventory. The helper normalizes the generated Debian control version to use a tilde, so the preview sorts before the stable application version. The separate manifest pins the whole archive checksum as well as extracted file bytes. GTK and WebKit remain system dependencies; the package does not copy their shared libraries into the distribution. Application/dependency/data notices and selected corresponding sources are installed under `/usr/lib/Cryptography Playground/notices`.

Compare the checksum before installing, then use `sudo apt install ./cryptography-playground-0.1.0-preview.1-linux-x86_64-unsigned.deb`. Package dependencies are resolved by apt. The initial acceptance target is Ubuntu 24.04 x64; other distributions and architectures need their own runtime acceptance.

Verify the package against its separate payload manifest on a system with `dpkg-deb` and `ar`:

```sh
python3 -m tools.preview_linux --verify-deb package.deb --manifest package.deb.manifest.json
```

The existing validation workflow also offers a manual `linux_preview` input. When enabled, its Ubuntu job builds a release package, verifies it, installs it, and opens the installed application in an isolated X11 session. The resulting package/checksum/manifest are uploaded as a workflow artifact for review. Ordinary PR/main validation keeps its existing debug build/window check and creates no installer artifact. Neither path publishes a GitHub release.
