# Cryptography Playground

An independent, experimental Rust + Tauri desktop application for composing, testing, and attacking toy ciphers.

**Not for protecting sensitive information.** These recipes deliberately discard linguistic information and are not substitutes for audited encryption.

## Direction

- Recipe-driven transformation engine with clear lossy/reversible boundaries.
- Recipe 001: full-phoneme American-English rotating collapse, not just the C/K/Q family.
- Key-derived rule parameters; inspect intermediate text and compare attack techniques.
- Cross-platform desktop interface built with Tauri 2, React, and TypeScript.

Recipe 001 will use the Carnegie Mellon Pronouncing Dictionary, with the original BSD-style license retained under `src-tauri/data/LICENSE-cmudict`. Unknown dictionary words must fail explicitly rather than silently using guessed pronunciation.

## Development

```sh
npm install
npm run tauri dev
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

No publication license has been selected yet. The repository remains private until a separate publication decision.
