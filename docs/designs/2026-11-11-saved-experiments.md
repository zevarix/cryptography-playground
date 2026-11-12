# Local saved experiments

Status: the owner selected a local experiment library on 2026-11-11 after reviewing the save/reopen/delete scope and passphrase boundary. Implementation details below are maintainer choices within that scope.

## Behavior

Save a named experiment explicitly from either Text or File mode. An entry captures the completed run's input, recipe configuration, direction, intermediate stages, exact result and, when present, speculative reading and selected word choices. The saved recipe is independent of later library edits. Saving always creates a new entry; identical titles are allowed. There is no automatic retention or synchronization.

An Experiments panel lists entries and offers Open and Delete. Opening restores a separate working copy into the existing notebook, including its inspection and candidate-selection state. It must work after restarting the app and after the original input file or custom recipe has disappeared. Existing unsaved work receives a replacement choice; cancelling or failing to open preserves it. Deletion needs an explicit confirmation and removes only that entry, not its original input, recipe library entry, or active working copy.

Do not save passphrases, absolute source paths, credentials, or appearance preferences in experiments. Clear the workbench passphrase on opening an experiment. Existing results remain inspectable and exportable; keyed reruns or candidate changes require re-entry. Further candidate changes are verified against the captured ciphertext as usual. Saved speculative readings remain distinct from exact surviving text and never prove original wording.

## Approaches

- **Local library (selected):** application-managed snapshots under app data, using the current notebook to inspect and continue work. No package parser, external service, or export format is needed.
- **Portable experiment files (deferred):** useful for sharing, but requires a separate import/export and packaging contract. Existing recipe JSON and result exports remain available.

Keep Rust/Tauri/React and the current recipe schemas, outputs, fingerprints, and streaming file semantics. Apple Silicon work is deferred until suitable hardware is available; Windows and Intel Mac remain local acceptance targets, with existing Linux CI retained.

## Persistence and native boundary

Rust owns immutable captures. Text runs are captured at evaluation time; saving refers to the returned run ID rather than accepting caller-authored results. Document saving refers to a completed job and the exact requested reading. A background worker streams large artifacts to private, newly created experiment directories; it never sends complete large files through IPC. A versioned manifest contains bounded metadata, hashes and fixed artifact roles, never arbitrary file paths. Publish the manifest only after staged files are flushed and verified; interrupted attempts never list as completed saves. Return the saved entry ID only after readback.

Reopening validates the manifest, recipe, schema version and actual artifact contents before replacing notebook state. It creates fresh document/job/reading IDs, remaps word choices and candidates, and retains private temporary working copies so later library deletion cannot invalidate an open notebook. Restored jobs contain no key; reconstruction receives a newly entered runtime key and retains its full forward-verification check. Recipe IDs are changed on adoption into the editable notebook to prevent colliding with unrelated saved recipes.

Use a 4 MiB bound for manifest metadata and the existing 16 KiB Text input contract, not a new document-size cap. File artifacts remain streamed and disk-backed. Refuse malformed IDs, symlinks for library entries/artifacts, unknown schemas, missing/truncated files, inconsistent hashes, and invalid recipes. A broken entry must not hide healthy entries or be silently treated as valid. List at most 200 saved entries; report storage and validation failures without discarding the current run. Directory and file cleanup stays within the selected experiment's owned directory.

## Verification

Rust tests cover save/restart/open/delete, byte-exact UTF-8/BOM/CRLF source retention, large files beyond preview bounds, complete stages and readings, remapped choices, no saved passphrase or absolute path, original/recipe preservation, corrupt/unsupported metadata, missing/tampered artifacts, invalid IDs, symlinks and cancelled saves. UI tests cover Text/File save, reload/list/open/delete, cleared passphrases, fresh recipe IDs, stale result configuration, cancellation and failed replacement. Native Windows/Intel Mac acceptance uses synthetic fixtures, accepted exact-source builds, safe installation refresh and process/window readback. Existing Linux native validation remains required. No new signing or binary publication is authorized.
