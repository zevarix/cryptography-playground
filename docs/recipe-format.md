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

The required schemaVersion is currently 1; name/description and IDs are bounded. Each recipe has 1–16 steps, at least one enabled. Each rule has a stable type with validated options; unknown types or parameters are rejected. No scripts or arbitrary code can be embedded.

- position_shift: base 0–25, keyed true/false (passphrase optionally mixed into positional shift).
- rotate_vowels: steps 0–4.
- phonetic_rotate: must be first enabled rule and may appear only once; needs a letter-only key and dictionary-supported words.
- caesar: shift 0-25 (shift 13 is ROT13).
- atbash: no parameters.
- vigenere and beaufort: require the workbench passphrase (1-64 ASCII English letters); the key is not saved in JSON.
- affine: multiplier a is one of 1,3,5,7,9,11,15,17,19,21,23,25; offset b is 0-25. Invalid values are rejected before upstream setters can panic.
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

For lossy pipelines, the decoder reverses the final reversible suffix and retains the exact surviving intermediate. The known Recipe 001 phonetic pipeline and Recipe 002 consonant skeleton have dictionary-ranked English suggestions; other lossy combinations may lack reconstruction support. The decoder does not claim to recover destroyed original information.

## Bundled examples

- recipes/001.json: Phonetic Rotation with keyed shift; byte-for-byte tested against the previous Recipe 001 ciphertext.
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

The public classical cipher rules are adapters to the exact pinned MIT-licensed cifers crate version 0.1.5. See docs/classical-cipher-adoption.md for implementation, test vectors, ASCII and punctuation semantics, upstream license ambiguity, and the independent public-distribution gate.

## Storage and sharing

Custom recipes are JSON files in a recipes subdirectory under Tauri's app-data directory. No cloud account. Saves use same-directory temporary files and verified readback; bounded to 200 user recipes and 64 KiB per file.

Users can create, duplicate, enable/disable, reorder, configure, save, and delete recipes, copy JSON, or import pasted/selected .json files. Copy JSON exports to the clipboard; paste into a file to share it. A native Save As dialog is not yet implemented.

The former Recipe 001 implementation remains the phonetic_rotate rule, not a special recipe selection. Semantics-changing upgrades must introduce a versioned rule or schema rather than silently changing saved recipes.
