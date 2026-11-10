# Text files, recipe exports, and fingerprint lookup

Status: proposed design for implementation approval. The expanded approach is agreed; the engineering choices below are proposed defaults.

Source reviewed: `zevarix/cryptography-playground` at `10bb3aa416af75f63c9ac97e882c98fc93c47ced`.

## Intent and agreed scope

Use the existing notebook to encode and decode `.txt` and `.md` documents. Markdown is raw text: headings, links, and code blocks participate in the chosen transformation. The source file remains unchanged.

The agreed workflow includes native Open and Save As, a separate recipe JSON file when wanted, an always-available fingerprint copy action, optional fingerprint-text export instead of JSON, and automatic matching against bundled and saved recipes. The passphrase remains a separate input.

The selected approach includes Unicode and larger documents. The current global ASCII/16 KiB restriction must not become the file feature's limit. Existing recipes, fingerprints, and accepted ASCII results must remain reproducible.

This is a contained application project, rather than only adding file dialogs: the current runner retains every full stage in memory, and several byte-to-character transforms would corrupt UTF-8 if their ASCII validation were removed.

## Approaches considered

| Approach | Consequence | Decision |
| --- | --- | --- |
| Add document dialogs around the current runner | Smaller change, retaining ASCII/16 KiB restrictions and full-stage UI payloads | Not selected |
| Extend text handling and document execution | Supports ordinary UTF-8 documents and larger inputs; requires engine, lifecycle, and save work | Selected |

Keep Rust/Tauri 2/React and the existing notebook. Native dialogs use the maintained Tauri dialog API from Rust; Rust owns file handles, processing, and publication. No private infrastructure becomes an application dependency. Exact new dependency versions and their license review belong in the implementation plan.

## Proposed compatibility contract

Introduce document-text semantics as recipe schema version 2. Version 1 retains its existing interpretation, validation, canonical serialization, `CRYPT-R1` fingerprints, parameter derivation, and golden results. No existing saved recipe is rewritten automatically.

Version 2 uses an explicitly versioned canonical configuration and derivation domain, with `CRYPT-R2` display identifiers and a full SHA-256 digest. Its configuration includes the schema version, ordered steps, options, and enabled flags. Names, descriptions, file paths, file contents, and passphrases remain outside the configuration fingerprint.

Use `cryptoplay:recipe-configuration:v2\0` and `cryptoplay:rule-parameter:v2\0`, where `\0` denotes one NUL byte. Keep the existing field ordering and parameter-input encodings described in the fingerprint specification, substituting the version-2 domains and schema version. The display code is `CRYPT-R2-` followed by the first 16 lowercase hex digits; the full digest is 64 lowercase hex digits. Passphrase normalization and the existing derived numeric ranges remain unchanged.

For fixed-parameter rules, ASCII vectors must agree between versions. Derived rules in a version-2 copy can resolve different parameters because the versioned configuration has a different identity. That is a new recipe, not a changed interpretation of an existing `CRYPT-R1` recipe.

The app offers a clearly labelled document-compatible copy of a legacy recipe when the file needs version-2 handling. It shows the new fingerprint before processing and retains the original. A recipe selected through an imported JSON or fingerprint is never silently upgraded. Include document-compatible bundled examples so normal document use has a ready starting point.

## Text behavior

The initial document encoding is strict UTF-8, with or without a UTF-8 BOM. Invalid UTF-8 produces a useful error; it is never decoded with replacement characters. Other encodings need explicit conversion outside this initial scope.

Keep a BOM as a transport header, outside the recipe payload, and retain it in the saved result. Do not normalize Unicode or translate line endings while reading or writing. Reversible version-2 pipelines must restore the original UTF-8 bytes, including combining marks, emoji sequences, tabs, controls, mixed line endings, and BOM handling.

Version-2 alphabet substitutions operate on ASCII A–Z/a–z and copy other Unicode scalars unchanged. Only eligible ASCII letters advance repeating keys. Word boundaries for the English letter rules are ASCII-letter runs; a non-ASCII scalar is retained and acts as a boundary. This defines behavior for accented and non-Latin text without inventing a universal alphabet.

Rail Fence transposes Unicode scalars, including whitespace and punctuation, without splitting UTF-8 encodings. Combining sequences can look unusual while encoded; decoding restores their exact sequence. Word Mirror reverses eligible ASCII-letter runs and retains the surrounding UTF-8 bytes.

