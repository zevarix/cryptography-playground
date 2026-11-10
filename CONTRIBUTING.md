# Contributing

Keep changes focused on a concrete behavior, bug, or documented rule. Discuss new rule semantics and recipe-schema changes before implementing them.

## Development and checks

Follow [README.md](README.md). Use `npm ci` for the committed npm lockfile and `--locked` for Cargo checks. Keep the Rust engine testable independently of React and the desktop window.

Use the public [Rust Coding Standards](https://rcs.zevarix.com/) for new and meaningfully changed Rust code. Record the standards revision consulted in review evidence. Project-specific requirements in [AGENTS.md](AGENTS.md) take precedence.

Before submitting:

- Explain the problem, resulting behavior, and relevant limitations.
- Run Rust formatting, tests, Clippy, and the TypeScript production build for the code affected.
- Run `npm run test:gui` with `CHROME_EXECUTABLE` set when changing interface behavior. Its mocked bridge is not native acceptance.
- For CI scope changes, run `python -m unittest discover -s ci -p "test_*.py"` and explain which changes run or skip expensive checks. Unknown paths must run the checks.
- Test meaningful behavior and failure cases. Preserve recipe outputs unless a deliberate, versioned compatibility decision changes them.
- Retain data provenance and third-party notices. Do not commit real passphrases, private experiment text, machine paths, credentials, or generated screenshots.

For an application change, validate the accepted revision on Windows and macOS and update existing local test builds as described in AGENTS.md. Report an unavailable platform's native acceptance as pending.

## Recipe changes

Bundled examples use the same JSON schema as user-authored recipes. Do not special-case a recipe ID. Validate options before calling upstream adapters, preserve reversible/lossy boundaries, and keep unsupported reconstruction explicit.

See [the recipe schema](docs/recipe-format.md) and [fingerprint specification](docs/recipe-fingerprints.md). Never describe a recipe, passphrase-derived parameter, or fingerprint as secure encryption.

## Project and release decisions

Product direction, application licensing, public publication, distribution, and signing remain owner decisions. Third-party components retain their own terms; a contribution cannot relicense them. The current licensing decision is recorded in [the publication checklist](docs/public-readiness.md).
