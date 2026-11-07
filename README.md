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
npm run tauri dev  # development Vite server + desktop window
npm run tauri build -- --no-bundle  # standalone release executable
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

GUI smoke testing: set CHROME_EXECUTABLE to an installed Chromium-family browser executable and run npm run test:gui. The isolated headless browser mocks the Tauri invoke bridge; Rust engine tests and native GUI acceptance remain separate. Screenshots go into the ignored test-artifacts directory.

No publication license has been selected yet. The repository remains private until a separate publication decision.
