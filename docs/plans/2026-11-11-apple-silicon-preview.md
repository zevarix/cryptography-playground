# Apple Silicon Preview 2

The owner approved a separate experimental prerelease on 2026-11-11, following the recommendation to retain build instructions in source and upload the ZIP and checksum as release assets. Apple Silicon hardware testing remains pending. Existing Preview 1 downloads remain available.

Use the existing committed-source preview packager. Add an explicit macOS architecture target, preserve and verify the ARM executable's compiler-generated ad-hoc signature, retain dependency notices, and record the pending runtime acceptance in the archive. No publisher signing credentials or notarization are used. The app version remains 0.1.0; the distribution label is 0.1.0-preview.2.

- [x] Add regressions for target selection and real ARM signature verification, including changed executable bytes.
- [x] Extend the existing helper and document the repeatable Intel-to-Apple-Silicon build.
- [x] Run packaging checks and obtain an independent review (27 packaging tests passed; no P1/P2 findings).
- [ ] Build from the committed revision, independently inspect an extracted archive and checksum, and confirm application sources match the tested saved-experiments revision.
- [ ] Prepare and publish a GitHub prerelease with the ZIP, checksum, exact provenance and explicit runtime-testing limitation. Read back the published assets.

This release does not merge the saved-experiments development PR or claim new Windows/Intel Mac/Ubuntu packages. Preview 1 remains the tested download for those platforms. The compiler target and temporary output directories are build inputs/artifacts, not committed application data.
