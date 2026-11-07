// Isolated frontend acceptance: the Tauri invoke bridge is mocked.
// Rust cipher semantics and native window acceptance are tested separately.
import assert from "node:assert/strict";
import { mkdir, stat, readFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const artifacts = resolve(root, "test-artifacts");
await mkdir(artifacts, { recursive: true });
const recipeFixtures = await Promise.all(
  Array.from({ length: 11 }, (_, i) => String(i + 1).padStart(3, "0")).map(async number =>
    JSON.parse(await readFile(resolve(root, "recipes", number + ".json"), "utf8"))
  )
);
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
  await page.addInitScript((fixtures) => {
    let saved = JSON.parse(localStorage.getItem("test-saved-recipes") ?? "[]");
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command === "builtin_recipes") return fixtures;
        if (command === "list_saved_recipes") return saved;
        if (command === "save_recipe") {
          saved = [args.recipe, ...saved.filter(r => r.id !== args.recipe.id)];
          localStorage.setItem("test-saved-recipes", JSON.stringify(saved));
          return args.recipe;
        }
        if (command === "delete_saved_recipe") {
          saved = saved.filter(r => r.id !== args.id);
          localStorage.setItem("test-saved-recipes", JSON.stringify(saved));
          return null;
        }
        if (command !== "evaluate_recipe") throw new Error("Unexpected command: " + command);
        const { recipe, direction } = args;
        const keyRequired = recipe.steps.some(s => s.enabled &&
          (s.rule.type === "phonetic_rotate" ||
            (s.rule.type === "position_shift" && s.rule.keyed) ||
            ["vigenere", "beaufort"].includes(s.rule.type)));
        if (keyRequired && !["spectacular", "lemon"].includes(args.key)) throw new Error("Test bridge: incorrect key.");
        const vigenere = recipe.steps.some(s => s.enabled && s.rule.type === "vigenere");
        const railFence = recipe.steps.some(s => s.enabled && s.rule.type === "rail_fence");
        const mirror = recipe.steps.some(s => s.enabled && s.rule.type === "word_mirror");
        const phonetic = recipe.steps.some(s => s.enabled && s.rule.type === "phonetic_rotate");
        const skeleton = recipe.steps.some(s => s.enabled && s.rule.type === "remove_vowels");
        if (direction === "encode") {
          const output = mirror ? "mirrorfixture" : skeleton ? "skeletonfixture" :
            phonetic ? "fixturecipher" : vigenere ? "vigenerefixture" :
            railFence ? "railfixture" : "pairfixture";
          return {
            output, intermediate: output, exact: !phonetic && !skeleton,
            reconstruction: null,
            note: phonetic || skeleton ? "Lossy encoding." : "Exact encoding.",
            stages: recipe.steps.filter(s => s.enabled).map(s => ({
              label: s.rule.type, text: s.rule.type === "phonetic_rotate" ? "fixtureintermediate" : output,
              lossy: ["phonetic_rotate", "remove_vowels", "collapse_doubles"].includes(s.rule.type),
            })),
          };
        }
        if (!phonetic && !skeleton) {
          return {
            output: "The quick brown fox jumps over the lazy dog.", intermediate: "The quick brown fox jumps over the lazy dog.",
            exact: true, reconstruction: null, note: "Exact reverse.",
            stages: [{label:"Undo shift",text:"The quick brown fox jumps over the lazy dog.",lossy:false}],
          };
        }
        const picked = args.choices?.find(choice => choice.index === 3)?.candidate ?? "fix";
        if (picked === "fax") throw new Error("Candidate conflicts with remaining rotations.");
        if (!["fox", "fix"].includes(picked)) throw new Error("Invalid candidate.");
        const suggested = "the quick brown " + picked + " jumps over the lazy dog";
        const words = ["the","quick","brown",picked,"jumps","over","the","lazy","dog"].map((word,index)=>({
          transformed: ["the","qwhik","brogn","fucc","jammpsc","evi","dho","luza","degg"][index],
          suggested:word,
          alternatives:index===3?(picked==="fox"?["fix","fax"]:["fox","fax"]):[],
          compatibleCount:index===3?3:1,
        }));
        return {
          output:suggested, intermediate:"fixtureintermediate", exact:false,
          reconstruction:{suggested,note:"Speculative expansion. 1 word has several valid matches.",words},
          note:"Speculative expansion. 1 word has several valid matches.",
          stages:[{label:"Undo shift",text:"fixtureintermediate",lossy:false}],
        };
      },
    };
  }, recipeFixtures);

  await page.goto("http://127.0.0.1:15423/", { waitUntil: "networkidle" });
  assert.equal(await page.title(), "Cryptography Playground");
  assert.match(await page.locator("h1").innerText(), /Experiment: Phonetic Rotation/);
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  const pngRecipes = resolve(artifacts, "recipe-library-and-builder.png");
  await page.screenshot({ path: pngRecipes, fullPage: true, animations: "disabled" });
  assert.equal(await page.locator(".recipe-library-entry").count(), 11);
  await page.getByRole("button", { name: /Vigenere/ }).click();
  assert.equal(await page.locator("#rule-0").inputValue(), "vigenere");
  assert.equal(await page.locator("#rule-0 optgroup[label='Classical substitution (cifers)']").count(), 1);
  await page.getByRole("button", { name: "Use in workbench" }).click();
  await page.locator("#passphrase").fill("");
  assert.equal(await page.getByRole("button", { name: /Encode/ }).isDisabled(), true);
  await page.locator("#passphrase").fill("lemon");
  await page.getByRole("button", { name: /Encode/ }).click();
  assert.equal(await page.locator("#cipher-result").innerText(), "vigenerefixture");
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  await page.getByRole("button", { name: /Affine Substitution/ }).click();
  await page.locator("#affine-a-0").selectOption("11");
  await page.locator("#affine-b-0").fill("7");
  assert.match(await page.locator(".recipe-edit-status").innerText(), /Unsaved changes/);
  const pngClassical = resolve(artifacts, "classical-affine-rule-editor.png");
  await page.screenshot({ path: pngClassical, fullPage: true, animations: "disabled" });
  await page.getByRole("button", { name: "Save a copy" }).click();
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  await page.getByRole("button", { name: /Affine Substitution \(custom\)/ }).click();
  assert.equal(await page.locator("#affine-a-0").inputValue(), "11");
  assert.equal(await page.locator("#affine-b-0").inputValue(), "7");
  await page.getByRole("button", { name: /Rail Fence/ }).click();
  await page.locator("#rail-count-0").fill("4");
  await page.getByRole("button", { name: "Use in workbench" }).click();
  assert.match(await page.locator(".recipe-save-state").innerText(), /Unsaved recipe/);
  await page.locator("#passphrase").fill("");
  await page.getByRole("button", { name: /Encode/ }).click();
  assert.equal(await page.locator("#cipher-result").innerText(), "railfixture");
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  await page.getByRole("button", { name: /Word Mirror/ }).click();
  assert.match(await page.locator(".recipe-edit-status").innerText(), /Bundled example/);
  await page.locator("#shift-base-1").fill("7");
  assert.match(await page.locator(".recipe-edit-status").innerText(), /Unsaved changes/);
  await page.getByRole("button", { name: "Use in workbench" }).click();
  assert.match(await page.locator(".recipe-save-state").innerText(), /Unsaved recipe/);
  assert.match(await page.locator(".recipe-save-state").innerText(), /restarting/);
  await page.getByRole("button", { name: "Save this recipe" }).click();
  assert.match(await page.locator(".recipe-edit-status").innerText(), /Unsaved changes/);
  await page.getByRole("button", { name: "Save a copy" }).click();
  assert.match(await page.locator(".recipe-save-state").innerText(), /Saved locally on this computer/);
  await page.locator("#passphrase").fill("");
  assert.equal(await page.getByRole("button", { name: /Encode/ }).isDisabled(), false);
  await page.getByRole("button", { name: /Encode/ }).click();
  assert.equal(await page.locator("#cipher-result").innerText(), "mirrorfixture");
  await page.locator("#operation-mode").selectOption("decode");
  await page.getByRole("button", { name: /Decode exactly/ }).click();
  assert.equal(await page.locator("#cipher-result").innerText(), "The quick brown fox jumps over the lazy dog.");
  assert.equal(await page.locator("#intermediate-result").count(), 0, "Exact decode must not duplicate the output");
  assert.equal(await page.locator(".result-grid > div").count(), 1);
  assert.match(await page.locator(".inspector-exact-status").innerText(), /Exact decoded result/);
  assert.equal(await page.locator(".inspector-tabs").count(), 0);
  assert.match(await page.locator(".data-rows").innerText(), /Word Mirror/);
  assert.match(await page.getByRole("button", { name: "Copy decoded text" }).innerText(), /Copy decoded text/);
  assert.equal(await page.locator(".reconstruction-warning").count(), 0);
  const pngExact = resolve(artifacts, "notebook-exact-decode-no-duplicates.png");
  await page.screenshot({ path: pngExact, fullPage: true, animations: "disabled" });
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  await page.getByRole("button", { name: "Create a new recipe" }).click();
  assert.match(await page.locator(".recipe-edit-status").innerText(), /Unsaved recipe/);
  await page.locator("#recipe-name").fill("My Custom Cipher");
  await page.locator("#rule-0").selectOption("remove_vowels");
  await page.getByRole("button", { name: "Save recipe" }).click();
  assert.match(await page.locator(".recipe-save-state").innerText(), /Saved locally on this computer/);
  await page.reload({ waitUntil: "networkidle" });
  assert.match(await page.locator("h1").innerText(), /Experiment: My Custom Cipher/);
  assert.match(await page.locator(".recipe-save-state").innerText(), /Saved locally on this computer/);
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  assert.ok(await page.getByRole("button", { name: /My Custom Cipher/ }).count() > 0);
  await page.getByRole("button", { name: /My Custom Cipher/ }).click();
  assert.equal(await page.locator("#rule-0").inputValue(), "remove_vowels");
  await page.getByRole("button", { name: "Duplicate" }).click();
  await page.getByRole("button", { name: "Add rule" }).click();
  assert.equal(await page.locator(".recipe-step").count(), 2);
  await page.locator("#rule-1").selectOption("position_shift");
  await page.locator("#shift-base-1").fill("3");
  await page.getByRole("checkbox", { name: "Use passphrase in shift 2" }).check();
  await page.getByRole("button", { name: "Move rule 2 up" }).click();
  assert.equal(await page.locator("#rule-0").inputValue(), "position_shift");
  assert.equal(await page.locator("#shift-base-0").inputValue(), "3");
  await page.getByRole("button", { name: "Save recipe" }).click();
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  assert.ok(await page.getByRole("button", { name: /My Custom Cipher \(copy\)/ }).count() > 0);

  await page.getByRole("button", { name: "Import JSON" }).click();
  const imported = {
    schemaVersion: 1, id: "ignored-import-id", name: "Imported Mirror",
    description: "Portable recipe import example.",
    steps: [{ enabled: true, rule: { type: "word_mirror" } }],
  };
  await page.locator("#recipe-json").fill(JSON.stringify(imported));
  await page.getByRole("button", { name: "Import and save locally" }).click();
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  await page.getByRole("button", { name: /Imported Mirror/ }).click();
  assert.equal(await page.locator("#rule-0").inputValue(), "word_mirror");
  page.on("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Delete saved recipe" }).click();
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  assert.equal(await page.getByRole("button", { name: /Imported Mirror/ }).count(), 0);
  await page.getByRole("button", { name: /Phonetic Rotation/ }).click();
  await page.getByRole("button", { name: "Use in workbench" }).click();
  await page.locator("#passphrase").fill("spectacular");
  await page.locator("#operation-mode").selectOption("encode");

  await page.getByRole("button", { name: /Encode/ }).click();
  assert.equal(await page.locator("#cipher-result").innerText(), "fixturecipher");
  assert.equal(await page.locator("#intermediate-result").innerText(), "fixtureintermediate");
  const pngWorkbench = resolve(artifacts, "notebook-workbench-desktop.png");
  await page.screenshot({ path: pngWorkbench, fullPage: true, animations: "disabled" });
  await page.locator("#operation-mode").selectOption("decode");
  await page.getByRole("button", { name: /Decode \/ suggest English/ }).click();
  assert.equal(await page.locator("#intermediate-result").innerText(), "fixtureintermediate");
  assert.equal(await page.locator("#cipher-result").innerText(), "the quick brown fix jumps over the lazy dog");
  // Click an ambiguous word in the sentence, select a different candidate,
  // then verify the backend results update the entire phrase and inspector.
  await page.getByRole("button", { name: "Choose a different word for fix, word 4" }).click();
  await page.getByRole("group", { name: "Choose word 4" }).getByRole("button", { name: "Use fox for word 4" }).click();
  await page.waitForFunction(() =>
    document.querySelector("#cipher-result")?.textContent?.includes("brown fox jumps"));
  assert.equal(await page.locator("#cipher-result").innerText(), "the quick brown fox jumps over the lazy dog");
  assert.equal(await page.locator("#intermediate-result").innerText(), "fixtureintermediate");
  assert.equal(await page.getByRole("group", { name: "Candidates for word 4 (fucc)" })
    .getByRole("button", { name: "Use fox for word 4" }).getAttribute("aria-pressed"), "true");
  // A rejected alternative must roll back the visual preview, not corrupt it.
  await page.getByRole("group", { name: "Candidates for word 4 (fucc)" })
    .getByRole("button", { name: "Use fax for word 4" }).click();
  await page.waitForFunction(() =>
    document.querySelector('[role="status"]')?.textContent?.includes("Choice not applied"));
  assert.equal(await page.locator("#cipher-result").innerText(), "the quick brown fox jumps over the lazy dog");
  await page.getByRole("button", { name: /Reset word selections/ }).click();
  await page.waitForFunction(() =>
    document.querySelector("#cipher-result")?.textContent?.includes("brown fix jumps"));
  assert.equal(await page.locator("#cipher-result").innerText(), "the quick brown fix jumps over the lazy dog");
  assert.match(await page.locator(".reconstruction-warning").innerText(), /cannot be proven|not verified|cannot verify/);
  assert.match(await page.locator(".reconstruction-word").innerText(), /fox|fax/);
  const pngDecode = resolve(artifacts, "notebook-decoder-english-suggestion.png");
  await page.screenshot({ path: pngDecode, fullPage: true, animations: "disabled" });
  const editableEnglish = page.getByRole("textbox", { name: "Edit suggested English" });
  await editableEnglish.fill("the quick brown fox jumps over the lazy dog");
  assert.equal(await page.locator("#cipher-result").innerText(), "the quick brown fox jumps over the lazy dog");
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
    checked: [ "classical recipe catalog", "Vigenere passphrase requirement", "Affine configuration and saved copy", "Rail Fence parameter editing", "recipe selection", "reversible exact decode", "recipe-specific inspector labels", "no duplicate exact output", "unsaved recipe warning", "save from workbench", "save custom recipe", "persist recipe after reload", "import JSON recipe", "delete saved recipe", "duplicate and reorder rules", "configure shift parameters", "workbench render", "encode invoke", "decode invoke", "exact intermediate and speculative English separation", "candidate alternatives", "inline word selection", "inspector candidate buttons", "rejected-choice rollback", "reset choices", "editable reconstruction", "error feedback", "mode selection", "Dracula palette", "accent override", "background override", "font selection", "settings persistence", "reset", "mobile mode controls", "mobile layout"],
    screenshots: [
      { path: pngDesktop, bytes: (await stat(pngDesktop)).size },
      { path: pngWorkbench, bytes: (await stat(pngWorkbench)).size },
      { path: pngRecipes, bytes: (await stat(pngRecipes)).size },
      { path: pngClassical, bytes: (await stat(pngClassical)).size },
      { path: pngExact, bytes: (await stat(pngExact)).size },
      { path: pngDecode, bytes: (await stat(pngDecode)).size },
      { path: pngMobile, bytes: (await stat(pngMobile)).size },
    ],
    bridge: "mocked (not native IPC acceptance)",
  }, null, 2));
} finally {
  await browser?.close();
  await server.close();
}
