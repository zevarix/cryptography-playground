# Recipe 001 - Phonetic Rotation (experimental)

## Recipe

Recipe 001 combines `phonetic_rotate` with a keyed `position_shift` using base 3. It takes a manual passphrase and produces lowercase ASCII words. The phonetic step discards information, so decoding cannot uniquely recover the original English.

The bundled recipe is [recipes/001.json](recipes/001.json). It uses the same editable [recipe format](docs/recipe-format.md) as other examples. Rules can be enabled, reordered, or configured in the editor, but phonetic rotation must remain the first enabled rule.

## Current transformation

1. Tokenize ASCII-English words. Remove punctuation and casing, preserving word order and boundaries.
2. Look up the first pronunciation of each word in the bundled CMU US-English dictionary. Reject unknown words explicitly.
3. Process the ARPABET sound inventory: consecutive vowel phonemes collapse to one vowel from a global A/E/I/O/U cycle. Each consonant sound has a separate, key-seeded cycle of alternative spellings.
4. Cycle the passphrase across transformed letters (not spaces), shifting each letter by `(3 + word_position + letter_position + key_letter_index) mod 26`. Positions begin at 1; A = 0.
5. Output lowercase ASCII words separated by single spaces. The decoder reverses step 4 to yield the exact intermediate, not the original English.

## Explicit limitations

- Not secure encryption. Do not use it to protect sensitive information.
- The full ARPABET sound inventory is covered, but not every English word or dialect is in the pronunciation dictionary.
- Phoneme-to-spelling transforms intentionally do not preserve natural English spelling.
- Vowels, punctuation, source graphemes and capitalization are destroyed. Plaintext recovery requires inference and may have multiple solutions.
- Changes to rule semantics require a new rule or schema version; they must not silently change saved recipes.

## Decoding and uncertainty

Decoding first reverses the shift to recover the intermediate exactly. The program then searches dictionary pronunciations using the passphrase and the rotation counters carried across words. A bounded beam search ranks compatible phrases using an offline word-frequency heuristic. Several original phoneme sequences can yield the same intermediate, so the highest-ranked suggestion is **not** a confirmed plaintext. Long or unknown messages may have no candidate; the exact intermediate is preserved.

## Candidate selection and re-evaluation

Select an ambiguous word in the decoder to choose a dictionary alternative. Each choice is pinned by its zero-based **word index**. The search restarts with all pinned choices and recomputes the later rotation counters. A selection is accepted only if the full suggested phrase re-encodes to the recovered intermediate. A conflicting choice is rejected; the previous suggestion, source ciphertext, and exact intermediate remain available.

Only the first few ranked alternatives are displayed. A word that reproduces its local transformed representation may still fail to produce a valid full-message path when the counters advance differently. None of this resolves the fundamental ambiguity caused by lossy preprocessing.

## Unimplemented experiments

- Automatic passphrase selection from the longest adjective in the source text.
- Selecting rule activation and order from the passphrase. Derived numeric parameters are available separately; see [recipe fingerprints](docs/recipe-fingerprints.md).
- A cryptanalysis lab for key search and scoring comparisons.
- User-selectable pronunciation dictionaries and dialects.
