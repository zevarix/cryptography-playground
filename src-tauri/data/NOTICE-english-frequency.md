# English Reconstruction Vocabulary

File: `src-tauri/data/english-frequency.txt` (20,000 ranked, lowercase ASCII English words)

This is a derived vocabulary used **only as a heuristic ranking prior** for speculative English reconstruction. It is not used to verify plaintext identity. Candidates must also re-encode exactly to the cipher's recovered intermediate form.

## Provenance and attribution

- Source project: [wordfreq](https://github.com/rspeer/wordfreq), version **3.1.1**, maintained by **Robyn Speer**. Please preserve this credit in any derivative distribution.
- Generated using the Python `wordfreq.top_n_list("en", 25000)` API. Entries containing non-ASCII characters, non-letter characters or single-character words were excluded; first 20,000 unique surviving words retained in original ranking order.
- The source project's [NOTICE](https://github.com/rspeer/wordfreq/blob/main/NOTICE.md) describes the data as redistributable under **Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)** and attributes its multiple source datasets. The wordfreq *code* has a different license (Apache 2.0); the derived ranked *data* here is not represented as Apache-licensed.
- This adapted vocabulary data is distributed under [Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)](https://creativecommons.org/licenses/by-sa/4.0/).
- Upstream content acknowledgements and terms: https://github.com/rspeer/wordfreq/blob/main/NOTICE.md

This is an adaptation/selection of the original vocabulary data. Changes made: normalized to lowercase ASCII-only words; filtered to alphabetic tokens with length greater than one; selected first 20,000 results in frequency order, one per line. Any public distribution must preserve appropriate attribution/share-alike obligations and be checked for other upstream-source requirements before publication.

This vocabulary is **not** a security corpus and must not imply cryptographic strength or guaranteed English recovery.
