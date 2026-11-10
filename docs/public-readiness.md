# Public source and release preparation

This checklist separates source publication from distributing a desktop binary. Product, licensing, publication, signing, and release decisions remain with the owner.

## Source preparation

- [x] Describe the current 12 examples, editable recipes, speculative decoding, and fingerprints.
- [x] Link to publicly accessible Rust Coding Standards.
- [x] Add contributor instructions, safe bug-report guidance, and issue forms.
- [x] Retain data provenance and adapted-theme notices; verify CMU checksums and the 20,000-word ranking file.
- [x] Prepare pinned-action validation and weekly dependency updates.
- [x] Validate the candidate and verify the first full GitHub Actions run.
- [x] Review source and published branch history with targeted credential/private-context patterns; record the scope and limitations in the pull request.
- [x] Apply the owner-selected MIT license to project-original code and documentation, with license text and package metadata.
- [x] Document retained third-party notice questions and their binary-distribution gate.
- [x] Owner approved merging the preparation and making the source repository public.
- [ ] Apply and verify public visibility, branch protection, and security settings.

## CI scope and cost

The validation job always starts, so a docs-only change does not leave a required check pending. It first tests and runs the dependency-free applicability classifier. Known documentation-only changes skip dependency installation, compilation, and browser execution. Mixed, source, data, lockfile, workflow, and unknown changes run all application checks; an unavailable diff also runs them. Manual dispatch runs the full set. Superseded PR attempts may be cancelled. Main and manual runs use distinct concurrency groups, so a later docs update cannot discard a pending or running code validation.

One Windows runner covers the frontend build, Rust formatting/tests/Clippy, and the isolated headless-browser smoke test. Node downloads use the lockfile cache; Rust caching includes the compiler version and Cargo lockfile. There is no matrix, deployment, installer publication, or screenshot-artifact upload. Execution and queue-time measurements belong in first-run evidence; no speed or cost savings are claimed before measurement.

The frontend test mocks Tauri commands. It does not replace native Windows/macOS acceptance for application behavior changes.

## Separate binary-release work

- Resolve the cifers notice question in [classical-cipher-adoption.md](classical-cipher-adoption.md).
- Assemble and deliver notices for resolved runtime dependencies, data, and palettes.
- Verify the accepted executable on each claimed platform. Existing Windows/Intel-Mac development use is not Linux or Apple Silicon runtime proof.
- Review release configuration and the renderer/native boundary, including the currently unset CSP.
- Decide distribution targets and signing/notarization requirements.

The app remains an experimental cipher playground. No release may describe its toy ciphers as protection for sensitive information.
