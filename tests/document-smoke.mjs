// Real renderer, simulated native boundary. Rust tests cover actual file bytes,
// publication races, identifiers and complete-document reconstruction.
import assert from "node:assert/strict";
import { readFile, mkdir } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { chromium } from "playwright-core";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const recipes = await Promise.all(Array.from({ length: 14 }, (_, i) =>
  readFile(resolve(root, "recipes", `${String(i + 1).padStart(3, "0")}.json`), "utf8").then(JSON.parse)
));
const server = await createServer({ root, configFile: resolve(root, "vite.config.ts"), server: { host: "127.0.0.1", port: 15424, strictPort: true }, logLevel: "error" });
let browser;
try {
  await server.listen();
  assert.ok(process.env.CHROME_EXECUTABLE, "Set CHROME_EXECUTABLE to an installed Chromium-family browser.");
  browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_EXECUTABLE });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 } });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.addInitScript(fixtures => {
    const clone = value => structuredClone(value);
    const state = window.fileFixture = { calls: [], clipboard: [], next: 0, activities: {}, jobs: {}, callbacks: {}, listeners: {}, holdRun: false, readings: {}, experiments: JSON.parse(localStorage.getItem("file-experiments") ?? "[]"), savedRecipes: [], nextImportRecipe: null, saveCancel: false, saveError: false, openError: false };
    const fingerprint = async recipe => {
      const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify([recipe.schemaVersion, recipe.steps])));
      const value = [...new Uint8Array(digest)].map(b => b.toString(16).padStart(2, "0")).join("");
      return { algorithm: "SHA-256", version: recipe.schemaVersion, value, display: `CRYPT-R${recipe.schemaVersion}-` + value.slice(0, 16) };
    };
    const finish = result => {
      const id = "activity-" + ++state.next;
      return state.activities[id] = { id, state: "complete", phase: "Complete", processedBytes: "0", totalBytes: null, result, error: null };
    };
    const opened = () => finish({ kind: "opened", document: { id: "document-" + ++state.next, name: "notes.md", encoding: "UTF-8", bytes: "16449", digest: "a".repeat(64), bom: true, legacyCompatible: false } });
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async value => { state.clipboard.push(value); } } });
    state.emit = (event, payload) => { for (const [id, listener] of Object.entries(state.listeners)) if (listener.event === event) state.callbacks[listener.handler]?.({ event, id: Number(id), payload }); };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_event, id) => { delete state.listeners[id]; } };
    window.__TAURI_INTERNALS__ = {
      transformCallback: callback => { const id = ++state.next; state.callbacks[id] = callback; return id; },
      unregisterCallback: id => { delete state.callbacks[id]; },
      invoke: async (command, args) => {
        state.calls.push({ command, args: clone(args) });
        if (command === "plugin:event|listen") { const id = ++state.next; state.listeners[id] = args; return id; }
        if (command === "plugin:event|unlisten") { delete state.listeners[args.eventId]; return; }
        if (command === "builtin_recipes") return fixtures;
        if (command === "list_saved_recipes") return clone(state.savedRecipes);
        if (command === "recipe_fingerprint") return fingerprint(args.recipe);
        if (command === "save_recipe") { if (!/^user-[a-zA-Z0-9-]+$/.test(args.recipe.id)) throw new Error("Invalid saved recipe ID."); state.savedRecipes = [clone(args.recipe), ...state.savedRecipes.filter(recipe => recipe.id !== args.recipe.id)]; return clone(args.recipe); }
        if (command === "doc_open" || command === "doc_open_drop") { if (state.openError) throw { message: "Invalid UTF-8 source." }; return clone(opened()); }
        if (command === "doc_activity") return clone(state.activities[args.id]);
        if (["doc_discard_activity", "doc_discard_document", "doc_discard_job"].includes(command)) return;
        if (command === "doc_cancel_activity") { state.activities[args.id].state = "cancelled"; return; }
        if (command === "doc_preview") {
          let text, nextOffset, bytes, eof;
          if (args.documentId) { text = args.offset === "0" ? "é".repeat(8192) : "<script>window.sourceExecuted = true</script>\r\n😀"; nextOffset = args.offset === "0" ? "16384" : "16449"; bytes = "16449"; eof = args.offset !== "0"; }
          else { text = args.readingId ? "the quick brown fox" : args.stage === null ? "Exact result 😀\r\n" : "Captured stage é"; nextOffset = bytes = String(new TextEncoder().encode(text).length); eof = true; }
          return { text, offset: args.offset, nextOffset, bytes, eof };
        }
        if (command === "doc_start") {
          const id = "job-" + ++state.next;
          return clone(state.jobs[id] = { id, documentId: args.documentId, state: "running", phase: "Transforming text", processedBytes: "0", totalBytes: "16449", recipe: clone(args.recipe), fingerprint: await fingerprint(args.recipe), direction: args.direction, exact: !args.recipe.steps.some(s => s.enabled && ["phonetic_rotate", "remove_vowels", "collapse_doubles"].includes(s.rule.type)), stages: [{ label: "Captured stage", bytes: "22", lossy: false }], note: "Completed captured recipe." });
        }
        if (command === "doc_job") { const job = state.jobs[args.id]; if (!state.holdRun && job.state === "running") job.state = "complete"; return clone(job); }
        if (command === "doc_cancel_job") { state.jobs[args.id].state = "cancelled"; state.jobs[args.id].note = "File operation cancelled."; return; }
        if (command === "doc_save") {
          if (state.saveCancel) return null;
          if (state.saveError) throw { message: "Result publication failed.", published: [{ name: "result.txt.recipe.json" }], unverified: ["result.txt"] };
          return clone(finish({ kind: "saved", receipt: { artifacts: [{ name: "result.txt", bytes: "22", digest: "b".repeat(64) }] } }));
        }
        if (command === "resolve_recipe_identifier") {
          for (const recipe of fixtures) { const fp = await fingerprint(recipe); if ([fp.value, fp.display].includes(args.identifier)) return clone(recipe); }
          return null;
        }
        if (command === "doc_import") { const recipe = state.nextImportRecipe ? clone(state.nextImportRecipe) : { ...clone(fixtures[12]), id: "imported-session", name: "Imported document recipe" }; return clone(finish({ kind: "imported", companion: { kind: "recipe", recipe } })); }
        if (command === "list_experiments") return { entries: state.experiments.map(value => value.summary), problems: [] };
        if (command === "save_file_experiment") {
          if (Object.keys(args).sort().join(",") !== "jobId,name,readingId") throw new Error("Only captured job/reading references may be saved.");
          const job = clone(state.jobs[args.jobId]);
          const reading = args.readingId ? clone(state.readings[args.readingId]) : null;
          const summary = { id: "experiment-" + ++state.next, name: args.name, kind: "file", recipeName: job.recipe.name, createdAt: "1794432000" };
          state.experiments.push({ summary, job, reading });localStorage.setItem("file-experiments", JSON.stringify(state.experiments));return summary;
        }
        if (command === "open_experiment") {
          const saved = state.experiments.find(value => value.summary.id === args.id);
          const id = "job-restored-" + ++state.next;
          const document = { id: "document-restored-" + ++state.next, name: "notes.md", encoding: "UTF-8", bytes: "16449", digest: "a".repeat(64), bom: true, legacyCompatible: false };
          const job = { ...clone(saved.job), id, documentId: document.id, restored: true };
          state.jobs[id] = job;
          const reading = saved.reading ? { ...saved.reading.info, id: "reading-restored-" + ++state.next, jobId: id } : null;
          const choices = (saved.reading?.choices ?? []).map(choice => ({ ...choice, wordId: id + ":" + choice.wordId.split(":").at(-1) }));
          return { kind: "file", summary: saved.summary, document, job, reading, choices };
        }
        if (command === "doc_reconstruct") {
          if (args.choices.some(choice => choice.candidate === "fax")) throw { message: "Choice cannot re-encode the complete file." };
          const reading = { id: "reading-" + ++state.next, jobId: args.jobId, words: "40", note: "Speculative reading verified by complete re-encoding." };
          state.readings[reading.id] = { info: clone(reading), choices: clone(args.choices) };
          return clone(finish({ kind: "reconstructed", reading }));
        }
        if (command === "doc_candidate_page") return { words: [{ wordId: args.jobId + ":" + (args.offset === "0" ? "0" : "32"), transformed: "fx", suggested: "fox", alternatives: ["fix", "fax"], compatibleCount: 3 }], nextOffset: "32", eof: args.offset !== "0" };
        throw new Error("Unexpected native command: " + command);
      },
    };
  }, recipes);
  await page.goto("http://127.0.0.1:15424/", { waitUntil: "networkidle" });
  const wait = (selector, pattern) => page.waitForFunction(({ selector, pattern }) => new RegExp(pattern).test(document.querySelector(selector)?.textContent ?? ""), { selector, pattern });
  const panel = page.locator(".document-panel");
  const run = page.getByRole("button", { name: "Run file recipe", exact: false });
  const ready = () => page.waitForFunction(() => !document.querySelector(".document-progress"));
  await page.locator("#source-input").fill("Keep my editable text.");
  await page.getByRole("button", { name: "File", exact: true }).click();
  await page.getByRole("button", { name: "Open file", exact: true }).click();
  await wait(".document-panel h1", "notes.md");
  assert.equal(await run.isDisabled(), true, "Legacy recipes cannot silently run Unicode files.");
  await panel.getByRole("button", { name: "Create UTF-8 recipe copy" }).click();
  await wait(".document-panel .document-fingerprint", "CRYPT-R2");
  await wait(".document-recipe-tools p", "Unsaved recipe");
  await panel.getByRole("button", { name: "Copy fingerprint", exact: true }).click();
  await panel.getByRole("button", { name: "Copy short code", exact: true }).click();
  const copied = await page.evaluate(() => window.fileFixture.clipboard);
  assert.match(copied[0], /^[a-f0-9]{64}$/); assert.match(copied[1], /^CRYPT-R2-[a-f0-9]{16}$/);
  const firstPage = panel.getByLabel("Source preview text", { exact: true });
  assert.equal((await firstPage.innerText()).length, 8192);
  await panel.getByRole("button", { name: "Next Source preview page", exact: true }).click();
  await wait('[aria-label="Source preview text"]', "<script>");
  assert.equal(await page.evaluate(() => window.sourceExecuted), undefined, "Source is plain preview text.");
  assert.equal(await panel.locator("script").count(), 0);
  await panel.getByRole("button", { name: "Previous Source preview page", exact: true }).click();
  await page.getByRole("button", { name: "Text", exact: true }).click();
  assert.equal(await page.locator("#source-input").inputValue(), "Keep my editable text.");
  await page.getByRole("button", { name: "File", exact: true }).click();
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  await page.getByRole("button", { name: "Workbench", exact: true }).click();
  assert.equal(await panel.locator("h1").innerText(), "notes.md", "Source survives navigation.");
  await run.click(); await wait(".document-result h2", "Encoded result"); await ready();
  assert.equal(await panel.getByLabel("Result preview text", { exact: true }).innerText(), "Exact result 😀\r\n");
  await panel.locator("#file-stage").selectOption("0");
  await wait('[aria-label="Result preview text"]', "Captured stage");
  await panel.locator("#file-stage").selectOption("result");
  for (const [label, expected] of [["Recipe JSON", "json"], ["Fingerprint text instead of JSON", "fingerprint"], ["No companion", "none"]]) {
    await panel.getByLabel(label, { exact: true }).check();
    await panel.getByRole("button", { name: "Save As…", exact: true }).click(); await ready();
    const call = await page.evaluate(() => window.fileFixture.calls.filter(c => c.command === "doc_save").at(-1));
    assert.equal(call.args.companion, expected); assert.equal(call.args.readingId, null);
  }
  await page.locator("#operation-mode").selectOption("decode");
  await wait(".document-stale", "Previous result");
  await page.evaluate(() => { window.fileFixture.holdRun = true; });
  await run.click(); await panel.getByRole("button", { name: "Cancel operation" }).click(); await ready();
  assert.equal(await panel.locator(".document-result h2").innerText(), "Encoded result", "Cancelling retains prior result.");
  await page.evaluate(() => { window.fileFixture.holdRun = false; });
  const fp = await page.evaluate(async () => (await window.__TAURI_INTERNALS__.invoke("recipe_fingerprint", { recipe: (await window.__TAURI_INTERNALS__.invoke("builtin_recipes"))[12] })).display);
  await panel.locator("#file-recipe-identifier").fill(fp); await panel.getByRole("button", { name: "Find recipe", exact: true }).click();
  const dialog = page.locator(".document-confirm"); await dialog.waitFor({ state: "visible" });
  await page.keyboard.press("Escape");
  assert.match(await panel.locator(".document-recipe-tools p").innerText(), /Unsaved recipe/);
  await panel.getByRole("button", { name: "Find recipe", exact: true }).click();
  await dialog.getByRole("button", { name: "Discard changes" }).click();
  await wait(".document-recipe-tools p", "^UTF-8 document$");
  await run.click(); await wait(".document-result h2", "Exact decoded text"); await ready();
  await page.getByRole("button", { name: "Open file", exact: true }).click();
  await dialog.waitFor({ state: "visible" }); await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  assert.equal(await panel.locator(".document-result").count(), 1);
  await page.evaluate(() => { window.fileFixture.saveCancel = true; });
  await page.getByRole("button", { name: "Open file", exact: true }).click(); await dialog.getByRole("button", { name: "Save first" }).click(); await ready();
  assert.equal(await panel.locator(".document-result").count(), 1, "Cancelled Save As cancels replacement.");
  await page.evaluate(() => { window.fileFixture.saveCancel = false; window.fileFixture.saveError = true; });
  await panel.getByRole("button", { name: "Save As…", exact: true }).click(); await ready();
  await wait(".document-notice", "Already saved:.*Check these files before retrying:");
  await page.evaluate(() => { window.fileFixture.saveError = false; });
  await panel.locator("#file-recipe-identifier").fill("CRYPT-R2-" + "f".repeat(16)); await panel.getByRole("button", { name: "Find recipe", exact: true }).click();
  await panel.getByRole("button", { name: "Import recipe JSON", exact: true }).click(); await ready();
  await wait(".document-recipe-tools p", "Imported document recipe.*Unsaved recipe");
  const saves = await page.evaluate(() => window.fileFixture.calls.filter(c => c.command === "save_recipe"));
  assert.equal(saves.length, 0, "Imported recipes stay session-local until explicitly saved.");
  // Select a known lossy recipe through the same fingerprint lookup.
  const lossy = await page.evaluate(async () => (await window.__TAURI_INTERNALS__.invoke("recipe_fingerprint", { recipe: (await window.__TAURI_INTERNALS__.invoke("builtin_recipes"))[0] })).display);
  await panel.locator("#file-recipe-identifier").fill(lossy); await panel.getByRole("button", { name: "Find recipe", exact: true }).click(); await dialog.getByRole("button", { name: "Discard changes" }).click();
  await panel.getByRole("button", { name: "Create UTF-8 recipe copy" }).click();
  await run.click(); await dialog.getByRole("button", { name: "Discard changes" }).click();
  await wait(".document-result h2", "Exact surviving intermediate"); await ready();
  await panel.getByRole("button", { name: "Suggest an English reading" }).click(); await wait('[aria-label="Speculative English reading"] h2', "Speculative"); await ready();
  const readingId = await page.evaluate(() => window.fileFixture.calls.filter(c => c.command === "doc_preview" && c.args.readingId).at(-1).args.readingId);
  await panel.getByRole("button", { name: "Next words", exact: true }).click(); await wait(".document-word strong", "Word 33");
  await panel.getByRole("button", { name: "fax", exact: true }).click(); await ready();
  await wait(".document-notice", "Reading not applied");
  assert.equal(await panel.getByLabel("English preview text", { exact: true }).innerText(), "the quick brown fox");
  await panel.locator("#file-save-output").selectOption("reading");
  await panel.getByRole("button", { name: "Save As…", exact: true }).click(); await ready();
  assert.equal(await page.evaluate(() => window.fileFixture.calls.filter(c => c.command === "doc_save").at(-1).args.readingId), readingId);
  await panel.getByRole("button", { name: "fix", exact: true }).click();await ready();
  await panel.getByRole("textbox", { name: "Experiment name", exact: true }).fill("Document reading");
  await panel.getByRole("button", { name: "Save experiment", exact: true }).click();
  await panel.getByText("Saved “Document reading” in Experiments.", { exact: true }).waitFor();
  await page.reload({ waitUntil: "networkidle" });
  await page.getByRole("button", { name: "Experiments", exact: true }).click();
  await page.getByRole("button", { name: "Open Document reading", exact: true }).click();
  await wait(".document-panel h1", "notes.md");
  assert.equal(await page.locator("#passphrase").inputValue(), "");
  assert.equal(await panel.getByRole("button", { name: "Suggest an English reading", exact: true }).isDisabled(), true);
  assert.equal(await panel.getByLabel("English preview text", { exact: true }).innerText(), "the quick brown fox");
  await panel.getByRole("button", { name: "Next words", exact: true }).click();await wait(".document-word strong", "Word 33");
  assert.equal(await panel.getByRole("button", { name: "fix", exact: true }).getAttribute("aria-pressed"), "true");
  await page.locator("#passphrase").fill("spectacular");
  await panel.getByRole("button", { name: "fox", exact: true }).click();await ready();
  assert.equal(await page.evaluate(() => window.fileFixture.calls.filter(call => call.command === "doc_reconstruct").at(-1).args.key), "spectacular");
  await panel.getByRole("button", { name: "Save As…", exact: true }).click();await ready();
    // A failed drop leaves the usable result and snapshot in place.
  await page.evaluate(() => { window.fileFixture.openError = true; window.fileFixture.emit("document-drop", { token: "owned-drop", name: "bad.md" }); });
  await wait(".document-notice", "Invalid UTF-8");
  assert.equal(await panel.locator(".document-result").count(), 1);
  await mkdir(resolve(root, "test-artifacts"), { recursive: true });
  await page.screenshot({ path: resolve(root, "test-artifacts/document-notebook.png"), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, "File mode fits narrow windows.");
  await page.screenshot({ path: resolve(root, "test-artifacts/document-notebook-mobile.png"), fullPage: true });

  // A companion identifies a portable configuration, not an existing local
  // storage slot. Saving it must preserve a colliding saved recipe.
  await page.setViewportSize({ width: 1440, height: 960 });
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  await page.locator("#recipe-name").fill("Original local recipe");
  await page.getByRole("button", { name: "Save recipe", exact: true }).click();
  await wait(".document-recipe-tools p", "^Original local recipe$");
  const original = await page.evaluate(() => structuredClone(window.fileFixture.savedRecipes.find(recipe => recipe.name === "Original local recipe")));
  assert.ok(original);
  for (const [sourceId, name] of [[original.id, "Imported collision recipe"], ["portable-external", "Imported portable recipe"]]) {
    const imported = { ...original, id: sourceId, name, steps: [{ enabled: true, rule: { type: "caesar", shift: 5 } }] };
    const expectedFingerprint = await page.evaluate(async recipe => {
      window.fileFixture.nextImportRecipe = recipe;
      return window.__TAURI_INTERNALS__.invoke("recipe_fingerprint", { recipe });
    }, imported);
    await panel.locator("#file-recipe-identifier").fill("");
    await panel.getByRole("button", { name: "Import recipe or fingerprint file", exact: true }).click();
    await ready();
    await wait(".document-recipe-tools p", name + ".*Unsaved recipe");
    assert.deepEqual(await page.evaluate(id => window.fileFixture.savedRecipes.find(recipe => recipe.id === id), original.id), original, "Import alone preserves the saved recipe.");
    await page.getByRole("button", { name: "Recipes", exact: true }).click();
    await page.getByRole("button", { name: "Save recipe", exact: true }).click();
    await wait(".document-recipe-tools p", "^" + name + "$");
    const saved = await page.evaluate(() => structuredClone(window.fileFixture.savedRecipes));
    assert.deepEqual(saved.find(recipe => recipe.id === original.id), original, "Saving a companion must never replace the original local recipe.");
    const added = saved.find(recipe => recipe.name === name);
    assert.ok(added, "The imported configuration is saved separately.");
    assert.match(added.id, /^user-/);
    assert.notEqual(added.id, sourceId, "Companion IDs must not become local storage identities.");
    assert.deepEqual(added.steps, imported.steps);
    await wait(".document-fingerprint", "^" + expectedFingerprint.display + "$");
  }
  assert.equal(await page.evaluate(() => window.fileFixture.savedRecipes.length), 3);

  assert.deepEqual(errors, []);
  console.log("PASS: file open/drop, UTF-8 selection, paging, copy, captured results, companions, cancellation, replacement, lookup/import and separate English reading");
} finally { await browser?.close(); await server.close(); }
