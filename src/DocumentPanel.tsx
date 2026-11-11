import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import ExperimentSave from "./ExperimentSave";
import type { OpenedFileExperiment } from "./experimentTypes";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { copyRecipe, newUserRecipe, recipeNeedsKey, type Recipe, type RecipeFingerprint } from "./recipeTypes";
import type { ActivityInfo, ActivityResult, CandidatePage, Companion, DocChoice, DocumentInfo, DropNotice, FilePanelActions, FilePanelState, JobInfo, OperationError, Preview, ReadingInfo } from "./documentTypes";

type Props = {
  visible: boolean; actionsRef: Ref<FilePanelActions>; recipe: Recipe | null; fingerprint: RecipeFingerprint | null;
  builtins: Recipe[]; unsavedRecipe: boolean; passphrase: string; direction: "encode" | "decode";
  onExperimentSaved: () => void; onExperimentBusy: (value: boolean) => void;
  onRecipe: (recipe: Recipe) => void; onSaveRecipe: (recipe: Recipe) => Promise<Recipe>;
  onNotice: (text: string) => void; onOpenMode: () => void; onStateChange: (state: FilePanelState) => void;
};
type Pending = { kind: "open" | "save" | "run" | "reading" | "import"; id: string | null; cancelled: boolean; version: number };
type Target = { documentId: string | null; jobId: string | null; readingId: string | null; stage: number | null };
const pause = () => new Promise<void>(resolve => window.setTimeout(resolve, 150));
function size(value: string): string { try { return BigInt(value).toLocaleString() + " bytes"; } catch { return "Unknown size"; } }
function failure(error: unknown): string {
  if (typeof error === "object" && error && "message" in error) {
    const info = error as OperationError;
    return [String(info.message), info.published?.length ? "Already saved: " + info.published.map(item => item.name).join(", ") + "." : "", info.unverified?.length ? "Check these files before retrying: " + info.unverified.join(", ") + "." : ""].filter(Boolean).join(" ");
  }
  return String(error);
}