Vowel removal and consonant collapse remain explicitly lossy, with Unicode-safe traversal. The phonetic rule remains an English-dictionary transform: unsupported words or non-English alphabetic input receive rule-specific errors. Unicode file support does not imply multilingual pronunciation support.

Empty documents are valid for reversible rules. A linguistic rule that requires words reports that requirement. Lossy processing never claims exact recovery of the original document or its formatting.

## Notebook and document lifecycle

Manual text entry remains editable. Opening or dropping a file attaches a document to the same notebook, with a read-only, pageable source preview. The full document stays in Rust-owned storage rather than being copied into a browser textarea. Show the file name, encoding, byte count, direction, selected recipe, and processing state.

Opening is transactional from the user's perspective: a cancelled picker, failed read, or invalid file leaves the prior notebook intact. Replacing dirty manual input or an unsaved recipe follows a clear discard/save choice.

Read through an opened native handle into an owned snapshot, validating UTF-8 and computing its digest/statistics during that pass. Check source identity and metadata around capture to detect ordinary concurrent edits. The job processes the captured bytes; previews and results always identify that same snapshot generation.

Capture source generation, direction, complete recipe, fingerprint, and runtime passphrase when Run starts. A later edit invalidates the visible result and prevents a stale completion from replacing current state. Save always uses the completed job's recipe and fingerprint, never whichever recipe happens to be selected later.

Run processing off the UI thread, with progress and cancellation. Preserve state across I/O buffers: repeating-key position, word counters, collapse state, phonetic counters, and transposition state may not reset at chunk boundaries.

Use an input snapshot and disk-backed stage artifacts with bounded previews. Never send full input plus every full stage through Tauri for a large document. Stage inspection reads pages from the corresponding immutable artifact. Native paths stay in Rust behind opaque document/job identifiers.

There is no arbitrary maximum document size. Working buffers and preview pages are bounded implementation resources, not processing limits. Filesystem, address-space, or resource failures are explicit failures; no truncation is presented as success. Counters and offsets use checked native arithmetic, and control identifiers do not depend on imprecise JavaScript numbers.

## Processing requirements

Reuse the pinned cipher library for bounded substitution blocks where its behavior is suitable. Carry repeating-key phase explicitly between blocks. Keep the legacy adapter available for version-1 compatibility tests.

Rail Fence needs a document-wide algorithm. Use rail spools and Unicode-scalar counts to split/concatenate or interleave rails; independently processing file chunks would produce the wrong result. Reuse statistics produced while reading/writing rather than rescanning the same stable artifact needlessly.

Word Mirror must handle an unbroken word longer than a working buffer. A bounded word buffer may spill to an owned spool, with reverse reads, instead of imposing a word-length limit. Dictionary lookup can reject words outside the actual bundled dictionary's vocabulary; that is a linguistic constraint, not a file-size cap.

For lossy decoding, always retain the complete exact surviving intermediate. English reconstruction is a separately requested background operation for document mode, with paged candidates, cancellation, and stable job-bound word identifiers. Preserve the current rule-recognition and forward re-encoding checks for accepted candidate selections. Do not reconstruct independent chunks and concatenate guesses: the global constraints must remain consistent. An actual inference resource failure leaves the exact intermediate available and does not become a claim of original-text recovery.

## Save and companion choices

Save As writes a new result, for example `notes.encoded.txt`. Decoded output gets a new suggested name and the user chooses its extension. Saving never writes over the source document.

The companion choice is:

| Choice | Result |
| --- | --- |
| Recipe JSON (default) | A separate `notes.recipe.json` containing the complete recipe in its declared schema |
| Fingerprint text | A separate `notes.fingerprint.txt`, containing the versioned display code and full digest |
| No companion | Only the result file; fingerprint copy remains available |

Keep the current full-digest copy behavior; make short-code copying a clearly labelled separate action. Fingerprint export is optional and is never an additional mandatory third file. Neither companion contains the passphrase or absolute source path.

A fingerprint-text companion has two lines: the versioned short display code, then `SHA-256: ` followed by the full digest. Import verifies that the short code agrees with the digest and supported version. This retains both the familiar label and a complete identifier for exact matching.

A JSON recipe is portable. A fingerprint identifies an already available configuration and cannot recreate it. If a custom configuration is not saved locally, show that status and offer saving it or exporting JSON; fingerprint copying remains available.

