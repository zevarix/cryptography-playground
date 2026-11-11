# Binary-release checklist

The [source repository](https://github.com/zevarix/cryptography-playground) is public. Project-original code and documentation use the [MIT License](../LICENSE); third-party materials retain their own terms.

## Published preview

[Preview 1](https://github.com/zevarix/cryptography-playground/releases/tag/v0.1.0-preview.1) is available as unsigned Windows x64 and Intel Mac ZIPs and an Ubuntu 24.04 x64 Debian package. The release records exact source revisions, platform acceptance, retained notices, checksums, and upstream notice qualifications. Apple Silicon, older macOS versions, and other Linux distributions or architectures remain unverified.

The Mac app is unnotarized. Checksums establish archive integrity, not publisher identity. Preparation and verification are documented in [Unsigned preview packages](preview-packages.md).

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
