# Text files

The file workflow runs locally in the desktop app. These experimental ciphers are for puzzles and learning, not sensitive information.

## Open and run

1. Choose **File**, then **Open file**, or drop one `.txt` or `.md` file into the app. Files must contain valid UTF-8. Markdown is raw text, including headings, links and code blocks.
2. Choose a recipe and encode or decode. The two UTF-8 examples are convenient starting points. **Create UTF-8 recipe copy** explicitly copies a legacy recipe to schema 2 with a new fingerprint; it does not change the saved original.
3. Inspect the source, result or individual stages. Preview pages are bounded; the recipe runs on the complete document. Changing the selected recipe, direction or passphrase marks an older result as previous. Saving that result uses its captured recipe.
4. Use **Cancel operation** to stop work. A failed or cancelled run leaves the last completed result available. Opening a replacement or rerunning with changed settings prompts before discarding an unsaved result.

The app captures a private temporary copy after opening and checks source identity and metadata during capture. Later edits to the source do not alter an admitted run. Temporary copies, stage files and readings are removed with their session; they are not persistent experiment history or guaranteed secure deletion.

## Save results and companions

**Save As…** writes a new UTF-8 result. Existing source, result and companion files are never overwritten. Choose a new name if any destination already exists.

| Companion choice | Contents | Use |
| --- | --- | --- |
| Recipe JSON (default) | `<result-stem>.recipe.json` | Carries the complete recipe, without its passphrase |
| Fingerprint text instead of JSON | `<result-stem>.fingerprint.txt` | Identifies a recipe already available in the recipient's library |
| No companion | Result only | Keep or share recipe information separately |

Both **Copy fingerprint** (full digest) and **Copy short code** remain available. A fingerprint identifies the configuration, not the document, and cannot reconstruct an unknown recipe. Use **Find recipe by fingerprint** to select a bundled or saved match; if absent, import the recipe JSON. Imported recipes are session-local until saved in the library. Keep passphrases separately; neither source paths nor passphrases are added to companions.

Saving stages and verifies bytes before publishing new files, then verifies their current names. The companion is published before the result. If publication partly succeeds, the app reports which files were saved or need checking. Check those files and choose another destination before retrying; multi-file saves are not an atomic transaction.

## Text semantics

Schema 1 retains its original nonempty ASCII and 16 KiB input limit, results, fingerprints and derivations. File mode permits it only for compatible payloads. Schema 2 supports strict UTF-8 and empty documents with reversible recipes. The editable **Text** mode still has a 16 KiB limit; use **File** for larger inputs.

There is no fixed document-size limit. Processing uses bounded working buffers and disk-backed intermediate files, so time and available temporary storage still constrain practical size. Every enabled stage may need another document-sized file; Rail Fence, long-word reversal and reconstruction need additional temporary storage. Opening and saving also require temporary copies.

- No Unicode normalization or newline conversion occurs.
- A leading file UTF-8 BOM is preserved as a transport header outside the cipher payload. A leading U+FEFF entered in editable Text mode is actual text.
- Alphabet substitutions, word mirroring, vowel removal and consonant rules operate on ASCII English letters. Other Unicode scalars pass through as separators or unchanged characters under those rules.
- Rail Fence transposes Unicode scalar values across the complete payload, including punctuation and line breaks. A visible grapheme may contain multiple scalars and may look separated in ciphertext; exact reversal restores its original bytes.
- Key positions, rotations, word positions and rail state continue across buffer and preview boundaries.
- Phonetic rotation remains an English dictionary transform. It canonicalizes linguistic text and rejects unsupported words. It is lossy, including formatting; UTF-8 support does not make it a general-language cipher.

## Lossy decoding and English readings

A lossy decode stops at the first irreversible step and preserves the **Exact surviving intermediate**. **Suggest an English reading** runs separately for supported phonetic or consonant-skeleton pipelines. Candidate choices apply to stable word positions across the whole document. The app validates a proposed reading by forward-encoding the complete captured recipe and comparing it with the complete captured ciphertext.

Candidate pages do not divide the search into independent sentences. The bounded dictionary search may find no complete reading, even when some plausible original exists. A rejected choice or cancelled search leaves the previous reading and exact intermediate available.

A successful reading is speculative: re-encoding establishes compatibility, not original wording, spelling or formatting. Choose explicitly between the exact intermediate and the English reading in **Result to save**. Saving the reading never changes the exact intermediate.
