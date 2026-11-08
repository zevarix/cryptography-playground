# Recipe fingerprints and derived parameters

A recipe is a versioned JSON configuration describing the sequence of rules to run. This project experimentally treats the configuration and a passphrase as separate inputs; **neither the recipe nor the passphrase makes these classical/linguistic ciphers secure**.

## Configuration fingerprint (CP-R1)

The editor and workbench display a short code such as \`CP-R1-0123456789abcdef\` and offer a full 64-character SHA-256 digest for copying.

Version 1 is computed as follows:

1. Parse recipe JSON into the typed Rust \`Recipe\` structure and validate the schema.
2. Serialize exactly \`{"schemaVersion":1,"steps":[...]}\` using the stable declared order of the typed \`Step\` / \`Rule\` fields (compact UTF-8 JSON with no optional unspecified fields). All steps—including disabled ones—and their order/configuration are retained.
3. SHA-256 the byte sequence \`cryptoplay:recipe-configuration:v1\\0\` followed by those canonical JSON bytes. The \`\\0\` here is one NUL byte.
4. Present the full lowercase SHA-256 hex digest. The short comparison code is \`CP-R1-\` followed by its first 16 lowercase hex digits. **Use the full digest for meaningful equality checks; the short code is a convenience only.**

The recipe's \`id\`, \`name\` and \`description\` are not hashed. That means users can rename or duplicate a configuration without changing its fingerprint. Changing a rule, its settings, enabled flags, or step order changes the fingerprint. Reordering JSON object properties does not change the fingerprint, because typed decoding normalizes their order.

The fingerprint is **not a key, signature, authenticity proof, password hash, or a way to recover the passphrase**. Two parties can use it to check they have the same declared recipe configuration, not that either party is trustworthy. The passphrase is deliberately omitted from the fingerprint and is not stored in recipe JSON.

## Deriving rule settings from a passphrase

Recipe 012 uses two newly supported **rule types**, which can also be added in the editor to any compatible saved recipe:

- \`{"type":"derived_caesar"}\`: Caesar shift chosen from 0–25
- \`{"type":"derived_rail_fence"}\`: Rail Fence count chosen from 2–16

These rule types have no fixed stored numeric parameter. At encoding *and* decoding, each derives its setting from the **entire normalized recipe configuration**, its **original zero-based step index** (including disabled steps), and the workbench **passphrase**.

For each derived rule:

1. Normalize the passphrase as existing keyed rules do: trim ASCII whitespace, lowercase, then validate 1–64 ASCII English letters.
2. Compute the 32-byte SHA-256 recipe configuration digest described above.
3. SHA-256 \`cryptoplay:rule-parameter:v1\\0\` + the raw 32-byte configuration digest + the rule index encoded as 4 little-endian bytes + the ASCII rule type ID (\`derived_caesar\` or \`derived_rail_fence\`) + one zero byte + normalized passphrase bytes.
4. Interpret the first four digest bytes as an unsigned little-endian integer \`n\`.
5. Caesar shift = \`n % 26\`. Rail count = \`2 + (n % 15)\`.

The resolved settings are shown in the transformation-stage labels so users can inspect how a puzzle was made. The same recipe and passphrase deterministically produce the same settings on Windows and macOS; different rule orders may change multiple settings. Existing Caesar/Rail Fence JSON remains **unchanged** and continues to use explicitly configured fixed values.

This is intentionally a fast derivation for puzzle reproducibility. It is **not PBKDF2, Argon2, a modern key-derivation function, authenticated encryption, or a cryptographic security scheme**. Rule values are chosen from small ranges and can be brute-forced. Do not protect personal, financial, login, or other sensitive data with this program.

## Sharing

Export/import the recipe JSON separately from the passphrase. The recipient can compare the full recipe fingerprint, then enter the agreed passphrase. A matching fingerprint and passphrase are necessary to reproduce derived rule settings. The fingerprint alone does not tell the recipient which passphrase to use.

Rule type identifiers and the derivation domain are explicitly versioned; **changing them in the future requires a new version** rather than silently changing previously saved recipe behavior.
