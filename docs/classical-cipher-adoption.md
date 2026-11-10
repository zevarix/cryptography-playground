# Classical-cipher adapter

The recipe engine uses **ciphers = "=0.1.0"** from crates.io through `src-tauri/src/recipes/classical.rs`. Recipe rule IDs and saved JSON are independent of the library's types.

## Dependency and notices

- Package: https://crates.io/crates/ciphers/0.1.0
- Source: https://github.com/sradley/ciphers
- API: https://docs.rs/ciphers/0.1.0/ciphers/
- License: MIT, Copyright (c) 2019 Stephen Radley.
- [Retained license](../licenses/ciphers-0.1.0-MIT.txt), copied byte-for-byte from the published package's `LICENSE` file.
- Package SHA-256: `1b001d2f53009135648d4f38656955fbbd0992144bee2c82c7f6c49786ec7f4e`.
- License-file SHA-256: `45a93980856752724f1b64314e3902539d5d8953ccaef7ca5cd523c33e2c11a0`.

The full notice is imported as text into the About page, so it is included in the compiled frontend and native app, including unbundled local builds. Retain it when distributing or modifying the app.

This replaces cifers, whose 0.1.5 and 0.1.6 packages declare MIT but omit a standalone license notice and have conflicting README metadata. `ciphers` provides all six required algorithms, includes its MIT notice, and has no dependencies. [Issue #7](https://github.com/zevarix/cryptography-playground/issues/7) records the dependency decision and compatibility evidence.

## Rule mapping

| Recipe rule | Implementation | Parameters | Decoding |
| --- | --- | --- | --- |
| `caesar` | `Caesar` | shift 0–25 | Shift backward |
| `atbash` | `Atbash` | None | Its own inverse |
| `vigenere` | `Vigenere` | Workbench passphrase | Decrypt using passphrase |
| `beaufort` | `Beaufort` | Workbench passphrase | Its own inverse |
| `affine` | `Affine` | a in {1,3,5,7,9,11,15,17,19,21,23,25}; b 0–25 | Modular inverse |
| `rail_fence` | `RailFence` | rails 2–16 | Reverse zigzag transposition |

The upstream substitution APIs accept letters and return uppercase. The adapter extracts ASCII letters, applies the cipher, and restores their original case and the untouched nonletters. Only letters advance a repeating key. Rail Fence receives the full ASCII message and transposes every byte, including spaces, punctuation, and newlines.

Vigenere and Beaufort use the workbench passphrase, normalized to lowercase ASCII letters and limited to 1–64 letters. The passphrase is not saved in recipe JSON. The engine accepts nonempty ASCII messages up to 16 KiB. Parameter guards run before upstream constructors that can panic; unsupported input returns an error.

These boundaries preserve the meaning of existing recipe rules, including derived Caesar and Rail Fence settings. No recipe-schema or fingerprint version changes are needed.

## Compatibility checks

The adapter retains these test vectors:

- Caesar shift 3: `Hello, World!` → `Khoor, Zruog!`
- Atbash: `ABC xyz!` → `ZYX cba!`
- Vigenere key lemon: `ATTACK AT DAWN!` → `LXFOPV EF RNHR!`
- Beaufort key key: `exampletext` → `ghyypnglunl`
- Affine a=5, b=8: `AFFINE CIPHER` → `IHHWVC SWFRCP`
- Rail Fence 3 rails: `WEAREDISCOVEREDFLEEATONCE` → `WECRLTEERDSOEEFEAOCAIVDEN`

Tests also cover mixed case, all ASCII nonletters, repeating-key position, short Rail Fence inputs, every supported rail count, invalid parameters, and randomized reversible recipe composition. Fingerprint golden vectors remain unchanged.

These are historical teaching ciphers, **not secure encryption**. The remaining binary-release checklist still applies to other runtime dependencies, packaging, and platform support.
