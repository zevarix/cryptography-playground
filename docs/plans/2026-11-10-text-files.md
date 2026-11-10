# Text Files Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open UTF-8 documents, run versioned recipes with bounded previews, and save new results with an optional portable recipe or fingerprint companion.

**Architecture:** Rust owns snapshots, streaming transformations, background jobs, previews, and non-overwriting publication. React presents the existing notebook with a document panel. Schema 1 remains unchanged; schema 2 defines Unicode scalar transposition and ASCII-alphabet substitutions.

**Tech Stack:** Rust, Tauri 2, React 19, TypeScript 7; official Tauri dialog plugin, tempfile, existing ciphers 0.1.0 and SHA-256.

**Spec:** `docs/designs/2026-11-10-text-files.md`

## Global Constraints

- Preserve schema-1 validation, fingerprints, derivation and ASCII results.
- Schema 2 uses `CRYPT-R2`, `cryptoplay:recipe-configuration:v2\0` and `cryptoplay:rule-parameter:v2\0`.
- Strict UTF-8, BOM retained outside payload, no normalization or newline conversion.
- No arbitrary document-size cap; bounded working buffers and preview pages.
- Never overwrite source or existing destination files. Report partial companion/result publication.
- Keep full passphrases and native source paths out of sidecars and logs.
- Local application acceptance on Windows and macOS; retain Linux validation.

## Review Focus

- Concurrent source edits must fail capture or yield one identified immutable snapshot.
- Cancelled or obsolete jobs must never replace the active result.
- Repeated Save As, destination aliases and sibling collisions must preserve existing bytes.
- A short fingerprint collision must never select a configuration by catalog order.
- Empty files, BOM-only files, split UTF-8 reads and extremely long words must remain valid for reversible recipes.

---

### Task 1: Versioned recipes and identifiers

**Files:** Modify `src-tauri/src/recipes.rs`, `src-tauri/src/recipes/fingerprint.rs`, `src/recipeTypes.ts`; create `src-tauri/src/recipes/lookup.rs`.

**Interfaces:** Consume `Recipe`, `Step`, `Rule`; produce schema-2 validation, version-aware `fingerprint`, `resolve`, and `lookup(identifier: &str, recipes: &[Recipe]) -> Result<Option<Recipe>, String>`.

- [ ] Add failing tests for schema 2, version-domain golden hashes, schema-1 compatibility, full/short matching, missing identifiers, duplicate configurations, malformed versions and mismatched two-line fingerprints.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml recipes`; expect new schema-2 assertions to fail while legacy tests pass.
- [ ] Implement versioned validation/domains and exact catalog lookup. Keep schema-1 goldens unchanged; factor identifier resolution so a synthetic collision is testable.
- [ ] Run the recipe tests; expect all to pass. Commit `feat: version document recipes and resolve recipe fingerprints`.

### Task 2: Streaming Unicode transformations

**Files:** Create `src-tauri/src/documents/stream.rs`, `src-tauri/src/documents/transform.rs`, `src-tauri/src/documents/tests.rs`; modify `src-tauri/src/engine.rs` for a stateful phonetic word adapter; declare modules in `src-tauri/src/lib.rs`.

**Interfaces:** Produce `transform(rule: &Rule, source: &Path, destination: &Path, key: &str, undo: bool, control: &Control) -> Result<ArtifactStats, String>`, `ArtifactStats { bytes: u64, scalars: u64, digest: String }`, and `Control` cancellation/progress observations. Byte counters serialize to strings at the IPC boundary.

- [ ] Add failing tests with literal Caesar/Vigenere/rail vectors, mixed Unicode and CRLF round trips, all rail counts, long-word reversal, key/position/collapse state crossing buffers, cancellation and malformed UTF-8.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml documents`; expect missing document transformation behavior.
- [ ] Implement strict buffered scalar reading; bounded upstream substitution blocks; disk-backed word reversal and global rail spools. Retain all global counters across blocks, using modular state where sufficient. Stream phonetic words with dictionary-derived length bounds and global family/vowel state.
- [ ] Run document and legacy tests; expect exact small vectors and all round trips to pass. Commit `feat: stream Unicode document transformations`.

### Task 3: Snapshot and job lifecycle

**Files:** Create `src-tauri/src/documents/mod.rs`, `src-tauri/src/documents/jobs.rs`, `src-tauri/src/documents/storage.rs`.

**Interfaces:** Produce `DocumentStore` with opaque document/job IDs, capture, start, status, cancel, preview and discard operations. `DocumentInfo` exposes name, byte count, BOM and digest; `JobInfo` binds source generation, direction, recipe, fingerprint, stages and terminal state. `preview(id, stage, offset)` returns bounded text and string next-offset.

