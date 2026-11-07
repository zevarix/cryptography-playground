// Headless visual acceptance for the React surface. The Tauri invoke bridge is
// deliberately mocked here; the real Rust engine is validated by cargo test.
import assert from "node:assert/strict";
import { mkdir, stat } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const artifacts = resolve(root, "test-artifacts");
await mkdir(artifacts, { recursive: true });

let browser;
const vite = await createServer({
  root,
  configFile: resolve(root, "vite.config.ts"),
  server: { host: "127.0.0.1", port: 15421, strictPort: true },
  logLevel: "error",
});
try {
  await vite.listen();
  const executablePath = process.env.CHROME_EXECUTABLE;
  if (!executablePath) {
    throw new Error("Set CHROME_EXECUTABLE to a dedicated Chromium-family browser executable.");
  }
  browser = await chromium.launch({ headless: true, executablePath });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 960 },
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => {
    if (message.type() === "error") errors.push(message.text());
  });

  // A deterministic bridge stub, not a claim that the native bridge was tested.
  await page.addInitScript(() => {
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, arguments_) => {
        if (arguments_.key !== "spectacular") {
          throw new Error("Test bridge: incorrect key");
        }
        if (command === "encode_message") {
          return {
            ciphertext: "fixturecipher",
            intermediate: "fixtureintermediate",
            words: 9,
            phonemes: 32,
          };
        }
        if (command === "decode_message") {
          return { intermediate: "fixtureintermediate" };
        }
        throw new Error("Unexpected command: " + command);
      },
    };
  });

  await page.goto("http://127.0.0.1:15421/", { waitUntil: "networkidle" });
  assert.equal(await page.title(), "Cryptography Playground");
  assert.equal(await page.locator("h1").textContent(), "Turn language into a puzzle.");
  assert.match(await page.locator("body").innerText(), /Phonetic rotation/);
  assert.equal(await page.locator("#message").inputValue(), "The quick brown fox jumps over the lazy dog.");
  await page.getByRole("button", { name: "Encode with Recipe 001" }).click();
  assert.equal(await page.locator("#result").inputValue(), "fixturecipher");
  await page.locator("details.intermediate summary").click();
  assert.match(await page.locator("details.intermediate pre").textContent(), /fixtureintermediate/);

  const desktopPath = resolve(artifacts, "desktop-1440.png");
  await page.screenshot({ path: desktopPath, fullPage: true, animations: "disabled" });

  await page.getByRole("button", { name: "Inspect in decoder" }).click();
  assert.equal(await page.locator("#message").inputValue(), "fixturecipher");
  await page.getByRole("button", { name: "Reverse the shift" }).click();
  assert.equal(await page.locator("#result").inputValue(), "fixtureintermediate");
  await page.locator("#passphrase").fill("wrong");
  await page.getByRole("button", { name: "Reverse the shift" }).click();
  assert.match(await page.locator('[role="alert"]').innerText(), /incorrect key/);

  const mobile = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 1,
  });
  const mobilePage = await mobile.newPage();
  await mobilePage.goto("http://127.0.0.1:15421/", { waitUntil: "networkidle" });
  assert.equal(await mobilePage.locator("h1").textContent(), "Turn language into a puzzle.");
  const overflow = await mobilePage.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    page: document.documentElement.scrollWidth,
  }));
  assert.ok(overflow.page <= overflow.viewport + 1, "Mobile page overflows: " + JSON.stringify(overflow));
  const mobilePath = resolve(artifacts, "mobile-390.png");
  await mobilePage.screenshot({ path: mobilePath, fullPage: true, animations: "disabled" });
  assert.deepEqual(errors, [], "Unexpected browser console/page errors");
  console.log(JSON.stringify({
    status: "PASS",
    checked: ["desktop render", "encode action", "intermediate preview", "decoder navigation", "decode action", "error feedback", "mobile no-overflow", "no page errors"],
    screenshots: [
      { path: desktopPath, bytes: (await stat(desktopPath)).size },
      { path: mobilePath, bytes: (await stat(mobilePath)).size },
    ],
    nativeBridge: "mocked (not native GUI acceptance)",
  }, null, 2));
} finally {
  await browser?.close();
  await vite.close();
}
