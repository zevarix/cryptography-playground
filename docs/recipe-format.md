# Data-driven recipes (schema 1)

Cryptography Playground provides rule implementations in Rust and recipes as JSON content. The bundled examples use the same schema as user-created recipes. These are experimental ciphers, not secure encryption.

## Representation

~~~json
{
  "schemaVersion": 1,
  "id": "user-6d5e0fbe-8300-4e93-a392-01cd29538f05",
  "name": "My Mirror Cipher",
  "description": "Try two reversible rules together.",
  "steps": [
    { "enabled": true, "rule": { "type": "word_mirror" } },
    { "enabled": true, "rule": { "type": "position_shift", "base": 5, "keyed": false } }
  ]
}
~~~

`schemaVersion` must be 1. IDs contain 1–64 lowercase ASCII letters, digits, or hyphens. Names must be nonblank and at most 100 UTF-8 bytes; descriptions may use up to 1,024 UTF-8 bytes. Each recipe has 1–16 steps, at least one enabled. Unknown rule types or fields are rejected, and options are range-checked for enabled rules. Recipes contain data, not executable scripts.

- position_shift: base 0–25, keyed true/false (passphrase optionally mixed into positional shift).
- rotate_vowels: steps 0–4.
- phonetic_rotate: must be first enabled rule and may appear only once; needs a letter-only key and dictionary-supported words.
- caesar: shift 0-25 (shift 13 is ROT13).
- atbash: no parameters.
- vigenere and beaufort: require the workbench passphrase (1-64 ASCII English letters); the key is not saved in JSON.
- affine: multiplier a is one of 1,3,5,7,9,11,15,17,19,21,23,25; offset b is 0-25. Invalid values are rejected before upstream constructors can panic.
- rail_fence: rails 2-16. All ASCII characters are transposed and restored, including whitespace and punctuation.
- derived_caesar and derived_rail_fence: no numeric parameters in JSON; their values derive from the exact rule configuration, zero-based step position and workbench passphrase (1-64 ASCII English letters). See [the fingerprint/derivation specification](recipe-fingerprints.md).

## Rule catalog

| Rule | Type | Reversible |
| --- | --- | --- |
| Phonetic rotation | phonetic_rotate | No |
| Remove vowels | remove_vowels | No |
| Collapse doubled consonants | collapse_doubles | No |
| Mirror each word | word_mirror | Yes |
| Voiced/unvoiced pair swap | pair_swap | Yes |
| Rotate vowels | rotate_vowels | Yes |
| Position shift | position_shift | Yes |
| Caesar / ROT13 | caesar | Yes |
| Atbash | atbash | Yes |
| Vigenere | vigenere | Yes |
| Beaufort | beaufort | Yes |
| Affine | affine | Yes |
| Rail Fence | rail_fence | Yes |
| Derived Caesar shift | derived_caesar | Yes (requires passphrase) |
| Derived Rail Fence | derived_rail_fence | Yes (requires passphrase) |

For reversible pipelines, decoding applies the enabled rules in reverse order and uses each inverse. These preserve supported ASCII input, including case, punctuation and whitespace.

For lossy pipelines, the decoder reverses the final reversible suffix and retains the exact surviving intermediate. Phonetic rotation followed by reversible rules supports dictionary-ranked English suggestions. So does vowel removal followed by doubled-consonant collapse and a reversible suffix. Other lossy combinations have no reconstruction strategy. Recognition depends on rule types, not recipe IDs.

## Bundled examples

- recipes/001.json: Phonetic Rotation followed by a keyed position shift with base 3.
- recipes/002.json: Consonant Skeleton, doubled-consonant collapse, unkeyed shift.
- recipes/003.json: Word Mirror, unkeyed shift.
- recipes/004.json: Pair swap, vowel rotation, unkeyed shift.
- recipes/005.json: Caesar / ROT13.
- recipes/006.json: Vigenere (workbench passphrase).
- recipes/007.json: Beaufort (workbench passphrase).
- recipes/008.json: Affine with a=5, b=8.
- recipes/009.json: Rail Fence with three rails.
- recipes/010.json: Atbash.
- recipes/011.json: Classical Layers (Vigenere, Rail Fence and Atbash).
- recipes/012.json: Passphrase-Derived Layers (Derived Caesar + Derived Rail Fence + Atbash).

These are editable examples, not special engine implementations. The examples use base 5 for the unkeyed shift. Users can change this rule and reorder compatible rules.

The classical rules use ciphers 0.1.0, pinned in Cargo.toml. Its MIT notice is retained and included in the app. See [the adapter documentation](classical-cipher-adoption.md) for provenance, test vectors, and ASCII formatting behavior.

## Storage and sharing

Custom recipes are JSON files in a `recipes` subdirectory under Tauri's app-data directory. Saving writes a temporary file in the same directory, renames it into place, then reads it back to verify the bytes. Storage is limited to 200 user recipes and 64 KiB per file.

Users can create, duplicate, enable/disable, reorder, configure, save, and delete recipes, copy JSON, or import pasted/selected .json files. Copy JSON exports to the clipboard; paste into a file to share it. A native Save As dialog is not yet implemented.

Changes to rule semantics require a new rule or schema version rather than silently changing saved recipes.
