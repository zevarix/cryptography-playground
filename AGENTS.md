# Project instructions

Cryptography Playground is a standalone desktop application. This repository is the authoritative owner of its source, architecture, tests, and release decisions.

- Product direction and publication decisions remain with the project owner.
- Do not introduce dependencies on private project-management systems, local tunnels, shared automation tooling, or unrelated repositories.
- Keep the Rust engine testable independently of its Tauri/React interface.
- Preserve explicit boundaries between reversible steps and permanently lossy linguistic transforms.
- Describe every cipher as experimental, never as protection for sensitive data.
- Protect existing workspaces; do not reset or overwrite unrelated changes.
- Verify Rust formatting, tests, Clippy, and the TypeScript production build for relevant changes.
- Honor third-party licenses and retain provenance with bundled data.
- Publication, licensing, distribution, signing, and release require separate decisions.

## Rust coding guidance

Source: `zevarix/rust-coding-standards` (internal development reference).
Standards reference: latest `main` at the time of coding/review.
Scope: new and meaningfully changed Rust code.
Project-specific exceptions: none currently.
Reviewers record the actual RCS revision consulted in review evidence.

This is a contributor workflow convention, not a runtime, build, governance, or release dependency. Before any public publication, replace inaccessible contributor instructions with self-contained public guidance if the standards are still private.
