# Cryptography Playground

An independent, experimental Rust + Tauri desktop application for composing, testing, and attacking toy ciphers.

**Not for protecting sensitive information.** These recipes deliberately discard linguistic information and are not substitutes for audited encryption.

## Direction

- Recipe-driven transformation engine with clear lossy/reversible boundaries.
- Recipe 001: full-phoneme American-English rotating collapse, not just the C/K/Q family.
- Key-derived rule parameters; inspect intermediate text and compare attack techniques.
- Cross-platform desktop interface built with Tauri 2, React, and TypeScript.

Recipe 001 will use the Carnegie Mellon Pronouncing Dictionary, with the original BSD-style license retained under `src-tauri/data/LICENSE-cmudict`. Unknown dictionary words must fail explicitly rather than silently using guessed pronunciation.

## Notebook Lab interface

The experimental desktop workbench combines a compact source editor, a step-by-step phonetic transformation notebook, and a result inspector. Appearance settings follow a desktop-style layout with an icon rail, searchable settings list, and a main preferences panel. System/light/dark mode, theme, accent, background/foreground, and font selections apply to the workbench and persist locally.

The community theme choices are adapted palettes from independent developer-theme projects (Catppuccin, Dracula, Gruvbox, Ayu, GitHub Primer, and Everforest); see [theme attribution and sources](docs/theme-attributions.md). They are not a dependency on any other application.

The application now provides a data-driven Rust rule engine and four bundled JSON recipe examples. Rules are configured through the recipe editor, while the existing manual-key phonetic behavior remains available as Recipe 001. Persistent experiment history and automatic adjective-key derivation are not implemented.

## Selecting candidate words

When decoding, the English guess highlights ambiguous words. Click one to open alternatives, or use the candidate buttons in the Inspector. Clicking a candidate updates the visible phrase immediately while the Rust engine checks the entire message against the **same ciphertext, key, and stateful phonetic rotations**. Successful choices become pinned; incompatible choices are rolled back with a message, without changing the exact recovered intermediate. The *Reset word selections* control removes pins and re-ranks the complete phrase.

Choices are interactive interpretations rather than proven plaintext. A manual English edit stays editable, but another candidate selection rebuilds the phrase from validated dictionary readings. The experiment remains intentionally lossy.

## Best-effort decoding

With the correct key, the reversible shift yields the exact phonetic intermediate text. The decoder now offers a **speculative English reconstruction** by matching dictionary pronunciations against the actual key-driven rotation counters. When multiple originals share the same lossy transform, it presents alternatives and allows editing; it does not claim to know the original spelling or punctuation.

For example, **fox** and **fix** can produce the same transformed word `fucc` and the same ciphertext under Recipe 001. The top-ranked suggestion may be wrong. For large or unsupported inputs, the exact intermediate remains available even if the English reconstruction fails.

The optional word-ranking data is derived from wordfreq and distributed with its attribution and notice: [reconstruction data provenance](src-tauri/data/NOTICE-english-frequency.md). The desktop application needs no Python runtime.

## User-authored recipes

The Rust rule engine now loads bundled example recipes from versioned JSON files in recipes/. These use exactly the same schema as user-created recipes; no recipe ID is special-cased. Open the Recipes panel to add, configure, enable, reorder, and save rules as locally stored JSON. You can duplicate bundled examples, copy JSON for sharing, or import pasted/selected JSON files. A native Save As dialog is not yet provided.

The built-ins are Recipe 001 Phonetic Rotation (previous output preserved), Recipe 002 Consonant Skeleton (lossy), Recipe 003 Word Mirror (reversible), and Recipe 004 Pair Swap + Vowel Rotation (reversible). Decoding applies reversible operations backward. Candidate reconstruction is available only for explicitly supported lossy combinations.

The editor identifies bundled examples, saved local recipes, and unsaved changes. Running a modified or newly created recipe without saving displays an **Unsaved recipe** notice and a shortcut back to the editor: those in-memory changes will be lost when the application restarts. Exact reversible decoding presents one plaintext result and uses the active recipe's name in the inspector, while lossy recipes retain the surviving intermediate and speculative English suggestion separately.

See docs/recipe-format.md for the schema and storage model.

## Classical cipher rules (public Rust crate)

The Rule Library now includes Caesar/ROT13, Vigenere, Beaufort, Affine, Atbash, and Rail Fence from the pinned cifers 0.1.5 crate. New bundled example recipes 005-011 use the same JSON schema as every user recipe; users can freely combine these steps with the experimental linguistic rules and save the result.

Vigenere and Beaufort use the workbench passphrase without storing it inside the recipe. All classical adapters preserve supported ASCII input on exact reverse, including case and punctuation. Rail Fence transposes all characters rather than stripping formatting. See [classical cipher adoption notes](docs/classical-cipher-adoption.md) for upstream source/license and test vectors. These historical ciphers are not secure encryption.

## Recipe configuration fingerprints and derived parameters

Each recipe has a reproducible \`CP-R1-...\` comparison code with a full SHA-256 fingerprint.
The fingerprint is computed from its versioned, normalized rule steps, **not** its name,
ID, description or passphrase. The editor updates it as rules change; the workbench shows
the active recipe's fingerprint. This is a configuration identifier, not proof of
authenticity or a secret key.

Recipe 012 demonstrates two new reusable rules: *Derived Caesar shift* and
*Derived Rail Fence*. These choose their numeric settings deterministically from
the recipe configuration, rule position and the runtime passphrase. The derived
values are visible in the transformation notebook, and existing fixed-parameter
rules retain their previous behavior. Passphrases are never exported in recipe JSON.
See [recipe fingerprint specification](docs/recipe-fingerprints.md) for the exact
versioned byte format and important limitations. This is for puzzles, **not secure
encryption**.

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