Prepare the selected artifacts in the destination directory before publication. Use new, non-overwriting final names, verify the resulting bytes/identities, and report success only when every selected artifact is verified. A collision offers another Save As name and leaves existing files intact.

Two sibling files cannot be assumed to publish atomically. Publish the companion before the result, and explicitly report partial publication if the later operation fails. Retain the completed processing job for retry. Never delete or overwrite an existing or externally changed file as rollback. Owned staging files can be cleaned up safely. The original remains unchanged in every failure case.

## Fingerprint matching and recipe import

Accept the existing short `CRYPT-R1` code, its full digest, and version-2 equivalents. Recompute fingerprints from validated bundled and saved configurations in Rust. Group identical configurations with different names/IDs by full digest.

An exact full-digest match selects the corresponding available configuration. A short code may select only one distinct matching configuration. Multiple distinct full digests under the short code require a full digest or recipe JSON; no match offers JSON import. Unknown versions and malformed identifiers fail clearly. A fingerprint is never expanded or guessed into rules.

Importing a companion validates its schema, field bounds, and rules before changing selection. Imported recipes are session-local until explicitly saved. Do not overwrite a stored recipe because a companion happens to reuse its ID. Current recipe metadata/storage bounds remain separate from document-size handling.

Companions can be chosen explicitly. Adjacent matching-name companions may be offered without a directory-wide search; automatically changing a dirty recipe still needs the normal discard/save choice. Where both a full digest and JSON are supplied, verify that they agree before selecting a recipe.

## Ownership, failures, and privacy

All processing is local. Temporary snapshots and stages live under an application-owned session directory with appropriate owner permissions. Cleanup acts only on proven-owned entries; crash recovery does not sweep arbitrary temporary directories. Cancellation and failure retain the previous usable notebook state.

Do not write passphrases to companion files, job journals, or logs. Progress/error messages identify the operation and useful location without dumping document contents. Native dialogs, file identity checks, snapshot generation, and publication receipts belong in Rust. Treat text/Markdown and recipe JSON as data, never executable markup or scripts.

The existing experimental-cipher status remains. Fingerprints and file checksums do not establish authenticity or turn these recipes into secure encryption.

## Verification and acceptance

- Keep all version-1 ASCII vectors, recipe fingerprints, and derived-parameter golden results unchanged. Test version-1 imports and lookup explicitly.
- Test version-2 fixed-rule ASCII compatibility and byte-exact reversible round trips for accented/decomposed text, non-Latin text, emoji, controls, empty input, BOM, and LF/CRLF/mixed endings.
- Exercise data beyond 16 KiB, multiple working buffers, UTF-8 characters split across reads, long words, key/word state across boundaries, and every supported rail count. Use synthetic multi-megabyte fixtures and hand-derived small vectors.
- Verify complete output with bounded preview payloads and a bounded working set; record throughput, memory, cancellation behavior, and disk work. Do not substitute an arbitrary input cap for this evidence.
- Test missing/ambiguous fingerprints, duplicate configuration names, invalid JSON/UTF-8, recipe version mismatch, stale results, source changes during capture, disk/write failures, destination collisions/aliases, partial pair publication, retry, and cancellation. Originals and unrelated files remain unchanged.
- Keep lossy intermediates distinct from suggestions, and validate accepted candidate choices by forward re-encoding.
- Reuse the existing frontend test harness for mocked dialog/job states. Rust integration tests own their temporary files. Native Windows/macOS acceptance uses owned fixtures, exact accepted builds, safe local installation refresh, rollback builds, and executable/process/window readback per `AGENTS.md`. Retain Linux native validation.
- Review CI applicability and marginal execution cost before changing checks. Documentation-only changes retain cheap routing; application, manifest, native dialog, engine, and mixed/unknown changes keep required validation. Record execution and queue measurements separately.

## Choices left to implementation planning

Choose module/API names, compatible dependency versions, bounded buffer/page sizes, filesystem publication primitives, and the on-disk stage/reconstruction representation. Those choices must satisfy the contracts above and be tested on supported platforms. Re-read the adopted Rust Coding Standards at implementation/review time and record the actual revision consulted.

This design does not authorize public installers, signing, or a release. It describes one document per notebook operation; batch/folder processing and additional text encodings are separate features.

## References

- [Project instructions](../../AGENTS.md)
- [Recipe format](../recipe-format.md)
- [Current fingerprint and derivation contract](../recipe-fingerprints.md)
- [Tauri native dialog API](https://v2.tauri.app/plugin/dialog/)
