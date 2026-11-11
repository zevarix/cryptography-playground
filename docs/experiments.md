# Saved experiments

The local experiment library is available in current source builds. Published Preview 1 predates this feature.

## Save a completed run

Run a recipe in **Text** or **File** mode. In **Keep this experiment**, enter a name and choose **Save experiment**. Saving creates a new entry in **Experiments**; identical names are allowed.

A saved experiment retains the completed run's input, recipe, direction, result and intermediate stages. When the run has a selected speculative English reading, its reading and accepted word choices are retained too. It uses the recipe captured by that run, even if you later change the workbench controls or edit/delete that recipe in the library. Manual English edits that have not been checked by the engine need to be run or selected as candidates before saving a completed experiment.

Saving is explicit. The app does not automatically retain experiment text. Appearance preferences and passphrases are excluded. Experiment data lives under the application's app-data directory, in `experiments`; it is local to this computer. It includes plaintext and intermediate text, so treat the saved library with the same care as the input documents.

## Open and continue

Choose **Experiments**, then **Open** beside an entry. The notebook restores its saved input, recipe, stages, result and selected reading. The original source file and custom recipe do not need to remain available. The opened recipe is an independent editable copy; it does not replace a recipe in your recipe library.

Opening clears the passphrase. You can inspect existing results and export a File result immediately. Re-enter any required passphrase before rerunning or changing candidate words. Further candidate choices are checked against the complete captured ciphertext. A speculative reading remains a compatible guess, not proof of the original wording.

Opening prompts before replacing unsaved notebook work. **Cancel** keeps the current notebook. A damaged or unsupported experiment reports an error and leaves the current work intact. The library lists readable entries even when another entry needs attention.

## Delete and storage

**Delete** asks for confirmation, then removes the selected saved entry. It does not remove the original input, a separately saved recipe, exported files, or the open working copy. Save again to retain a new copy of an experiment after changing it.

The library supports up to 200 completed entries and 4 MiB of metadata per entry. Text mode retains its 16 KiB input limit. File inputs and stages are copied in bounded chunks and keep the existing document workflow's size semantics; disk space and processing time still constrain practical size. A multi-stage file experiment can use several times the input's disk space.

Portable experiment packages and automatic synchronization are not currently provided. Existing recipe JSON and result exports remain available. Deletion is ordinary filesystem cleanup, not guaranteed secure erasure.
