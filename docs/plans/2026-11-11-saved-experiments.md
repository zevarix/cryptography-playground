# Saved experiments implementation plan

Goal: explicitly save and reopen complete local Text/File experiments without retaining passphrases.
Spec: ../designs/2026-11-11-saved-experiments.md
Stack: existing Rust/Tauri 2/React/TypeScript; no new dependencies or CI jobs.

## Global constraints

- Preserve recipe schemas, fingerprints, outputs and reversible/lossy boundaries.
- Retain full large-file artifacts with bounded I/O and previews.
- Save only Rust-captured completed runs; no passphrases or absolute input paths.
- Opening creates independent working copies and fresh IDs; failure/cancellation preserves existing work.
- Keep Apple Silicon deferred. Verify exact builds on Windows and Intel Mac; retain Linux CI.

## Tasks

1. Download documentation: add the published preview link and correct Linux/release coverage. Done in 1e35cd6; relative links, whitespace and 10 classifier tests passed.
2. Rust capture/persistence/IPC: add documents/experiments with bounded versioned manifests, private streamed artifact copies and native-controlled Text captures; restore jobs/readings with fresh IDs and no stored key; list failures per entry. Add focused real-filesystem tests before implementation, then run the full Rust suite, formatting and Clippy.
3. React notebook/library: add experiment types, a named Save form, Experiments list with open/delete confirmation, and Text/File restoration. Use captured run IDs/JobInfo rather than current controls when saving. Clear passphrases, adopt fresh recipe IDs, preserve selections, and guard busy/dirty replacement. Add behavioral renderer cases before implementation; reuse existing test processes.
4. Integrate and qualify: update README/text-file docs, run production build and complete GUI suites, inspect narrow/keyboard layouts, run native Windows/Intel Mac save/restart/open/delete with owned fixtures, and refresh test apps. Run existing Windows/Linux CI on the committed revision. Review the entire branch independently, address actionable findings, and prepare a draft PR.

## Interfaces shared by Tasks 2 and 3

All IPC uses camelCase arguments. `evaluate_recipe` retains all existing result fields and adds `runId`. An optional input `runId` binds restored Text word changes/reset to their immutable native capture for complete forward verification; ordinary evaluations omit it. `save_text_experiment({runId,name}) -> Summary`; `save_file_experiment({jobId,readingId,name}) -> Summary` (readingId null excludes a reading). `list_experiments() -> {entries:Summary[],problems:string[]}`; `delete_experiment({id}) -> void`; `open_experiment({id}) -> Opened`.

Summary = {id,name,createdAt,kind:'text'|'file',recipeName}. Opened is {kind:'text',summary,recipe,direction,source,result,choices,runId} or {kind:'file',summary,document,job,reading,choices}. Types reuse RecipeResult, WordChoice, DocumentInfo, JobInfo, ReadingInfo and DocChoice. File open adopts restored IDs in the existing document panel; JobInfo adds `restored:boolean` (false for normal runs). `doc_reconstruct` accepts optional `key` for restored jobs; old callers retain their current behavior. For restored jobs, the backend requires supplied runtime key normalization before reconstruction.

## Review focus

- Controls edited after a completed run: saves retain the original run/recipe.
- Loading while another run/save or prompt is active: reject or cancel safely; do not lose current work.
- Original file or recipe deleted: saved experiment still opens; adopting its recipe never overwrites a library recipe.
- One corrupt saved entry: healthy entries remain accessible; invalid metadata never drives a pathname.
- Key omitted/wrong after restart: stored output remains visible, further computation requires a valid supplied key and complete compatibility verification.
