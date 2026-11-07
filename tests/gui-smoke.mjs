// Isolated frontend acceptance: the Tauri invoke bridge is mocked.
// Rust cipher semantics and native window acceptance are tested separately.
import assert from "node:assert/strict";
import { mkdir, stat } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const artifacts = resolve(root, "test-artifacts");
await mkdir(artifacts, { recursive: true });
const server = await createServer({
  root, configFile: resolve(root, "vite.config.ts"),
  server: { host: "127.0.0.1", port: 15423, strictPort: true },
  logLevel: "error",
});
let browser;
try {
  await server.listen();
  const executablePath = process.env.CHROME_EXECUTABLE;
  if (!executablePath) throw new Error("Set CHROME_EXECUTABLE to an installed Chromium-family browser.");
  browser = await chromium.launch({ headless: true, executablePath });
  const desktop = await browser.newContext({ viewport: { width: 1440, height: 960 }, deviceScaleFactor: 1 });
  const page = await desktop.newPage();
  const errors = [];
  page.on("pageerror", e => errors.push(e.message));
  page.on("console", m => { if (m.type() === "error") errors.push(m.text()); });
  await page.addInitScript(() => {
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (args.key !== "spectacular") throw new Error("Test bridge: incorrect key.");
        if (command === "encode_message") return {
          ciphertext: "fixturecipher", intermediate: "fixtureintermediate", words: 9, phonemes: 32,
        };
        if (command === "decode_message") return {
          intermediate: "fixtureintermediate",
          reconstruction: {
            suggested: "the quick brown fox jumps over the lazy dog",
            note: "Speculative expansion. 1 word has several valid matches.",
            words: [
              { transformed: "fucc", suggested: "fox", alternatives: ["fix", "fax"], compatibleCount: 3 },
            ],
          },
        };
        throw new Error("Unexpected command: " + command);
      },
    };
  });

  await page.goto("http://127.0.0.1:15423/", { waitUntil: "networkidle" });
  assert.equal(await page.title(), "Cryptography Playground");
  assert.match(await page.locator("h1").innerText(), /Experiment: Phonetic Rotation/);
  await page.getByRole("button", { name: /Encode/ }).click();
  assert.equal(await page.locator("#cipher-result").innerText(), "fixturecipher");
  assert.equal(await page.locator("#intermediate-result").innerText(), "fixtureintermediate");
  const pngWorkbench = resolve(artifacts, "notebook-workbench-desktop.png");
  await page.screenshot({ path: pngWorkbench, fullPage: true, animations: "disabled" });
  await page.locator("#operation-mode").selectOption("decode");
  await page.getByRole("button", { name: /Decode \/ suggest English/ }).click();
  assert.equal(await page.locator("#intermediate-result").innerText(), "fixtureintermediate");
  assert.equal(await page.locator("#cipher-result").innerText(), "the quick brown fox jumps over the lazy dog");
  assert.match(await page.locator(".reconstruction-warning").innerText(), /not verified|cannot verify|not which original/);
  assert.match(await page.locator(".reconstruction-word").innerText(), /fix, fax/);
  const pngDecode = resolve(artifacts, "notebook-decoder-english-suggestion.png");
  await page.screenshot({ path: pngDecode, fullPage: true, animations: "disabled" });
  const editableEnglish = page.getByRole("textbox", { name: "Edit suggested English" });
  await editableEnglish.fill("the quick brown fix jumps over the lazy dog");
  assert.equal(await page.locator("#cipher-result").innerText(), "the quick brown fix jumps over the lazy dog");
  await page.locator("#passphrase").fill("incorrect");
  await page.getByRole("button", { name: /Decode \/ suggest English/ }).click();
  assert.match(await page.locator('[role="status"]').innerText(), /incorrect key/);

  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  assert.equal(await page.locator("h1").innerText(), "Appearance");
  await page.getByRole("group", { name: "Appearance mode" }).getByRole("button", { name: "Dark" }).click();
  assert.equal(await page.getByRole("group", { name: "Appearance mode" }).getByRole("button", { name: "Dark" }).getAttribute("aria-pressed"), "true");
  await page.locator("#theme-choice").selectOption("dracula");
  await page.getByRole("button", { name: "Accent #d79921" }).click();
  await page.locator("#background-choice").fill("#24212c");
  await page.locator("#font-choice").selectOption("mono");
  assert.equal(await page.locator("#theme-choice").inputValue(), "dracula");
  assert.equal(await page.locator(".app-root").evaluate(el => getComputedStyle(el).getPropertyValue("--app-bg").trim()), "#24212c");
  const pngDesktop = resolve(artifacts, "notebook-appearance-desktop.png");
  await page.screenshot({ path: pngDesktop, fullPage: true, animations: "disabled" });
  await page.reload({ waitUntil: "networkidle" });
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  assert.equal(await page.locator("#theme-choice").inputValue(), "dracula");
  assert.equal(await page.locator("#background-choice").inputValue(), "#24212c");
  assert.equal(await page.locator("#font-choice").inputValue(), "mono");
  await page.locator("details.advanced summary").click();
  await page.getByRole("button", { name: "Reset appearance to defaults" }).click();
  assert.equal(await page.locator("#theme-choice").inputValue(), "notebook-light");

  const mobile = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 1 });
  const smallPage = await mobile.newPage();
  await smallPage.goto("http://127.0.0.1:15423/", { waitUntil: "networkidle" });
  await smallPage.getByRole("button", { name: "Appearance", exact: true }).click();
  const dims = await smallPage.evaluate(() => ({
    page: document.documentElement.scrollWidth,
    viewport: document.documentElement.clientWidth,
  }));
  assert.ok(dims.page <= dims.viewport + 1, "Mobile horizontal overflow: " + JSON.stringify(dims));
  const darkButton = smallPage.getByRole("group", { name: "Appearance mode" }).getByRole("button", { name: "Dark" });
  const darkBounds = await darkButton.boundingBox();
  assert.ok(darkBounds && darkBounds.x >= 0 && darkBounds.x + darkBounds.width <= 390, "Dark mode button must fit the viewport.");
  await darkButton.click();
  assert.equal(await darkButton.getAttribute("aria-pressed"), "true");
  const pngMobile = resolve(artifacts, "notebook-appearance-mobile.png");
  await smallPage.screenshot({ path: pngMobile, fullPage: true, animations: "disabled" });
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({
    status: "PASS",
    checked: ["workbench render", "encode invoke", "decode invoke", "exact intermediate and speculative English separation", "candidate alternatives", "editable reconstruction", "error feedback", "mode selection", "Dracula palette", "accent override", "background override", "font selection", "settings persistence", "reset", "mobile mode controls", "mobile layout"],
    screenshots: [
      { path: pngDesktop, bytes: (await stat(pngDesktop)).size },
      { path: pngWorkbench, bytes: (await stat(pngWorkbench)).size },
      { path: pngDecode, bytes: (await stat(pngDecode)).size },
      { path: pngMobile, bytes: (await stat(pngMobile)).size },
    ],
    bridge: "mocked (not native IPC acceptance)",
  }, null, 2));
} finally {
  await browser?.close();
  await server.close();
}
