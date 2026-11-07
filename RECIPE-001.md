# Recipe 001 - Phonetic Rotation (experimental)

## Scope

First independent experiment in Cryptography Playground. This is a new specification, not compatibility with any earlier puzzle rounds.

The current implementation is a working foundation, with a **manual passphrase** and a fixed execution order. The roadmap includes automatic longest-adjective key derivation, configurable rule order/selection, and cryptanalytic scoring. Those features are not yet implemented.

## Current transformation

1. Tokenize ASCII-English words. Remove punctuation and casing, preserving word order and boundaries.
2. Look up the first pronunciation of each word in the bundled CMU US-English dictionary. Reject unknown words explicitly.
3. Process the full ARPABET sound inventory, not just C/K/Q: consecutive vowel phonemes collapse to one vowel from an independent global A/E/I/O/U cycle; consonant phonemes are rendered using a key-seeded, independently rotating set of alternative grapheme spellings for each sound.
4. Cycle the passphrase across transformed letters (not spaces), shifting each letter by `(3 + word_position + letter_position + key_letter_index) mod 26`. Positions begin at 1; A = 0.
5. Output lowercase ASCII words separated by single spaces. The decoder reverses step 4 to yield the exact intermediate, not the original English.

## Explicit limitations

- Not secure encryption. Do not use it to protect sensitive information.
- The full ARPABET sound inventory is covered, but not every English word or dialect is in the pronunciation dictionary.
- Phoneme-to-spelling transforms intentionally do not preserve natural English spelling.
- Vowels, punctuation, source graphemes and capitalization are destroyed. Plaintext recovery requires inference and may have multiple solutions.
- The exact input/output algorithm is subject to revision until the recipe reaches a versioned, reviewed specification.

## Decoding and uncertainty

Decode first reverses the shift to recover the intermediate exactly. The program then searches dictionary pronunciations against the current key and tracked global rotation counters to find possible original words. A bounded beam search ranks compatible phrases by an offline word-frequency heuristic. The best suggestion is **not** a confirmed plaintext; ambiguity is intrinsic because several original phoneme sequences can yield the same intermediate. Candidate alternatives are local to the chosen reconstructed prefix and may influence subsequent rotation states. Long or unknown messages may have no candidate; the exact intermediate is preserved.

## Roadmap

- Select the longest adjective from the source text with deterministic, documented lexical rules and fallback for ambiguity.
- Compose rule sets and vary activation, order, and parameters from the key; constrain incompatible orders.
- Add a cryptanalysis lab for key search, score models, and comparative measurements.
- Add configurable phonetic dictionaries and explicit dialect choices, with source provenance.
