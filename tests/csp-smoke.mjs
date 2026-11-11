// Production assets under the configured CSP; native IPC is simulated here.
// Native builds separately verify Tauri's generated hashes/nonces and Rust IPC.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, resolve, dirname, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer as createViteServer } from "vite";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = resolve(root, "dist");
const config = JSON.parse(await readFile(resolve(root, "src-tauri/tauri.conf.json"), "utf8"));
const serialize = policy => typeof policy === "string" ? policy : Object.entries(policy ?? {})
  .map(([directive, sources]) => directive + " " + (Array.isArray(sources) ? sources.join(" ") : sources)).join("; ");
const header = serialize(config.app.security.csp);
const recipes = await Promise.all(Array.from({ length: 14 }, (_, i) =>
  readFile(resolve(root, "recipes", String(i + 1).padStart(3, "0") + ".json"), "utf8").then(JSON.parse)
));
const listen = server => new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", () => resolve("http://127.0.0.1:" + server.address().port));
});
const close = server => new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
const outsideRequests = [];
const outside = createServer((req, res) => {
  outsideRequests.push(req.url);
  res.writeHead(200, { "Content-Type": "text/javascript", "Access-Control-Allow-Origin": "*" });
  res.end("window.cspRemoteExecuted = true;");
});
let browser, devServer;
const server = createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
    const path = resolve(dist, "." + (pathname === "/" ? "/index.html" : pathname));
    if (!path.startsWith(dist + sep)) { res.writeHead(403).end(); return; }
    const content = await readFile(path);
    const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png" };
    res.writeHead(200, { "Content-Type": types[extname(path)] ?? "application/octet-stream",
      ...(header ? { "Content-Security-Policy": header } : {}) });
    res.end(content);
  } catch {
    res.writeHead(404).end();
  }
});
try {
  const [origin, outsideOrigin] = await Promise.all([listen(server), listen(outside)]);
  assert.ok(process.env.CHROME_EXECUTABLE, "Set CHROME_EXECUTABLE to an installed Chromium-family browser.");
  browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_EXECUTABLE });
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 } });
  const errors = [];
  await page.addInitScript(fixtures => {
    window.cspViolations = [];
    document.addEventListener("securitypolicyviolation", event => window.cspViolations.push({
      directive: event.effectiveDirective, blocked: event.blockedURI,
    }));
    window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
      if (command === "builtin_recipes") return fixtures;
      if (command === "list_saved_recipes") return [];
      if (command === "recipe_fingerprint") return { algorithm: "SHA-256", version: 1, value: "a".repeat(64), display: "CRYPT-R1-" + "a".repeat(16) };
      if (command === "evaluate_recipe") return { output: "policy fixture", intermediate: "policy fixture",
        exact: true, reconstruction: null, note: "Exact encoding.", stages: [] };
      throw new Error("Unexpected native command: " + command);
    } };
  }, recipes);
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
  await page.goto(origin, { waitUntil: "networkidle" });
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  assert.equal(await page.locator(".recipe-library-entry").count(), 14);
  await page.getByRole("button", { name: /Vigenere/ }).click();
  await page.getByRole("button", { name: "Use in workbench" }).click();
  await page.locator("#passphrase").fill("lemon");
  await page.getByRole("button", { name: /Encode/ }).click();
  assert.equal(await page.locator("#cipher-result").innerText(), "policy fixture");
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  await page.getByRole("group", { name: "Appearance mode" }).getByRole("button", { name: "Dark" }).click();
  await page.locator("#theme-choice").selectOption("dracula");
  await page.getByRole("button", { name: "Accent #d79921" }).click();
  await page.locator("#background-choice").fill("#24212c");
  await page.locator("#font-choice").selectOption("mono");
  const appearance = await page.locator(".app-root").evaluate(element => {
    const style = getComputedStyle(element);
    return { background: style.getPropertyValue("--app-bg").trim(), accent: style.getPropertyValue("--app-accent").trim(), font: style.fontFamily };
  });
  assert.equal(appearance.background, "#24212c");
  assert.equal(appearance.accent, "#d79921");
  assert.match(appearance.font, /mono/i);
  assert.deepEqual(errors, [], "Bundled UI must work without CSP violations.");
  assert.deepEqual(await page.evaluate(() => window.cspViolations), []);

  // Insertion through DOM APIs is subject to CSP even though the test driver
  // can execute its own code through DevTools.
  await page.evaluate(() => {
    const script = document.createElement("script");
    script.textContent = "window.cspInlineExecuted = true";
    document.body.append(script);
  });
  assert.equal(await page.evaluate(() => window.cspInlineExecuted ?? false), false,
    "An injected inline script must not execute.");

  await page.evaluate(outsideOrigin => {
    const handler = document.createElement("button");
    handler.setAttribute("onclick", "window.cspHandlerExecuted = true");
    document.body.append(handler);
    handler.click();
    const external = document.createElement("script");
    external.src = outsideOrigin + "/script.js";
    document.body.append(external);
    void fetch(outsideOrigin + "/fetch").catch(() => {});
    const socket = new WebSocket(outsideOrigin.replace("http:", "ws:") + "/socket");
    socket.onerror = () => {};
    const image = new Image();
    image.src = outsideOrigin + "/image.png";
    const frame = document.createElement("iframe");
    frame.src = outsideOrigin + "/frame.html";
    document.body.append(frame);
    const style = document.createElement("style");
    style.textContent = ":root { --csp-injected-style: applied; }";
    document.head.append(style);
    const styled = document.createElement("div");
    styled.id = "csp-style-attribute";
    styled.setAttribute("style", "--csp-injected-attribute: applied");
    document.body.append(styled);
    const base = document.createElement("base");
    base.href = outsideOrigin + "/";
    document.head.append(base);
    const object = document.createElement("object");
    object.data = outsideOrigin + "/object";
    object.type = "text/html";
    document.body.append(object);
    const blobUrl = URL.createObjectURL(new Blob(["postMessage('unexpected')"], { type: "text/javascript" }));
    try {
      const worker = new Worker(blobUrl);
      worker.onerror = () => worker.terminate();
    } catch {}
    URL.revokeObjectURL(blobUrl);
    const form = document.createElement("form");
    form.action = outsideOrigin + "/submit";
    document.body.append(form);
    form.requestSubmit();
  }, outsideOrigin);
  const required = ["script-src-elem", "script-src-attr", "connect-src", "img-src", "frame-src",
    "style-src-elem", "style-src-attr", "base-uri", "object-src", "worker-src", "form-action"];
  await page.waitForFunction(required => required.every(directive =>
    window.cspViolations.some(violation => violation.directive === directive)), required);
  assert.equal(await page.evaluate(() => window.cspHandlerExecuted ?? false), false);
  assert.equal(await page.evaluate(() => window.cspRemoteExecuted ?? false), false);
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--csp-injected-style")), "");
  assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById("csp-style-attribute")).getPropertyValue("--csp-injected-attribute")), "");
  assert.equal(page.url(), origin + "/");
  assert.deepEqual(outsideRequests, [], "Blocked resources must never reach the other origin.");

  // Run the eval probe from a trusted same-origin script, rather than from
  // DevTools evaluation (which can bypass the unsafe-eval restriction).
  await page.route(origin + "/eval-probe.js", route => route.fulfill({ contentType: "text/javascript",
    body: "try { Function('window.cspEvalExecuted = true')(); } catch (error) { window.cspEvalError = error.name; } window.cspEvalDone = true;" }));
  await page.evaluate(() => {
    const script = document.createElement("script");
    script.src = "/eval-probe.js";
    document.body.append(script);
  });
  await page.waitForFunction(() => window.cspEvalDone === true);
  assert.equal(await page.evaluate(() => window.cspEvalExecuted ?? false), false);
  assert.equal(await page.evaluate(() => window.cspEvalError), "EvalError");
  await page.waitForFunction(() => window.cspViolations.some(violation => violation.blocked === "eval"));
  console.log("CSP acceptance passed: production UI/themes and blocked script, eval, network, frame, object, worker, base and form probes.");

  // Use the actual development policy with Vite/React refresh, including
  // a delivered HMR event, so production hardening cannot silently break dev.
  const devHeader = serialize(config.app.security.devCsp);
  devServer = await createViteServer({ root, configFile: resolve(root, "vite.config.ts"),
    server: { host: "127.0.0.1", port: 1420, strictPort: true,
      headers: { "Content-Security-Policy": devHeader } }, logLevel: "error" });
  await devServer.listen();
  errors.length = 0;
  await page.goto("http://127.0.0.1:1420/", { waitUntil: "networkidle" });
  await page.getByRole("button", { name: "Recipes", exact: true }).click();
  assert.equal(await page.locator(".recipe-library-entry").count(), 14);
  await page.evaluate(async () => {
    const { createHotContext } = await import("/@vite/client");
    createHotContext("/csp-test").on("csp-acceptance", data => { window.cspHotReload = data; });
  });
  devServer.ws.send({ type: "custom", event: "csp-acceptance", data: "delivered" });
  await page.waitForFunction(() => window.cspHotReload === "delivered");
  assert.deepEqual(errors, [], "Development startup and hot reload must not violate CSP.");
  assert.deepEqual(await page.evaluate(() => window.cspViolations), []);
  await page.evaluate(() => {
    const button = document.createElement("button");
    button.setAttribute("onclick", "window.cspDevHandlerExecuted = true");
    button.click();
  });
  assert.equal(await page.evaluate(() => window.cspDevHandlerExecuted ?? false), false);
  await page.waitForFunction(() => window.cspViolations.some(violation => violation.directive === "script-src-attr"));
  console.log("CSP development acceptance passed: Vite/React refresh and hot reload, with event handlers still blocked.");
} finally {
  await devServer?.close();
  await browser?.close();
  await Promise.all([close(server), close(outside)]);
}
