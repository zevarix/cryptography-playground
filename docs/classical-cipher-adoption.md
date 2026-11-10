# Public classical-cipher adapter review

## Dependency and scope

The rule engine imports exactly **cifers = "=0.1.5"** from crates.io, rather than reimplementing each classical algorithm. The adapter is in src-tauri/src/recipes/classical.rs; the rest of the application operates on stable, versioned JSON Rule values, independent of crate structs.

- Package: https://crates.io/crates/cifers/0.1.5
- Publisher's source repository: https://github.com/PhantomSoap/cifers (located during public-readiness review, November 10, 2026; not linked from the published package metadata).
- API and reference source: https://docs.rs/cifers/0.1.5/cifers/
- Version reviewed: 0.1.5
- Cargo package license metadata: MIT
- The upstream README additionally says "MIT / Apache-2.0" while its package metadata says MIT; the inspected package contains no LICENSE file. Resolve the license notices and attribution before distributing binaries; [issue #7](https://github.com/zevarix/cryptography-playground/issues/7) tracks this question.

No app runtime downloads or online cipher service are required.

## Rule mapping

| Recipe rule type | cifers implementation | Parameters | Decoding |
| --- | --- | --- | --- |
| caesar | Caeser (upstream spelling) | shift: 0..25 | Shift backward |
| atbash | Affine::atbash() | None | Its own inverse |
| vigenere | Vigenere | Workbench passphrase | Decrypt using passphrase |
| beaufort | Beaufort | Workbench passphrase | Its own inverse |
| affine | Affine | a in {1,3,5,7,9,11,15,17,19,21,23,25}, b: 0..25 | Modular inverse |
| rail_fence | Railfence | rails: 2..16 | Reverse zigzag transposition |

Vigenere and Beaufort use the **same workbench passphrase** already used by other keyed rules. They do not embed that key in portable recipe JSON. The passphrase is normalized to lowercase ASCII letters and bounded to 64 characters by engine validation.

All rules are **reversible on supported ASCII input**. The upstream implementations operate on ASCII Latin letters; the existing application explicitly rejects non-ASCII source data and limits message size to 16 KiB. Rail Fence in particular transposes every input character, including spaces and punctuation, so it is exact and composable. Historical examples often discard formatting; this rule intentionally does not.

Affine's upstream parameter setters can panic on invalid values. Schema validation and the adapter check multipliers and offsets before calling those setters. Rail count is limited to 2–16.

## Test vectors and regression evidence

Rust tests include:

- Caesar shift 3: Hello, World! -> Khoor, Zruog!
- Atbash: ABC xyz! -> ZYX cba!
- Vigenere key lemon: ATTACK AT DAWN! -> LXFOPV EF RNHR!
- Beaufort key key: exampletext -> ghyypnglunl
- Affine a=5, b=8: AFFINE CIPHER -> IHHWVC SWFRCP
- Rail Fence 3 rails: WEAREDISCOVEREDFLEEATONCE -> WECRLTEERDSOEEFEAOCAIVDEN
- Upper/lowercase, spaces, punctuation and newlines all round-trip for every supported adapter.
- Invalid affine multipliers, offsets, rail counts, and missing keys return errors rather than panicking.
- Randomized reversible recipe round-trip tests include all six classical rule types.

These are historical teaching ciphers, **not secure encryption**. Dependency integration and passing tests do not resolve the binary-distribution notice question.
