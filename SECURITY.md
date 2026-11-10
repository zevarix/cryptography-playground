# Security

Cryptography Playground explores toy and historical ciphers. It is not suitable for passwords, financial records, private messages, or other sensitive information. A recipe fingerprint does not authenticate a recipe, and speculative English reconstruction does not prove plaintext identity.

Security-relevant reports include recipe-import validation failures, access outside the intended app-data storage, dependency vulnerabilities, and renderer/native command boundary problems. The `main` branch is maintained; there is no supported binary release yet.

## Reporting

Use GitHub's **Report a vulnerability** option on the repository's Security page when available. Do not put exploit details, credentials, passphrases, private recipe files, or personal text in a public issue.

If private reporting is unavailable, open an issue that only requests a private reporting channel, without exploit details or sensitive attachments. A maintainer can arrange the next step.

For an ordinary cipher result or interface bug, use synthetic text and a minimal recipe that reproduces it. Include the app version and operating system.

## Local data

Custom recipes are stored in the app-data directory; appearance preferences use local WebView storage. Recipe JSON excludes the runtime passphrase. This does not make local storage or the clipboard a secure secret store.

Release security review, signing, and platform distribution are separate from a successful source build. See [the binary-release checklist](docs/public-readiness.md).