function PagedText({ title, target }: { title: string; target: Target }) {
  const [offsets, setOffsets] = useState(["0"]);
  const [page, setPage] = useState<Preview | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const identity = JSON.stringify(target);
  useEffect(() => { setOffsets(["0"]); setPage(null); }, [identity]);
  const offset = offsets[offsets.length - 1];
  useEffect(() => {
    let current = true;setLoading(true);setError("");
    void invoke<Preview>("doc_preview", { ...target, offset }).then(result => { if (current) setPage(result); })
      .catch(error => { if (current) setError(failure(error)); }).finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [identity, offset]);
  return <section className="document-preview" aria-label={title}>
    <h3>{title}</h3>
    <pre aria-label={title + " text"}>{page?.text ?? (loading ? "Loading preview…" : "")}</pre>
    {error && <p role="alert">{error}</p>}
    <div className="document-paging">
      <button type="button" aria-label={"Previous " + title + " page"} disabled={loading || offsets.length === 1} onClick={() => setOffsets(values => values.slice(0, -1))}>Previous</button>
      <span>{page ? size(page.offset) + " – " + size(page.nextOffset) + " of " + size(page.bytes) : "Preview"}</span>
      <button type="button" aria-label={"Next " + title + " page"} disabled={loading || !page || page.eof} onClick={() => { if (page) setOffsets(values => [...values, page.nextOffset]); }}>Next</button>
    </div>
  </section>;
}

export default function DocumentPanel(props: Props) {
  const latest = useRef(props);latest.current = props;
  const alive = useRef(true);
  const [document, setDocument] = useState<DocumentInfo | null>(null);
  const documentRef = useRef<DocumentInfo | null>(null);
  const [job, setJob] = useState<JobInfo | null>(null);
  const jobRef = useRef<JobInfo | null>(null);
  const [reading, setReading] = useState<ReadingInfo | null>(null);
  const [pins, setPins] = useState<DocChoice[]>([]);
  const [savedJob, setSavedJob] = useState<string | null>(null);
  const [companion, setCompanion] = useState<Companion>("json");
  const [saveOutput, setSaveOutput] = useState<"exact" | "reading">("exact");
  const [stage, setStage] = useState("result");
  const [identifier, setIdentifier] = useState("");
  const [missingRecipe, setMissingRecipe] = useState(false);
  const [notice, setNotice] = useState("");
  const [operationBusy, setBusy] = useState(false);
  const [savingExperiment, setSavingExperiment] = useState(false);
  const busy = operationBusy || savingExperiment;
  const [progress, setProgress] = useState<{ phase: string; bytes: string; total: string | null } | null>(null);
  const pending = useRef<Pending | null>(null);
  const version = useRef(0);
  const resultSignature = useRef("");
  const signature = JSON.stringify([document?.id, props.recipe, props.direction, props.passphrase]);
  const currentSignature = useRef(signature);currentSignature.current = signature;
  const previousSignature = useRef(signature);
  const [confirmation, setConfirmation] = useState<"result" | "recipe" | null>(null);
  const confirmationRef = useRef<((answer: boolean) => void) | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const openAction = useRef<(token?: string) => Promise<void>>(async () => {});

  function announce(text: string) { if (alive.current) { setNotice(text);latest.current.onNotice(text); } }
  async function cancel() {
    const operation = pending.current;if (!operation) return;
    operation.cancelled = true;
    if (operation.id) await invoke(operation.kind === "run" ? "doc_cancel_job" : "doc_cancel_activity", { id: operation.id }).catch(() => {});
  }
  useEffect(() => {
    alive.current = true;
    let active = true;
    let stopDrop: (() => void) | undefined, stopError: (() => void) | undefined;
    void listen<DropNotice>("document-drop", event => { if (active) { latest.current.onOpenMode();void openAction.current(event.payload.token); } })
      .then(stop => { if (active) stopDrop = stop;else stop(); }).catch(() => {});
    void listen<string>("document-drop-error", event => { if (active) announce(event.payload); })
      .then(stop => { if (active) stopError = stop;else stop(); }).catch(() => {});
    return () => {
      active = false;alive.current = false;stopDrop?.();stopError?.();confirmationRef.current?.(false);
      void cancel();
      if (jobRef.current) void invoke("doc_discard_job", { id: jobRef.current.id }).catch(() => {});
      if (documentRef.current) void invoke("doc_discard_document", { id: documentRef.current.id }).catch(() => {});
    };
  }, []);
  useEffect(() => {
    if (previousSignature.current !== signature) {
      previousSignature.current = signature;version.current += 1;
      if (pending.current?.kind === "run" || pending.current?.kind === "reading") void cancel();
    }
  }, [signature]);
  useEffect(() => { if (confirmation && !dialog.current?.open) dialog.current?.showModal();if (!confirmation) dialog.current?.close(); }, [confirmation]);
  const validLegacy = !document || props.recipe?.schemaVersion === 2 || document.legacyCompatible;
  const canRun = !!document && !!props.recipe && !!props.fingerprint && validLegacy && !busy && (!recipeNeedsKey(props.recipe) || !!props.passphrase.trim());
  useEffect(() => { props.onStateChange({ busy, canRun, hasDocument: !!document, hasUnsavedResult: !!job && savedJob !== job.id }); }, [busy, canRun, !!document, job, savedJob, props.onStateChange]);
  useImperativeHandle(props.actionsRef, () => ({ open: () => { void open(); }, run: () => { void run(); }, restore }));

  function restore(opened: OpenedFileExperiment, recipe: Recipe) {
    const previousJob = jobRef.current;const previousDocument = documentRef.current;
    version.current += 1;documentRef.current = opened.document;jobRef.current = opened.job;
    setDocument(opened.document);setJob(opened.job);setReading(opened.reading);setPins(opened.choices);
    setSavedJob(opened.job.id);setSaveOutput("exact");setStage("result");setCompanion("json");
    resultSignature.current = JSON.stringify([opened.document.id, recipe, opened.job.direction, ""]);
    announce("Opened “" + opened.summary.name + "”. Results and stages are available; re-enter any required passphrase to continue.");
    if (previousJob) void invoke("doc_discard_job", { id: previousJob.id }).catch(() => {});
    if (previousDocument) void invoke("doc_discard_document", { id: previousDocument.id }).catch(() => {});
  }

  function ask(kind: "result" | "recipe"): Promise<boolean> {
    if (confirmationRef.current) return Promise.resolve(false);
    setConfirmation(kind);return new Promise(resolve => { confirmationRef.current = resolve; });
  }
  function answer(value: boolean) { const resolve = confirmationRef.current;confirmationRef.current = null;setConfirmation(null);resolve?.(value); }
  function begin(kind: Pending["kind"]): Pending | null {
    if (pending.current) { announce("Wait for the current operation or cancel it first.");return null; }
    const operation = { kind, id: null, cancelled: false, version: version.current };
    pending.current = operation;setBusy(true);setProgress({ phase: kind === "save" ? "Choose a new filename" : "Starting…", bytes: "0", total: null });setNotice("");return operation;
  }
  function end(operation: Pending) { if (pending.current === operation) { pending.current = null;if (alive.current) { setBusy(false);setProgress(null); } } }
  async function activity(operation: Pending, initial: ActivityInfo): Promise<ActivityResult | null> {
    operation.id = initial.id;
    let info = initial;
    try {
      while (info.state === "running") {
        if (operation.cancelled) await invoke("doc_cancel_activity", { id: info.id });
        if (alive.current) setProgress({ phase: info.phase, bytes: info.processedBytes, total: info.totalBytes });
        await pause();info = await invoke<ActivityInfo>("doc_activity", { id: info.id });
      }
      if (info.error) throw info.error;
      if (info.state === "cancelled") { announce("Operation cancelled.");return null; }
      return info.result;
    } finally {
      const retain = info.state === "complete" && info.result?.kind === "opened" && !operation.cancelled && alive.current;
      await invoke("doc_discard_activity", { id: initial.id, keepDocument: retain }).catch(() => {});
    }
  }
  async function open(token?: string) {
    if (pending.current) { announce("Cancel the current operation before opening another file.");return; }
    if (jobRef.current && savedJob !== jobRef.current.id && !await ask("result")) return;
    const operation = begin("open");if (!operation) return;
    try {
      const initial = token ? await invoke<ActivityInfo>("doc_open_drop", { token }) : await invoke<ActivityInfo | null>("doc_open");
      if (!initial) return;
      const result = await activity(operation, initial);
      if (result?.kind !== "opened") return;
      if (operation.cancelled || !alive.current) { await invoke("doc_discard_document", { id: result.document.id }).catch(() => {});return; }
      const previous = documentRef.current;const oldJob = jobRef.current;
      documentRef.current = result.document;setDocument(result.document);jobRef.current = null;setJob(null);setReading(null);setPins([]);setSavedJob(null);setSaveOutput("exact");setStage("result");
      if (oldJob) await invoke("doc_discard_job", { id: oldJob.id });
      if (previous) await invoke("doc_discard_document", { id: previous.id });
      announce("Opened " + result.document.name + ".");
    } catch (error) { announce(failure(error)); } finally { end(operation); }
  }
  openAction.current = open;
  async function run() {
    if (!canRun || !document || !props.recipe) return;
    if (job && savedJob !== job.id && resultSignature.current !== signature && !await ask("result")) return;
    const operation = begin("run");if (!operation) return;
    const capturedSignature = signature;
    let created: JobInfo | null = null;let adopted = false;
    try {
      created = await invoke<JobInfo>("doc_start", { documentId: document.id, recipe: copyRecipe(props.recipe), key: props.passphrase, direction: props.direction });
      operation.id = created.id;let info = created;
      while (info.state === "running") {
        if (operation.cancelled || operation.version !== version.current) await invoke("doc_cancel_job", { id: info.id });
        if (alive.current) setProgress({ phase: info.phase, bytes: info.processedBytes, total: info.totalBytes });
        await pause();info = await invoke<JobInfo>("doc_job", { id: info.id });
      }
      if (info.state !== "complete") { announce(info.note || "File operation cancelled.");return; }
      if (operation.cancelled || capturedSignature !== currentSignature.current || !alive.current) { announce("Configuration changed; the previous result is retained.");return; }
      const old = jobRef.current;jobRef.current = info;setJob(info);setReading(null);setPins([]);setSavedJob(null);setSaveOutput("exact");setStage("result");resultSignature.current = capturedSignature;adopted = true;
      if (old) await invoke("doc_discard_job", { id: old.id });
      announce(info.note);
    } catch (error) { announce(failure(error)); } finally { if (created && !adopted) await invoke("doc_discard_job", { id: created.id }).catch(() => {});end(operation); }
  }
  async function save(): Promise<boolean> {
    if (!job) return false;
    const operation = begin("save");if (!operation) return false;
    try {
      const initial = await invoke<ActivityInfo | null>("doc_save", { jobId: job.id, readingId: saveOutput === "reading" ? reading?.id ?? null : null, companion });
      if (!initial) return false;
      const result = await activity(operation, initial);
      if (result?.kind !== "saved") return false;
      setSavedJob(job.id);announce("Saved and verified: " + result.receipt.artifacts.map(item => item.name).join(", ") + ".");return true;
    } catch (error) { announce(failure(error));return false; } finally { end(operation); }
  }
  async function select(recipe: Recipe) {
    if (latest.current.unsavedRecipe && !await ask("recipe")) return;
    latest.current.onRecipe(recipe);setMissingRecipe(false);
  }
  async function compatibleCopy() {
    if (!props.recipe) return;
    const copy = copyRecipe(props.recipe);copy.id = newUserRecipe().id;copy.schemaVersion = 2;
    while (new TextEncoder().encode(copy.name).length > 86) copy.name = Array.from(copy.name).slice(0, -1).join("");
    copy.name += " (UTF-8)";
    await select(copy);announce("The UTF-8 copy has a new fingerprint. Save the recipe to keep it.");
  }
  async function lookup(value = identifier) {
    if (!value.trim()) return;
    try {
      const found = await invoke<Recipe | null>("resolve_recipe_identifier", { identifier: value });
      if (found) await select(found);else { setMissingRecipe(true);announce("This fingerprint is not in the bundled or saved library. Import its recipe JSON."); }
    } catch (error) { announce(failure(error)); }
  }
  async function importCompanion() {
    const operation = begin("import");if (!operation) return;
    try {
      const initial = await invoke<ActivityInfo | null>("doc_import", { expected: identifier });if (!initial) return;
      const result = await activity(operation, initial);
      if (operation.cancelled || result?.kind !== "imported") return;
      if (result.companion.kind === "recipe") {
        const recipe = copyRecipe(result.companion.recipe);
        recipe.id = newUserRecipe().id;
        await select(recipe);announce("Imported recipe for this session. Save it to retain it.");
      }
      else { setIdentifier(result.companion.identifier);await lookup(result.companion.identifier); }
    } catch (error) { announce(failure(error)); } finally { end(operation); }
  }
  async function reconstruct(next = pins) {
    if (!job) return;
    const operation = begin("reading");if (!operation) return;
    try {
      const initial = await invoke<ActivityInfo>("doc_reconstruct", { jobId: job.id, choices: next, key: job.restored ? props.passphrase : null });
      const result = await activity(operation, initial);
      if (result?.kind === "reconstructed" && jobRef.current?.id === result.reading.jobId && alive.current) { setReading(result.reading);setPins(next);setSavedJob(null);announce(result.reading.note); }
    } catch (error) { announce("Reading not applied: " + failure(error)); } finally { end(operation); }
  }
  async function copy(value: string) { try { await navigator.clipboard.writeText(value);announce("Copied."); } catch { announce("Clipboard unavailable. Select the full fingerprint below to copy it."); } }
  async function saveBeforeReplace() {
    const kind = confirmation;setConfirmation(null);
    if (kind === "result") { const saved = await save();answer(saved); }
    else if (kind === "recipe" && props.recipe) {
      try { const recipe = copyRecipe(props.recipe);if (recipe.id.startsWith("builtin-")) recipe.id = newUserRecipe().id;await props.onSaveRecipe(recipe);answer(true); } catch (error) { announce(failure(error));answer(false); }
    } else answer(false);
  }
  const resultTitle = job?.direction === "encode" ? "Encoded result" : job?.exact ? "Exact decoded text" : "Exact surviving intermediate";
  const selectedTarget: Target | null = job ? { documentId: null, jobId: job.id, readingId: null, stage: stage === "result" ? null : Number(stage) } : null;
  return <article className="notebook-page document-panel" hidden={!props.visible}>
    <h1>{document?.name ?? "Text files"}</h1>
    <p>Open a UTF-8 text or Markdown file. Save As writes a new result file.</p>
    {document && <div className="meta-line"><span>{document.encoding}{document.bom ? " with BOM" : ""}</span><span>{size(document.bytes)}</span><span>Read-only snapshot</span></div>}
    {props.recipe?.schemaVersion === 1 && <div className="document-legacy-note">
      <p>Legacy recipes use nonempty ASCII text up to 16 KiB. A UTF-8 recipe supports Unicode and larger files.</p>
      <button type="button" disabled={busy} onClick={() => void compatibleCopy()}>Create UTF-8 recipe copy</button>
      <button type="button" disabled={busy || !props.builtins.some(recipe => recipe.schemaVersion === 2)} onClick={() => { const example = props.builtins.find(recipe => recipe.schemaVersion === 2);if (example) void select(example); }}>Use document example</button>
    </div>}
    <section className="document-recipe-tools" aria-label="File recipe">
      <h2>Recipe</h2>
      <p>{props.recipe?.name ?? "Choose a recipe"}{props.unsavedRecipe ? " · Unsaved recipe: export JSON or save it in the library for portability." : ""}</p>
      {props.fingerprint && <><code className="document-fingerprint">{props.fingerprint.display}</code><div className="document-actions">
        <button type="button" onClick={() => void copy(props.fingerprint!.value)}>Copy fingerprint</button><button type="button" onClick={() => void copy(props.fingerprint!.display)}>Copy short code</button>
      </div><details><summary>Full fingerprint</summary><code className="document-fingerprint">{props.fingerprint.value}</code></details></>}
      <form onSubmit={event => { event.preventDefault();void lookup(); }}>
        <label htmlFor="file-recipe-identifier">Find a recipe by fingerprint</label>
        <textarea id="file-recipe-identifier" rows={2} maxLength={256} spellCheck={false} autoCapitalize="off" value={identifier} onChange={event => { setIdentifier(event.target.value);setMissingRecipe(false); }} />
        <div className="document-actions"><button type="submit" disabled={busy}>Find recipe</button><button type="button" disabled={busy} onClick={() => void importCompanion()}>{missingRecipe ? "Import recipe JSON" : "Import recipe or fingerprint file"}</button></div>
      </form>
    </section>
    {progress && <section className="document-progress" aria-label="File operation progress"><p role="status">{progress.phase}{progress.total ? " · " + size(progress.bytes) + " / " + size(progress.total) : ""}</p><progress aria-label="File operation" /><button type="button" onClick={() => void cancel()}>Cancel operation</button></section>}
    {notice && <p className="document-notice" role="status">{notice}</p>}
    {document && <PagedText key={document.id} title="Source preview" target={{ documentId: document.id, jobId: null, readingId: null, stage: null }} />}
    {job && <section className="document-result" aria-label="File result">
      <h2>{resultTitle}</h2>
      {resultSignature.current !== signature && <p className="document-stale">Previous result — the configuration has changed. Save uses the recipe and fingerprint shown with this result.</p>}
      <p>{job.recipe.name} · <code>{job.fingerprint.display}</code></p>
      <label htmlFor="file-stage">Inspect a stage</label><select id="file-stage" value={stage} onChange={event => setStage(event.target.value)}><option value="result">{resultTitle}</option>{job.stages.map((item, index) => <option key={index} value={index}>{item.label}</option>)}</select>
      {selectedTarget && <PagedText key={job.id + ":" + stage} title="Result preview" target={selectedTarget} />}
      {job.direction === "decode" && !job.exact && <div className="document-actions"><button type="button" disabled={busy || (!!job.restored && recipeNeedsKey(job.recipe) && !props.passphrase.trim())} onClick={() => void reconstruct()}>Suggest an English reading</button>{pins.length > 0 && <button type="button" disabled={busy || (!!job.restored && recipeNeedsKey(job.recipe) && !props.passphrase.trim())} onClick={() => void reconstruct([])}>Reset word choices</button>}</div>}
      {reading && <section aria-label="Speculative English reading"><h2>Speculative English reading</h2><p>{reading.note}</p><PagedText key={reading.id + ":preview"} title="English preview" target={{ documentId: null, jobId: job.id, readingId: reading.id, stage: null }} /><Candidates key={reading.id + ":candidates"} jobId={job.id} reading={reading} pins={pins} busy={busy} canChoose={!job.restored || !recipeNeedsKey(job.recipe) || !!props.passphrase.trim()} onChoose={(wordId, candidate) => void reconstruct([...pins.filter(pin => pin.wordId !== wordId), { wordId, candidate }])} /></section>}
      <fieldset className="document-companions" disabled={busy}><legend>Recipe companion</legend>{([ ["json", "Recipe JSON"], ["fingerprint", "Fingerprint text instead of JSON"], ["none", "No companion"] ] as const).map(([value, label]) => <label key={value}><input type="radio" name="file-companion" value={value} checked={companion === value} onChange={() => setCompanion(value)} />{label}</label>)}</fieldset>
      <p>A fingerprint finds a recipe already in the library. JSON carries the complete recipe. Keep the passphrase separately.</p>
      {reading && <><label htmlFor="file-save-output">Result to save</label><select id="file-save-output" value={saveOutput} onChange={event => setSaveOutput(event.target.value as "exact" | "reading")}><option value="exact">{resultTitle}</option><option value="reading">Speculative English reading</option></select></>}
      <ExperimentSave target={{ kind: "file", jobId: job.id, readingId: reading?.id ?? null }} defaultName={job.recipe.name + " · " + document?.name} disabled={busy} onBusy={value => { setSavingExperiment(value);props.onExperimentBusy(value); }} onSaved={() => { setSavedJob(job.id);props.onExperimentSaved(); }} />
      <button type="button" className="document-save" disabled={busy} onClick={() => void save()}>Save As…</button>
    </section>}
    <dialog ref={dialog} className="document-confirm" onCancel={event => { event.preventDefault();answer(false); }}>
      <h2>{confirmation === "recipe" ? "Save the current recipe?" : "Save the current result?"}</h2>
      <p>{confirmation === "recipe" ? "The current recipe has unsaved changes." : "Choose how to handle the unsaved result before replacing it."}</p>
      <div className="document-actions"><button type="button" onClick={() => void saveBeforeReplace()}>Save first</button><button type="button" onClick={() => answer(true)}>Discard changes</button><button type="button" autoFocus onClick={() => answer(false)}>Cancel</button></div>
    </dialog>
  </article>;
}

function Candidates({ jobId, reading, pins, busy, canChoose, onChoose }: { jobId: string; reading: ReadingInfo; pins: DocChoice[]; busy: boolean; canChoose: boolean; onChoose: (id: string, candidate: string) => void }) {
  const [offsets, setOffsets] = useState(["0"]);const [page, setPage] = useState<CandidatePage | null>(null);const [error, setError] = useState("");
  useEffect(() => { setOffsets(["0"]);setPage(null); }, [reading.id]);
  const offset = offsets[offsets.length - 1];
  useEffect(() => { let current = true;setError("");void invoke<CandidatePage>("doc_candidate_page", { jobId, readingId: reading.id, offset }).then(value => { if (current) setPage(value); }).catch(error => { if (current) setError(failure(error)); });return () => { current = false; }; }, [jobId, reading.id, offset]);
  return <section className="document-candidates" aria-label="English word choices">
    {error && <p role="alert">{error}</p>}
    {page?.words.map(word => <div key={word.wordId} className="document-word"><strong>Word {(BigInt(word.wordId.slice(word.wordId.lastIndexOf(":") + 1)) + 1n).toString()}: {word.transformed}</strong><div className="document-actions">{Array.from(new Set([word.suggested, ...word.alternatives])).map(candidate => <button type="button" key={candidate} disabled={busy || !canChoose} aria-pressed={pins.some(pin => pin.wordId === word.wordId && pin.candidate === candidate)} onClick={() => onChoose(word.wordId, candidate)}>{candidate}</button>)}</div><span>{word.compatibleCount} compatible candidates</span></div>)}
    <div className="document-paging"><button type="button" disabled={busy || offsets.length === 1} onClick={() => setOffsets(values => values.slice(0, -1))}>Previous words</button><button type="button" disabled={busy || !page || page.eof} onClick={() => { if (page) setOffsets(values => [...values, page.nextOffset]); }}>Next words</button></div>
  </section>;
}