- [ ] Add failing tests for transactional invalid capture, source identity/metadata changes, BOM-only input, stale/unknown IDs, concurrent admission, stage paging, cancellation and prior-result retention.
- [ ] Run the document tests and observe failures before implementing the lifecycle.
- [ ] Add owner-private session temp storage and checked UTF-8 capture with one-pass digest/statistics. Run immutable jobs off-thread, serialize shared-state transitions, and expose phase/stage progress. Decode only the reversible suffix and retain the complete surviving intermediate for lossy pipelines.
- [ ] Verify multimegabyte pipelines and bounded previews; measure resident memory and cancellation responsiveness. Commit `feat: manage immutable document snapshots and cancellable jobs`.

### Task 4: Save As and companion import

**Files:** Create `src-tauri/src/documents/export.rs`, `src-tauri/src/documents/commands.rs`; modify `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/src/lib.rs`, `THIRD_PARTY.md` as required for dependency attribution.

**Interfaces:** Produce `save(job_id, destination: &Path, companion: Companion) -> Result<SaveReceipt, SaveFailure>`; `Companion` is `Json | Fingerprint | None`. Native Tauri commands open/save/import via Rust dialog API, retaining native paths. `SaveFailure` carries verified publication receipts, never an unqualified success.

- [ ] Add failing tests for source/destination aliases, preexisting siblings, staged verification failures, companion-first partial publication, safe retry with new names, no passphrase/path sidecar leakage and invalid companion import.
- [ ] Run document export tests; expect missing save/import behavior.
- [ ] Stage verified artifacts beside destinations, then publish using non-clobber primitives. Publish companion first and result last; retain job on failures. Read recipe companions with explicit bounded metadata limits before deserialization; never overwrite stored recipes on import.
- [ ] Verify unchanged hashes of originals/unrelated files and correct receipts after injected failures. Commit `feat: save document results with optional recipe companions`.

### Task 5: Document reconstruction

**Files:** Create `src-tauri/src/documents/reconstruct.rs`; modify `src-tauri/src/engine/recovery.rs`, `src-tauri/src/recipes/skeleton.rs`.

**Interfaces:** Produce separately requested cancellable reconstruction tied to a completed decode job, paged suggestions with stable word IDs, and candidate selection that recomputes global constraints. Keep existing interactive reconstruction contracts intact.

- [ ] Add failing tests for documents beyond interactive limits, cross-page choice IDs, incompatible pinned words, global phonetic state, cancellation and forward re-encoding of accepted suggestions.
- [ ] Run reconstruction tests; expect existing interactive limits to reject the document cases.
- [ ] Add document-oriented disk-backed search history and candidate output; retain bounded search width, dictionary constraints, explicit failure and exact intermediate. Stream skeleton reconstruction; preserve whole-stream phonetic constraints and verify suggested artifacts by forward re-encoding.
- [ ] Run full Rust suite; expect legacy behavior and document reconstruction tests to pass. Commit `feat: reconstruct document readings without losing exact intermediates`.

### Task 6: Notebook file interface

**Files:** Create `src/DocumentPanel.tsx`, `src/documentTypes.ts`; modify `src/App.tsx`, `src/App.css`, `src/RecipePage.tsx`, `tests/gui-smoke.mjs`.

**Interfaces:** `DocumentPanel` consumes catalog, active recipe, passphrase and direction and reports explicit recipe selection. Typed commands consume the Task 3/4/5 IPC contracts. Native file drop uses Tauri drag/drop events; browser tests mock only native boundaries.

- [ ] Add failing GUI cases for opening/failed opening, dirty discard, R1-to-R2 explicit copy, missing fingerprint import, bounded paging, progress/cancel, stale completion, companion choices and separate speculative output.
- [ ] Run `npm run build` and `npm run test:gui`; expect the new feature assertions to fail before UI implementation.
- [ ] Connect native Open/drop, source/stage previews, job controls, Save As and recipe/fingerprint import/copy. Present file mode inside the existing notebook, retaining manual entry. Label short/full copy separately and expose unsaved recipe portability status.
- [ ] Run build and GUI tests; expect all existing and new cases to pass. Commit `feat: add document workflow to the notebook`.

### Task 7: Documentation and complete acceptance

**Files:** Modify `README.md`, `docs/recipe-format.md`, `docs/recipe-fingerprints.md`; create `docs/text-files.md`.

- [ ] Document file encoding, R1/R2 semantics, companion portability, raw Markdown, temporary data lifecycle, no-clobber saves, lossy recovery and local-only processing.
- [ ] Run Rust fmt, all tests, Clippy with warnings denied, TypeScript build, GUI smoke, existing GLib regression and CI scope tests. Expect all required checks to pass.
- [ ] Review exact diff against the approved spec and RCS revision `11b12770b31b3ddcc2eb978080fa3e490c5f32dc`, refreshing current RCS at final review. Review coordination follows the owner's browser-only policy.
- [ ] Build the accepted revision unsigned with personal signing disabled on Linux, Windows and macOS. Refresh existing local test installations safely with rollback; read back executable/bundle, process and window. Record any unavailable target as pending.
- [ ] Commit documentation/evidence and present the completed branch and exact acceptance results. Public distribution and release remain separate decisions.
