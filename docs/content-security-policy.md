# Renderer content security policy

The installed app uses `app.security.csp` in [tauri.conf.json](../src-tauri/tauri.conf.json).
It loads bundled JavaScript, CSS and images and communicates with Rust through Tauri IPC.
It does not need remote scripts, fonts, network services, frames or browser workers.

| Resource | Production policy |
| --- | --- |
| Default | Deny |
| Scripts, stylesheets, images | Bundled origin only |
| Connections | `ipc:` and `http://ipc.localhost`, Tauri's native bridge |
| Script/style attributes, external fonts, objects, frames, workers, form submission, base URL overrides | Deny |

Production does not allow `unsafe-inline`, `unsafe-eval`, `data:`, `blob:`, generic HTTP/HTTPS origins or development WebSockets.
Tauri's automatic script hashes and script/style nonces remain enabled.
Appearance settings and frequency bars use React's direct style-property updates, which work without allowing inline style attributes or arbitrary style blocks.
System fonts do not require downloadable font permission.

This policy is an additional renderer boundary. It does not replace recipe/file validation or native command permissions, and it does not make the experimental ciphers suitable for sensitive information.
The capability file is unchanged: local `main` window only, no remote capability grant.

## Development

`devCsp` separately allows Vite's inline React refresh bootstrap, injected development CSS and local hot-reload connections.
WebSockets are limited to `localhost`/`127.0.0.1` on ports 1420 and 1421, matching [vite.config.ts](../vite.config.ts).
These permissions are absent from installed production builds.
The explicit script-attribute restriction stays enabled even in development.

If a developer deliberately uses `TAURI_DEV_HOST` on another device, add that exact development host to a local `devCsp` override; do not widen the production policy.

## Verification

Run `npm run build`, then `npm run test:gui` with `CHROME_EXECUTABLE` pointing to a Chromium-family browser.
The CSP suite serves the production assets with the configured production policy. It checks the recipe interface, rendering and appearance settings, and attempts inline JavaScript, event handlers, eval, external script/fetch/WebSocket/image loads, injected styles, frames, objects, workers, base overrides and form submission.
A second loopback server proves blocked requests do not reach another origin.
The suite also checks Vite/React refresh and a delivered hot-reload message under `devCsp`.

The browser suite simulates native IPC. It does not prove Tauri's generated policy, platform WebView behavior or native file dialogs; Windows and macOS build and installed-app acceptance remain required by [AGENTS.md](../AGENTS.md).

References: [Tauri CSP](https://v2.tauri.app/security/csp/), [Tauri security configuration](https://v2.tauri.app/reference/config/#securityconfig), [style attribute policy and direct property updates](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/style-src-attr).
