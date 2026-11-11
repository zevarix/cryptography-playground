# Apple Silicon Preview 2

The owner approved a separate experimental prerelease on 2026-11-11, following the recommendation to retain build instructions in source and upload the ZIP and checksum as release assets. Apple Silicon hardware testing remains pending. Existing Preview 1 downloads remain available.

Use the existing committed-source preview packager. Add an explicit macOS architecture target, preserve and verify the ARM executable's compiler-generated ad-hoc signature, retain dependency notices, and record the pending runtime acceptance in the archive. No publisher signing credentials or notarization are used. The app version remains 0.1.0; the distribution label is 0.1.0-preview.2.

- [x] Add regressions for target selection and real ARM signature verification, including changed executable bytes.
- [x] Extend the existing helper and document the repeatable Intel-to-Apple-Silicon build.
- [x] Run packaging checks and obtain an independent review (27 packaging tests passed; no P1/P2 findings).
- [x] Build from the committed revision, independently inspect an extracted archive and checksum, and confirm application sources match the tested saved-experiments revision.
- [x] Prepare and publish a GitHub prerelease with the ZIP, checksum, exact provenance and explicit runtime-testing limitation. Read back the published assets.

The initial publication did not merge the saved-experiments development PR and included only the Apple Silicon package, leaving the other platforms on Preview 1. The compiler target and temporary output directories are build inputs/artifacts, not committed application data.

Follow-up: after the initial Apple Silicon publication, the owner clarified that Preview 2 should include the other platforms too. Matching Windows, Intel Mac and Ubuntu packages were built from the same published source revision, `f109b52458e238966d9def8a96e2fa128149957f`, and added to Preview 2. Windows and Intel test installations were refreshed and native-checked; Ubuntu's exact package passed verification, apt installation and a native window check in the manual CI run. Apple Silicon's hardware limitation is unchanged. The release notes and public-readiness document record the final download set.
