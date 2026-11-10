# Binary-release checklist

The [source repository](https://github.com/zevarix/cryptography-playground) is public. Project-original code and documentation use the [MIT License](../LICENSE); third-party materials retain their own terms.

Before distributing installers or executables:

- Include notices for the exact resolved runtime dependencies, bundled data, and adapted palettes. See [THIRD_PARTY.md](../THIRD_PARTY.md).
- Test the release executable on each advertised platform. Windows and Intel macOS development testing does not establish Linux or Apple Silicon support.
- Review release configuration and the renderer/native command boundary, including the currently unset CSP.
- Decide distribution channels and signing/notarization requirements.

## What CI covers

The Windows validation job checks Rust formatting, tests, Clippy, the TypeScript build, and an isolated frontend smoke test. The smoke test mocks Tauri commands; application behavior changes also need native Windows and macOS testing under [AGENTS.md](../AGENTS.md).

The job always tests its scope classifier. Known docs-only changes skip dependency installation, compilation, and browser execution while still returning a completed check. Source, data, lockfile, workflow, mixed, unknown, and unavailable diffs run all checks; manual dispatch does too. Superseded PR attempts may be cancelled. Main and manual runs have distinct concurrency groups so later docs changes cannot discard code validation.

CI does not publish installers or screenshot artifacts. A passing source build is not binary-release approval.

The app remains an experimental cipher playground. Releases must not describe its toy ciphers as protection for sensitive information.
