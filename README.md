# Cryptography Playground

A desktop notebook for composing toy ciphers, inspecting each transformation, and exploring what a decoder can recover.

Built with Rust, Tauri 2, React, and TypeScript. **These experimental and historical ciphers are for puzzles and learning, not for protecting sensitive information.**

## What you can do

- Build recipes from configurable rules, enable or reorder steps, and inspect intermediate text.
- Create, duplicate, import, and save recipes as local JSON. The 12 bundled examples use the same schema.
- Combine phonetic rotation, consonant skeletons, word mirroring, vowel rotation, and positional shifts with Caesar/ROT13, Vigenere, Beaufort, Affine, Atbash, and Rail Fence.
- Compare recipe configurations using a reproducible `CRYPT-R1-...` fingerprint. Optional derived rules choose parameters from the configuration and runtime passphrase.
- Customize the notebook's light/dark appearance, theme, accent, colors, and font.

A recipe fingerprint identifies its configuration. It is not a secret key, signature, or proof of authenticity.

## Reversible steps and lossy guesses

Reversible recipes decode supported ASCII input exactly, including case, punctuation, and whitespace. Rules that discard phonetic or spelling information cannot recover the original text uniquely.

For supported lossy recipes, the decoder keeps the exact surviving intermediate separate from a speculative English reconstruction. For example, **fox** and **fix** can share the same phonetic transform. Select alternative words to explore candidates that still re-encode consistently; a compatible candidate is not proof of the original plaintext.

Recipe 001 uses the Carnegie Mellon Pronouncing Dictionary. Unsupported words fail explicitly. The ranking vocabulary comes from wordfreq and is only a heuristic.

## Recipes and local storage

Use the **Recipes** panel to create or duplicate a recipe, configure its rules, then save it. Unsaved edits are marked and are lost when the app restarts. Saved custom recipes live in a `recipes` directory under Tauri's app-data directory. Appearance preferences are stored in the local WebView.

Copy JSON to share a recipe, or import a pasted/selected JSON file. Passphrases are not included in recipe JSON. A native **Save As** dialog and persistent experiment history are not implemented.

- [Recipe format and all bundled examples](docs/recipe-format.md)
- [Fingerprint and derived-parameter specification](docs/recipe-fingerprints.md)
- [Classical cipher adapters and test vectors](docs/classical-cipher-adoption.md)
- [Recipe 001 phonetic rules](RECIPE-001.md)

## Build from source

Install a current stable Rust toolchain, Node.js 22.12 or newer supported by Vite, and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/):

- **Windows:** Microsoft C++ Build Tools with the desktop C++ workload, plus WebView2.
- **macOS:** Xcode Command Line Tools or Xcode.
- **Linux:** the distribution-specific WebKitGTK 4.1 and other native development libraries listed in the Tauri guide.

From a checkout:

```sh
npm ci
npm run tauri dev
```

Build the frontend or an unbundled local desktop executable:

```sh
npm run build
npm run tauri build -- --no-bundle
```

The desktop executable is written under `src-tauri/target/release/`. The frontend build alone is not the native application. Linux and Apple Silicon runtime acceptance have not been established by the existing Windows/Intel-Mac checks.

## Checks

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
npm run build
```

For the isolated frontend smoke test, set `CHROME_EXECUTABLE` to an installed Chromium-family browser executable and run `npm run test:gui`. It launches a separate headless browser and mocks Tauri commands; it does not establish native cipher or filesystem correctness. Screenshots are written to ignored `test-artifacts/`.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the check workflow and native acceptance requirements.

## Licensing and project status

Project-original code and documentation are licensed under the [MIT License](LICENSE). Third-party materials retain their own terms.

Bundled dictionaries, ranking data, and adapted theme palettes retain their own licenses and attribution. See [THIRD_PARTY.md](THIRD_PARTY.md), [theme notices](docs/theme-attributions.md), and the [remaining publication checklist](docs/public-readiness.md).

Report ordinary bugs through GitHub issues. For vulnerabilities or sensitive reports, follow [SECURITY.md](SECURITY.md).
