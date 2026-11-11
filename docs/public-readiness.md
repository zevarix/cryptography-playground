# Binary-release checklist

The [source repository](https://github.com/zevarix/cryptography-playground) is public. Project-original code and documentation use the [MIT License](../LICENSE); third-party materials retain their own terms.

## Published preview

[Preview 2](https://github.com/zevarix/cryptography-playground/releases/tag/v0.1.0-preview.2) includes the saved experiment library in Windows x64, Intel Mac and Ubuntu 24.04 x64 packages, plus an experimental Apple Silicon ZIP. All four were built from `f109b52458e238966d9def8a96e2fa128149957f`, pinned by the release tag and package manifests. Windows and Intel Mac extracted packages were adopted into their local test installations with rollback retained. Native acceptance covered restored experiments, Unicode file results and export; Windows additionally exercised fresh Text/File save, full process exit, restart and reopening after moving the original file. Ubuntu's exact Debian package was verified, installed with apt and native-window-tested in CI.

Windows, Intel Mac and Ubuntu packages are unsigned. Mac downloads are unnotarized. Checksums establish archive integrity, not publisher identity. Preparation and verification are documented in [Unsigned preview packages](preview-packages.md).

The owner approved publication on 2026-11-11 and subsequently clarified that Preview 2 should also carry the other platforms' updated packages. Apple Silicon hardware testing remains pending. Its compiler-generated ad-hoc executable signature is retained; there is no publisher signature, app-bundle resource seal or notarization. Its inclusion is not a verified platform-support claim. Older macOS versions and other Linux distributions or architectures remain unverified.

[Preview 1](https://github.com/zevarix/cryptography-playground/releases/tag/v0.1.0-preview.1) remains available as the earlier release without saved experiments.

## Before each distribution

For a new or changed installer or executable:

- Include notices for the exact resolved runtime dependencies, bundled data, and adapted palettes. See [THIRD_PARTY.md](../THIRD_PARTY.md).
- Test the exact release executable on each advertised platform. Preview 1's acceptance does not establish support for other architectures, operating systems, or later builds.
- Review release configuration and the renderer/native command boundary, including the [configured content security policy](content-security-policy.md).
- Confirm owner approval for the distribution channel, release, and signing/notarization policy. Preview 1's unsigned publication is not approval for future releases.

## What CI covers

The Windows validation job checks Rust formatting, tests, Clippy, the TypeScript build, packaging helpers, and isolated frontend suites. The frontend suites mock Tauri commands; application behavior changes also need native Windows and macOS testing under [AGENTS.md](../AGENTS.md). The Linux job additionally checks the optimized GLib regression and selected dependency graph, builds the native application, and verifies its window in an isolated X11 session.

The job always tests its scope classifier. Known docs-only changes skip dependency installation, compilation, and browser execution while still returning a completed check. Source, data, lockfile, workflow, mixed, unknown, and unavailable diffs run all checks; manual dispatch does too. Superseded PR attempts may be cancelled. Main and manual runs have distinct concurrency groups so later docs changes cannot discard code validation.

Ordinary PR/main validation does not create installer artifacts. An explicit manual `linux_preview` run prepares, verifies, installs, and native-window-tests the Debian package, then uploads the package, checksum, and manifest as workflow artifacts for review. Neither path publishes a GitHub release or uses signing credentials. A passing build or workflow artifact is not binary-release approval.

The app remains an experimental cipher playground. Releases must not describe its toy ciphers as protection for sensitive information.
