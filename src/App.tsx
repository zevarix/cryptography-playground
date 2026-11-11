import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  COLOR_ACCENTS, DEFAULT_APPEARANCE, THEMES, getTheme, paletteVars,
  readAppearance, type Appearance, type AppearanceMode, type FontChoice,
} from "./theme";
import RecipePage from "./RecipePage";
import DocumentPanel from "./DocumentPanel";
import ExperimentsPage from "./ExperimentsPage";
import ExperimentSave from "./ExperimentSave";
import ExperimentDialog from "./ExperimentDialog";
import type { ExperimentSummary, OpenedExperiment } from "./experimentTypes";
import type { FilePanelActions, FilePanelState } from "./documentTypes";
import classicalCipherLicense from "../licenses/ciphers-0.1.0-MIT.txt?raw";
import {
  RULE_CATALOG, labelForRule, lossyRule, recipeNeedsKey, copyRecipe, newUserRecipe,
  type RecipeFingerprint,
  type Recipe, type RecipeResult, type StageOutput, type Reconstruction,
  type RecoveryWord, type WordChoice,
} from "./recipeTypes";
import "./App.css";

type Page = "workbench" | "recipes" | "experiments" | "appearance" | "general" | "about";
type Direction = "encode" | "decode";
function wordOptions(word: RecoveryWord): string[] {
  return Array.from(new Set([word.suggested, ...word.alternatives]));
}

function frequencies(input: string): number[] {
  const counts = Array(26).fill(0) as number[];
  for (const ch of input.toLowerCase()) if (ch >= "a" && ch <= "z") counts[ch.charCodeAt(0) - 97]++;
  return counts;
}

function App() {
  const [page, setPage] = useState<Page>("workbench");
  const [appearance, setAppearance] = useState<Appearance>(readAppearance);
  const [systemDark, setSystemDark] = useState(() => matchMedia("(prefers-color-scheme: dark)").matches);
  const [search, setSearch] = useState("");
  const [message, setMessage] = useState("");
  const [direction, setDirection] = useState<Direction>("encode");
  const [plaintext, setPlaintext] = useState("The quick brown fox jumps over the lazy dog.");
  const [ciphertext, setCiphertext] = useState("");
  const [passphrase, setPassphrase] = useState("spectacular");
  const [output, setOutput] = useState("");
  const [intermediate, setIntermediate] = useState("");
  const [reconstruction, setReconstruction] = useState<Reconstruction | null>(null);
  const [builtins, setBuiltins] = useState<Recipe[]>([]);
  const [saved, setSaved] = useState<Recipe[]>([]);
  const [activeRecipe, setActiveRecipe] = useState<Recipe | null>(null);
  const [activeFingerprint, setActiveFingerprint] = useState<RecipeFingerprint | null>(null);
  const [stageOutputs, setStageOutputs] = useState<StageOutput[]>([]);
  const [exactOutput, setExactOutput] = useState(false);
  const [lockedChoices, setLockedChoices] = useState<WordChoice[]>([]);
  const [selectedWord, setSelectedWord] = useState<number | null>(null);
  const [choosing, setChoosing] = useState(false);
  const operationId = useRef(0);
  const [detail, setDetail] = useState("");
  const [busy, setBusy] = useState(false);
  const [inputMode, setInputMode] = useState<"text" | "file">("text");
  const [fileState, setFileState] = useState<FilePanelState>({ busy: false, canRun: false, hasDocument: false });
  const fileActions = useRef<FilePanelActions | null>(null);
  const [textSnapshot, setTextSnapshot] = useState<{ runId: string | null; dirty: boolean }>({ runId: null, dirty: false });
  const [restoredRunId, setRestoredRunId] = useState<string | null>(null);
  const [experimentBusy, setExperimentBusyState] = useState(false);
  const experimentBusyRef = useRef(false);
  const [experimentRevision, setExperimentRevision] = useState(0);
  const [openPrompt, setOpenPrompt] = useState(false);
  const confirmOpen = useRef<((answer: boolean) => void) | null>(null);
  const openingExperiment = useRef(false);
  const [inspected, setInspected] = useState<"intermediate" | "ciphertext">("intermediate");

  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const callback = () => setSystemDark(media.matches);
    media.addEventListener("change", callback);
    return () => media.removeEventListener("change", callback);
  }, []);
  useEffect(() => {
    localStorage.setItem("crypto-playground-appearance-v1", JSON.stringify(appearance));
  }, [appearance]);
  useEffect(() => {
    let active = true;
    void Promise.all([
      invoke<Recipe[]>("builtin_recipes"),
      invoke<Recipe[]>("list_saved_recipes"),
    ]).then(([installed, local]) => {
      if (!active) return;
      setBuiltins(installed);
      setSaved(local);
      const id = localStorage.getItem("crypto-playground-active-recipe-v1");
      setActiveRecipe([...installed, ...local].find(r => r.id === id) ?? installed[0] ?? null);
    }).catch(error => {
      if (active) setMessage("Could not load recipe library: " + String(error));
    });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    if (activeRecipe) localStorage.setItem("crypto-playground-active-recipe-v1", activeRecipe.id);
  }, [activeRecipe]);

  const mode = appearance.mode === "system" ? (systemDark ? "dark" : "light") : appearance.mode;
  const theme = getTheme(mode === "light" ? appearance.lightTheme : appearance.darkTheme, mode);
  const style = paletteVars(theme, appearance);
  const availableThemes = THEMES.filter(t => t.variant === mode);
  useEffect(() => {
    let current = true;
    setActiveFingerprint(null);
    if (activeRecipe) {
      void invoke<RecipeFingerprint>("recipe_fingerprint", { recipe: activeRecipe })
        .then(result => { if (current) setActiveFingerprint(result); })
        .catch(() => { if (current) setActiveFingerprint(null); });
    }
    return () => { current = false; };
  }, [activeRecipe]);
  const needsKey = recipeNeedsKey(activeRecipe);
  const missingKey = needsKey && !passphrase.trim();
  const stages = activeRecipe?.steps.filter(step => step.enabled).map(step => ({
    title: labelForRule(step.rule),
    detail: RULE_CATALOG.find(entry => entry.type === step.rule.type)?.detail ?? "",
    reversible: !lossyRule(step.rule),
  })) ?? [];
  const source = direction === "encode" ? plaintext : ciphertext;
  const canRun = !busy && !choosing && !experimentBusy && (inputMode === "file"
    ? fileState.canRun
    : !!activeRecipe && !!source.trim() && !missingKey && !fileState.busy);
  const isReversibleRecipe = stages.length > 0 && stages.every(stage => stage.reversible);
  const isExactDecode = direction === "decode" && isReversibleRecipe && exactOutput;
  const activeBaseline = activeRecipe && [...saved, ...builtins].find(recipe => recipe.id === activeRecipe.id);
  const activeIsUnsaved = !!activeRecipe &&
    (!activeBaseline || JSON.stringify(activeRecipe) !== JSON.stringify(activeBaseline));
  const inspectText = isExactDecode ? output :
    (inspected === "intermediate" ? intermediate : output);
  const bars = useMemo(() => frequencies(inspectText), [inspectText]);
  const total = bars.reduce((sum, count) => sum + count, 0);

  function setLook(patch: Partial<Appearance>) {
    setAppearance(current => ({ ...current, ...patch }));
  }
  function setExperimentBusy(value: boolean) {
    // Native events can arrive before React publishes the disabled controls.
    experimentBusyRef.current = value;
    setExperimentBusyState(value);
  }
  function clearResults() {
    operationId.current += 1;
    setOutput(""); setIntermediate(""); setReconstruction(null);
    setLockedChoices([]); setSelectedWord(null); setChoosing(false);
    setStageOutputs([]); setExactOutput(false);
    setDetail(""); setMessage("");
    setTextSnapshot({ runId: null, dirty: true });
    setRestoredRunId(null);
  }
  function applyResult(result: RecipeResult, mode: Direction) {
    setOutput(result.output);setReconstruction(result.reconstruction);setStageOutputs(result.stages);setExactOutput(result.exact);
    setIntermediate(mode === "encode" && result.stages.length > 1 ? result.stages[result.stages.length - 2].text : result.intermediate);
    setInspected(mode === "encode" ? "intermediate" : "ciphertext");setDetail(result.note);
    if (mode === "encode") setCiphertext(result.output);
  }
  function answerOpen(answer: boolean) { const resolve = confirmOpen.current;confirmOpen.current = null;setOpenPrompt(false);resolve?.(answer); }
  async function openExperiment(entry: ExperimentSummary) {
    if (openingExperiment.current || busy || choosing || fileState.busy || experimentBusyRef.current) return;
    openingExperiment.current = true;setExperimentBusy(true);
    try {
      if (textSnapshot.dirty || fileState.hasUnsavedResult || (entry.kind === "file" && fileState.hasDocument) || activeIsUnsaved) {
        setOpenPrompt(true);
        const accepted = await new Promise<boolean>(resolve => { confirmOpen.current = resolve; });
        if (!accepted) return;
      }
      const opened = await invoke<OpenedExperiment>("open_experiment", { id: entry.id });
      const recipe = copyRecipe(opened.kind === "text" ? opened.recipe : opened.job.recipe);
      recipe.id = newUserRecipe().id;
      clearResults();setPassphrase("");setActiveRecipe(recipe);
      if (opened.kind === "text") {
        setInputMode("text");setDirection(opened.direction);
        if (opened.direction === "encode") setPlaintext(opened.source);else setCiphertext(opened.source);
        applyResult(opened.result, opened.direction);
        setLockedChoices(opened.choices);
        setTextSnapshot({ runId: opened.runId, dirty: false });
        setRestoredRunId(opened.runId);
      } else {
        setTextSnapshot({ runId: null, dirty: false });
        setInputMode("file");setDirection(opened.job.direction);
        fileActions.current?.restore(opened, recipe);
      }
      setPage("workbench");setMessage("Opened “" + opened.summary.name + "”. Passphrases are kept separate.");
    } finally { openingExperiment.current = false;setExperimentBusy(false); }
  }
  async function run() {
    if (!canRun || experimentBusyRef.current) return;
    if (inputMode === "file") { fileActions.current?.run();return; }
    if (!activeRecipe) return;
    const operation = ++operationId.current;
    setBusy(true); setChoosing(false); setMessage("");
    try {
      const result = await invoke<RecipeResult & { runId: string }>("evaluate_recipe", {
        recipe: activeRecipe, text: source, key: passphrase, direction, choices: [],
        ...(restoredRunId && direction === "decode" ? { runId: restoredRunId } : {}),
      });
      if (operation !== operationId.current) return;
      applyResult(result, direction);
      setTextSnapshot({ runId: result.runId, dirty: true });
      if (restoredRunId) setRestoredRunId(result.runId);
      setLockedChoices([]);setSelectedWord(null);
      setMessage(result.exact
        ? (direction === "encode" ? "Reversible recipe encoded." : "Decoded exactly.")
        : direction === "encode"
          ? "Lossy recipe encoded."
          : result.reconstruction?.suggested
            ? "The English text is a speculative reconstruction."
            : "Recovered the surviving layer; no compatible English suggestion.");
    } catch (error) {
      if (operation !== operationId.current) return;
      if (!restoredRunId) {
        setOutput(""); setIntermediate(""); setReconstruction(null);
        setStageOutputs([]); setExactOutput(false); setDetail("");
        setTextSnapshot(current => ({ ...current, runId: null }));
      }
      setMessage(String(error));
    } finally {
      if (operation === operationId.current) setBusy(false);
    }
  }

  async function reviseChoices(next: WordChoice[], changed?: WordChoice) {
    if (!reconstruction || !activeRecipe || direction !== "decode" || choosing || busy || experimentBusy || missingKey) return;
    const operation = ++operationId.current;
    const previousOutput = output;
    const previousReconstruction = reconstruction;
    const previousDetail = detail;
    setChoosing(true);
    setSelectedWord(null);
    setMessage("Checking the selected words against the full phonetic rotation...");
    // Update the visible phrase immediately; the Rust engine validates all
    // remaining words before accepting the selection as a real interpretation.
    if (changed) {
      setOutput(reconstruction.words.map((word, index) =>
        index === changed.index ? changed.candidate : word.suggested).join(" "));
    }
    try {
      const result = await invoke<RecipeResult & { runId: string }>("evaluate_recipe", {
        recipe: activeRecipe, text: ciphertext, key: passphrase, direction: "decode", choices: next,
        runId: restoredRunId,
      });
      if (operation !== operationId.current) return;
      if (!result.reconstruction?.suggested) throw new Error("No compatible English reconstruction was found.");
      if (result.intermediate !== intermediate) throw new Error("The recovered intermediate changed unexpectedly.");
      setOutput(result.reconstruction.suggested);
      setReconstruction(result.reconstruction);
      setStageOutputs(result.stages);
      setLockedChoices(next);
      setTextSnapshot({ runId: result.runId, dirty: true });
      if (restoredRunId) setRestoredRunId(result.runId);
      setDetail(result.reconstruction.note);
      setInspected("ciphertext");
      setMessage(next.length
        ? "Updated and checked " + next.length + " selected word(s)."
        : "Restored the highest-ranked English suggestion.");
    } catch (error) {
      if (operation !== operationId.current) return;
      setOutput(previousOutput);
      setReconstruction(previousReconstruction);
      setDetail(previousDetail);
      setMessage("Choice not applied: " + String(error));
    } finally {
      if (operation === operationId.current) setChoosing(false);
    }
  }

  function chooseCandidate(index: number, candidate: string) {
    const previous = lockedChoices.find(choice => choice.index === index);
    const next = lockedChoices.filter(choice => choice.index !== index);
    if (previous?.candidate !== candidate) {
      next.push({ index, candidate });
    }
    void reviseChoices(next, previous?.candidate === candidate ? undefined : { index, candidate });
  }
  function activateRecipe(recipe: Recipe) {
    clearResults();
    setActiveRecipe(recipe);
    setPage("workbench");
    setMessage("Recipe selected: " + recipe.name);
  }
  async function saveRecipe(recipe: Recipe): Promise<Recipe> {
    const result = await invoke<Recipe>("save_recipe", { recipe });
    setSaved(existing => [result, ...existing.filter(item => item.id !== result.id)]);
    activateRecipe(result);
    return result;
  }
  async function deleteRecipe(id: string): Promise<void> {
    await invoke<void>("delete_saved_recipe", { id });
    setSaved(existing => existing.filter(item => item.id !== id));
    if (activeRecipe?.id === id) activateRecipe(builtins[0]);
  }
  async function copy(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setMessage("Copied.");
    } catch {
      setMessage("Clipboard access unavailable.");
    }
  }
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.key === ",") {
        event.preventDefault();
        setPage("appearance");
      } else if (event.key === "Enter" && page === "workbench") {
        event.preventDefault();
        void run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const navItems: Array<{ page: Exclude<Page, "workbench" | "recipes" | "experiments">; title: string; icon: string }> = [
    { page: "general", title: "General", icon: "⚙" },
    { page: "appearance", title: "Appearance", icon: "☼" },
    { page: "about", title: "About", icon: "ⓘ" },
  ];
  return (
    <div className={"app-root " + mode} style={style}>
      <header className="topbar">
        <div className="brand"><span className="brand-icon">⌁</span><strong>Cryptography Playground</strong><span className="brand-division">/ Notebook Lab</span></div>
        <div className="top-controls">
          <span className="recipe-name">{activeRecipe?.name ?? "Loading recipes."}</span>
          {activeIsUnsaved && page === "workbench" && (
            <span className="recipe-unsaved-top" title="This recipe configuration has not been saved and will not survive an app restart.">
              Unsaved draft
            </span>
          )}
          <button className="top-run" onClick={() => void run()} disabled={!canRun}>▶ {busy ? "Running" : fileState.busy ? "Working" : "Run"}</button>
          <button onClick={() => setPage("appearance")} aria-label="Appearance settings" title="Appearance settings">⚙</button>
        </div>
      </header>
      {openPrompt && <ExperimentDialog title="Open saved experiment?" confirmLabel="Open experiment" onConfirm={() => answerOpen(true)} onCancel={() => answerOpen(false)}><p>Opening replaces the current notebook work and recipe. Choose Cancel to keep them.</p></ExperimentDialog>}
      <div className="app-flex">
        <nav className="icon-rail" aria-label="Primary navigation">
          <button title="Workbench" aria-label="Workbench" className={page === "workbench" ? "active" : ""} onClick={() => setPage("workbench")}>▤</button>
          <button title="Recipes" aria-label="Recipes" className={page === "recipes" ? "active" : ""}
            onClick={() => setPage("recipes")}>▦</button>
          <button title="Experiments" aria-label="Experiments" className={page === "experiments" ? "active" : ""} onClick={() => setPage("experiments")}>▣</button>
          <button title="Appearance" aria-label="Appearance" className={page === "appearance" ? "active" : ""} onClick={() => setPage("appearance")}>◐</button>
          <div className="spacer" />
          <button title="About" aria-label="About" className={page === "about" ? "active" : ""} onClick={() => setPage("about")}>ⓘ</button>
        </nav>
          <main className={"workbench" + (inputMode === "file" ? " document-workbench" : "")} hidden={page !== "workbench"}>
            <aside className="source-sidebar">
              <div className="sidebar-title"><h2>Experiment</h2>
                <button className="recipe-change" onClick={() => setPage("recipes")}>{activeRecipe?.name ?? "Choose a recipe"} ↗</button>
              </div>
              {activeIsUnsaved ? (
                <div className="recipe-save-state unsaved" role="status">
                  <strong>Unsaved recipe</strong>
                  <span>Rule changes won't survive restarting the app.</span>
                  <button onClick={() => setPage("recipes")}>Save this recipe</button>
                </div>
              ) : activeRecipe ? (
                <div className="recipe-save-state">
                  <span>{activeRecipe.id.startsWith("builtin-") ? "Bundled example" : "Saved locally on this computer"}</span>
                </div>
              ) : null}
              {activeFingerprint && (
                <div className="workbench-fingerprint">
                  <span>Recipe fingerprint</span>
                  <code title={activeFingerprint.value} data-testid="active-recipe-fingerprint">
                    {activeFingerprint.display}
                  </code>
                </div>
              )}
              <label className="input-caption" htmlFor="operation-mode">Direction</label>
              <select disabled={experimentBusy} id="operation-mode" value={direction} onChange={e => { setDirection(e.target.value as Direction); clearResults(); }}>
                <option value="encode">Encode plaintext</option><option value="decode">{stages.every(s => s.reversible) ? "Decode exactly" : "Decode / reconstruct English"}</option>
              </select>
              <div className="input-mode" role="group" aria-label="Input source"><button type="button" aria-pressed={inputMode === "text"} onClick={() => setInputMode("text")}>Text</button><button type="button" aria-pressed={inputMode === "file"} onClick={() => setInputMode("file")}>File</button></div>
              <div hidden={inputMode !== "text"}>
              <label htmlFor="source-input" className="input-caption">{direction === "encode" ? "Plaintext" : "Ciphertext"}</label>
              <textarea disabled={experimentBusy} id="source-input" rows={8} spellCheck={false} value={source} onChange={e => {
                if (direction === "encode") setPlaintext(e.target.value);
                else setCiphertext(e.target.value);
                clearResults();
              }} placeholder="Enter text" />
              </div>
              <div className="file-source-actions" hidden={inputMode !== "file"}><button type="button" disabled={busy || choosing || fileState.busy || experimentBusy} onClick={() => fileActions.current?.open()}>Open file</button><p className="helper-text">Drop a file into the app or choose a UTF-8 .txt or .md file.</p></div>
              <label className="input-caption" htmlFor="passphrase">Passphrase {needsKey ? "(required)" : "(unused)"}</label>
              <input disabled={experimentBusy} id="passphrase" value={passphrase} maxLength={64} spellCheck={false} autoComplete="off"
                onChange={e => { setPassphrase(e.target.value); if (!restoredRunId || inputMode !== "text") clearResults(); }} />
              {restoredRunId && inputMode === "text" && needsKey && <p className="helper-text">Saved result restored. Re-enter the passphrase to rerun or change word choices.</p>}
              <p className="helper-text">{needsKey
                ? "The enabled keyed rules use this passphrase. Some rules derive parameters from the recipe configuration and key."
                : "This recipe has no keyed rule; the passphrase is ignored."}</p>
              <button className="sidebar-run" onClick={() => void run()} disabled={!canRun}>▶ {inputMode === "file" ? "Run file recipe" : direction === "encode" ? "Encode" : stages.every(s => s.reversible) ? "Decode exactly" : "Decode / suggest English"}</button>
              <div className="sidebar-note">
                <strong>Experimental only</strong>
                <p>{stages.some(s => !s.reversible)
                  ? "This recipe discards information. English reconstruction is speculative."
                  : activeRecipe?.schemaVersion === 2 ? "This recipe is reversible on UTF-8 text." : "This recipe is reversible on supported ASCII input."}
                  Never use experimental recipes for protecting sensitive data.</p>
              </div>
            </aside>
            <section className="notebook">
              <div className="notebook-tabs"><span className="active">◈ {activeRecipe?.name ?? "Choose a recipe"}</span>
                <button onClick={() => setPage("recipes")}>Edit rules</button></div>
              <article className="notebook-page" hidden={inputMode !== "text"}>
                <div className="punch-holes" aria-hidden="true">◦<span>◦</span><span>◦</span></div>
                <h1>Experiment: {activeRecipe?.name ?? "Loading"}</h1>
                <div className="meta-line"><span>{activeRecipe?.id ?? "loading"}</span>
                  <span>{stages.length} rules</span>
                  <span>{stages.some(s => !s.reversible) ? "lossy" : "reversible"}</span></div>
                <h2>Goal</h2>
                <p>{activeRecipe?.description ?? "Pick a recipe from the library to get started."}</p>
                <h2>Recipe · transformation sequence</h2>
                <ol className="stage-list">
                  {stages.map((stage, i) => (
                    <li key={stage.title}>
                      <span className="stage-index">{String(i + 1).padStart(2, "0")}</span>
                      <div><strong>{stage.title}</strong><p>{stage.detail}</p></div>
                      <span className={"stage-type " + (stage.reversible ? "reversible" : "lossy")}>{stage.reversible ? "REV" : "LOSS"}</span>
                    </li>
                  ))}
                </ol>
                <h2>Transformation results</h2>
                <div className="result-grid">
                  {(direction === "decode" ? !isReversibleRecipe : stageOutputs.length > 1) && (
                    <div><h3>{direction === "encode" ? "Last intermediate stage" : "Exact surviving intermediate"}</h3>
                      <pre id="intermediate-result">{intermediate || "Run the recipe to inspect the intermediate text."}</pre>
                    </div>
                  )}
                  <div>
                    <h3>{direction === "encode" ? "Final ciphertext" : isReversibleRecipe ? "Exact decoded plaintext" : "Suggested English (not verified)"}</h3>
                    {direction === "decode" && reconstruction?.suggested &&
                      output === reconstruction.suggested &&
                      reconstruction.words.length === output.split(/\s+/).length ? (
                      <div className="english-phrase" id="cipher-result" aria-label="Suggested English sentence">
                        {reconstruction.words.map((word, index) => (
                          <span key={index + "-" + word.transformed}>
                            {index > 0 ? " " : ""}
                            {word.compatibleCount > 1 ? (
                              <button type="button" className="selectable-word"
                                aria-label={"Choose a different word for " + word.suggested + ", word " + (index + 1)}
                                aria-expanded={selectedWord === index}
                                onClick={() => setSelectedWord(selectedWord === index ? null : index)}
                                disabled={choosing || busy || experimentBusy || missingKey} title={"Word " + (index + 1) + ": " + word.compatibleCount + " possible readings"}>
                                {word.suggested}
                              </button>
                            ) : word.suggested}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <pre id="cipher-result">{output || (direction === "decode" && intermediate && !exactOutput ? "No compatible English suggestion found." : "No result yet.")}</pre>
                    )}
                    {direction === "decode" && selectedWord !== null &&
                      reconstruction?.words[selectedWord] && (
                      <div className="word-choice-panel" role="group"
                        aria-label={"Choose word " + (selectedWord + 1)}>
                        <div className="word-choice-heading">
                          <strong>Word {selectedWord + 1}: <code>{reconstruction.words[selectedWord].transformed}</code></strong>
                          <button onClick={() => setSelectedWord(null)} aria-label="Close word choices">×</button>
                        </div>
                        <div className="word-option-buttons">
                          {wordOptions(reconstruction.words[selectedWord]).map(candidate => (
                            <button type="button" key={candidate}
                              className={reconstruction.words[selectedWord].suggested === candidate ? "current" : ""}
                              aria-pressed={reconstruction.words[selectedWord].suggested === candidate}
                              aria-label={"Use " + candidate + " for word " + (selectedWord + 1)}
                              onClick={() => chooseCandidate(selectedWord, candidate)}
                              disabled={choosing || busy || experimentBusy || missingKey}>{candidate}</button>
                          ))}
                        </div>
                        <small>Selections are checked against the rest of the message.</small>
                      </div>
                    )}
                    {direction === "decode" && !isReversibleRecipe && intermediate && reconstruction && (
                      <>
                        <p className="reconstruction-warning">Click an underlined word in the English suggestion, or choose a candidate in the inspector. Each selection is checked against the full phonetic rotation; the original English still cannot be proven.</p>
                        <label htmlFor="english-edit" className="english-edit-label">Manual interpretation (candidate selections replace manual edits)</label>
                        <textarea id="english-edit" aria-label="Edit suggested English" value={output}
                          onChange={event => { setOutput(event.target.value);setTextSnapshot({ runId: null, dirty: true }); }} spellCheck={true} rows={2}
                          placeholder="Enter your preferred English interpretation..." />
                      </>
                    )}
                  </div>
                </div>
                {stageOutputs.length > 0 && (
                  <details className="recipe-stage-details">
                    <summary>Inspect {stageOutputs.length} executed rule stages</summary>
                    {stageOutputs.map((stage, i) => (
                      <div key={i}><strong>{stage.label} · {stage.lossy ? "LOSS" : "REV"}</strong>
                        <pre>{stage.text}</pre></div>
                    ))}
                  </details>
                )}
                {textSnapshot.runId && <ExperimentSave
                  target={{ kind: "text", runId: textSnapshot.runId }}
                  defaultName={activeRecipe?.name ?? "Text experiment"}
                  disabled={busy || choosing || fileState.busy || experimentBusy}
                  onBusy={setExperimentBusy}
                  onSaved={() => {
                    setTextSnapshot(current => current === textSnapshot ? { ...current, dirty: false } : current);
                    setExperimentRevision(value => value + 1);
                  }} />}
                <div className="notebook-actions">
                  <span>{choosing ? "Rechecking the selected words and subsequent rotations…" : (detail || "No run recorded")}</span>
                  {direction === "decode" && lockedChoices.length > 0 && (
                    <button disabled={choosing || busy || experimentBusy || missingKey} onClick={() => void reviseChoices([])}>Reset word selections ({lockedChoices.length})</button>
                  )}
                  <button disabled={!output || choosing} onClick={() => void copy(output)}>
                    {direction === "decode" ? (isReversibleRecipe ? "Copy decoded text" : "Copy English suggestion") : "Copy result"}
                  </button>
                </div>
              </article>
              <DocumentPanel visible={inputMode === "file"} actionsRef={fileActions} recipe={activeRecipe} fingerprint={activeFingerprint}
                experimentBusy={experimentBusy} isExperimentBusy={() => experimentBusyRef.current}
                builtins={builtins} unsavedRecipe={activeIsUnsaved} passphrase={passphrase} direction={direction}
                onRecipe={activateRecipe} onSaveRecipe={saveRecipe} onNotice={setMessage}
                onOpenMode={() => { setInputMode("file");setPage("workbench"); }} onStateChange={setFileState}
                onExperimentSaved={() => setExperimentRevision(value => value + 1)} onExperimentBusy={setExperimentBusy} />
            </section>
            <aside className="inspector" hidden={inputMode === "file"}>
              <h2>Inspector</h2>
              {isReversibleRecipe && direction === "decode" ? (
                <div className="inspector-exact-status">{isExactDecode ? "Exact decoded result" : "Reversible recipe"}</div>
              ) : (
                <div className="inspector-tabs" role="group" aria-label="Inspected result">
                  <button className={inspected === "intermediate" ? "active" : ""} onClick={() => setInspected("intermediate")}>Intermediate</button>
                  <button className={inspected === "ciphertext" ? "active" : ""} onClick={() => setInspected("ciphertext")}>
                    {direction === "decode" ? "English suggestion" : "Output"}
                  </button>
                </div>
              )}
              <div className="inspector-block"><h3>{isExactDecode ? "Exact plaintext" : "Selected result"}</h3>
                <pre>{inspectText || "No result to inspect."}</pre>
              </div>
              <div className="inspector-block"><h3>Properties</h3>
                <dl className="data-rows">
                  <div><dt>Characters</dt><dd>{inspectText.length}</dd></div>
                  <div><dt>Letters</dt><dd>{total}</dd></div>
                  <div><dt>Input length</dt><dd>{source.length}</dd></div>
                  <div><dt>Recipe</dt><dd>{activeRecipe?.name ?? "Not selected"}</dd></div>
                  <div><dt>Rules enabled</dt><dd>{stages.length}</dd></div>
                </dl>
              </div>
              <div className="inspector-block"><h3>Frequency analysis</h3>
                <div className="frequency-bars" aria-label="Letter frequencies">
                  {bars.map((value, i) => <div key={i} className="frequency-col" title={String.fromCharCode(65 + i) + ": " + value}>
                    <span style={{ height: String(total ? Math.min(100, Math.max(2, 500 * value / total)) : 2) + "%" }} />
                    <small>{String.fromCharCode(65 + i)}</small>
                  </div>)}
                </div>
              </div>
              {direction === "decode" && reconstruction && (
                <div className="inspector-block">
                  <h3>Candidate word expansions</h3>
                  <p>{reconstruction.note}</p>
                  {reconstruction.words.map((word, index) => word.compatibleCount > 1 && (
                    <div key={index + "-" + word.transformed} className="reconstruction-word">
                      <div><code>{word.transformed}</code><span>→ {word.suggested}</span>
                        {lockedChoices.some(c => c.index === index) && <small className="locked-word">Selected</small>}
                      </div>
                      <small>Word {index + 1} · {word.compatibleCount} compatible dictionary readings (showing top options)</small>
                      <div className="candidate-options" role="group"
                        aria-label={"Candidates for word " + (index + 1) + " (" + word.transformed + ")"}>
                        {wordOptions(word).map(candidate => (
                          <button type="button" key={candidate}
                            className={candidate === word.suggested ? "candidate-current" : ""}
                            aria-pressed={candidate === word.suggested}
                            aria-label={"Use " + candidate + " for word " + (index + 1)}
                            disabled={busy || choosing || experimentBusy || missingKey}
                            onClick={() => chooseCandidate(index, candidate)}>
                            {candidate}
                          </button>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
                <div className="inspector-block"><h3>Reversibility</h3>
                  <p>{exactOutput ? (activeRecipe?.schemaVersion === 2 ? "All enabled rules reverse exactly on UTF-8 text." : "All enabled rules reverse exactly on supported ASCII input.")
                    : "This recipe contains a lossy rule. Any English suggestion is speculative, even when its encoding matches."}</p>
                </div>
            </aside>
          </main>
        {page === "experiments" ? <ExperimentsPage busy={busy || choosing || fileState.busy || experimentBusy} revision={experimentRevision} onOpen={openExperiment} /> : page === "recipes" ? (
          <RecipePage builtins={builtins} saved={saved} active={activeRecipe}
            busy={busy || choosing || fileState.busy || experimentBusy} onActivate={activateRecipe}
            onSave={saveRecipe} onDelete={deleteRecipe} onNotice={setMessage}
            onWorkbench={() => setPage("workbench")} />
        ) : page !== "workbench" ? (
          <main className="settings-layout">
            <aside className="settings-nav">
              <h2>Settings</h2>
              <div className="search-box"><span>⌕</span><input aria-label="Search settings" placeholder="Search" value={search} onChange={e => setSearch(e.target.value)} /></div>
              <div className="nav-heading">Personal</div>
              {navItems.filter(x => x.title.toLowerCase().includes(search.toLowerCase())).map(item => (
                <button key={item.page} className={page === item.page ? "selected" : ""} onClick={() => setPage(item.page)}><span>{item.icon}</span>{item.title}</button>
              ))}
              {search && !navItems.some(x => x.title.toLowerCase().includes(search.toLowerCase())) && <p className="helper-text">No matching settings.</p>}
            </aside>
            <article className="settings-main">
              {page === "appearance" ? (
                <div className="settings-inner">
                  <h1>Appearance</h1>
                  <h2>Visual style</h2>
                  <section className="settings-group">
                    <div className="settings-row mode-row"><span className="setting-label">Mode</span>
                      <div className="mode-choices" role="group" aria-label="Appearance mode">
                        {(["system", "light", "dark"] as AppearanceMode[]).map(m => (
                          <button key={m} className={appearance.mode === m ? "selected" : ""} aria-pressed={appearance.mode === m}
                            onClick={() => setLook({ mode: m })}>
                            <span className={"preview preview-" + m} aria-hidden="true"><i /><i /><i /></span>
                            {m === "system" ? "System" : m === "light" ? "Light" : "Dark"}
                          </button>
                        ))}
                      </div>
                    </div>
                  </section>
                  <section className="settings-group">
                    <div className="settings-row">
                      <label className="setting-label" htmlFor="theme-choice">Theme</label>
                      <select id="theme-choice" value={theme.id} onChange={e => setLook({
                        [mode === "light" ? "lightTheme" : "darkTheme"]: e.target.value,
                        background: null, foreground: null,
                      })}>
                        <optgroup label="Notebook Lab">
                          {availableThemes.filter(t => t.author === "Cryptography Playground").map(t => <option key={t.id} value={t.id}>{t.name}</option>)}
                        </optgroup>
                        <optgroup label="Community themes">
                          {availableThemes.filter(t => t.author !== "Cryptography Playground").map(t => <option key={t.id} value={t.id}>{t.name}</option>)}
                        </optgroup>
                      </select>
                    </div>
                    <div className="settings-row"><span className="setting-label">Accent</span>
                      <div className="accent-choices" role="group" aria-label="Accent color">
                        <button title="Theme default" aria-label="Theme default accent" className={!appearance.accent ? "selected" : ""}
                          onClick={() => setLook({ accent: null })}><span style={{ backgroundColor: theme.palette.accent }} /></button>
                        {COLOR_ACCENTS.map(color => <button key={color} title={color} aria-label={"Accent " + color}
                          className={appearance.accent === color ? "selected" : ""} onClick={() => setLook({ accent: color })}>
                          <span style={{ backgroundColor: color }} />
                        </button>)}
                      </div>
                    </div>
                    <div className="settings-row">
                      <label className="setting-label" htmlFor="background-choice">Background</label>
                      <div className="color-choice">
                        <input id="background-choice" aria-label="Background color" type="color" value={appearance.background ?? theme.palette.background}
                          onChange={e => setLook({ background: e.target.value })} />
                        <span>{(appearance.background ?? theme.palette.background).toUpperCase()}</span>
                        {appearance.background && <button onClick={() => setLook({ background: null })}>Reset</button>}
                      </div>
                    </div>
                    <div className="settings-row">
                      <label className="setting-label" htmlFor="foreground-choice">Foreground</label>
                      <div className="color-choice">
                        <input id="foreground-choice" aria-label="Foreground color" type="color" value={appearance.foreground ?? theme.palette.text}
                          onChange={e => setLook({ foreground: e.target.value })} />
                        <span>{(appearance.foreground ?? theme.palette.text).toUpperCase()}</span>
                        {appearance.foreground && <button onClick={() => setLook({ foreground: null })}>Reset</button>}
                      </div>
                    </div>
                    <div className="settings-row">
                      <label className="setting-label" htmlFor="font-choice">Font</label>
                      <select id="font-choice" value={appearance.font} onChange={e => setLook({ font: e.target.value as FontChoice })}>
                        <option value="system">System</option><option value="sans">Sans-serif</option><option value="mono">Monospace</option>
                      </select>
                    </div>
                  </section>
                  <div className="attribution">{theme.author === "Cryptography Playground"
                    ? "Notebook Lab original palette." : "Palette by " + theme.author + " · " + theme.license + "."}
                    {theme.source && <> <a href={theme.source} target="_blank" rel="noreferrer">Upstream source</a></>}
                  </div>
                  <details className="advanced"><summary>Advanced</summary>
                    <p>Theme and mode preferences are saved on this computer. Community color palettes are independently maintained and attributed in docs/theme-attributions.md.</p>
                    <button className="reset-appearance" onClick={() => { setAppearance({ ...DEFAULT_APPEARANCE }); setMessage("Appearance reset."); }}>Reset appearance to defaults</button>
                  </details>
                </div>
              ) : page === "general" ? (
                <div className="settings-inner"><h1>General</h1><h2>Environment</h2>
                  <section className="settings-group"><div className="settings-row"><span className="setting-label">Runtime</span><span>Tauri 2 · offline desktop</span></div>
                    <div className="settings-row"><span className="setting-label">Recipe</span><span>{activeRecipe?.name ?? "No recipe selected"}</span></div>
                    <div className="settings-row"><span className="setting-label">Preferences</span><span>Saved locally</span></div>
                  </section><p className="helper-text">No cloud account or synchronization is configured.</p>
                </div>
              ) : (
                <div className="settings-inner"><h1>About</h1><h2>Cryptography Playground</h2>
                  <section className="settings-group"><div className="settings-row"><span>Current experiment</span><span>{activeRecipe?.name ?? "None"}</span></div>
                    <div className="settings-row"><span>Technology</span><span>Rust · React · Tauri 2</span></div>
                  </section><p className="helper-text">An independent, experimental cryptography playground. It is not suitable for protecting sensitive data.</p>
                  <details className="advanced cipher-license">
                    <summary>Classical cipher library license</summary>
                    <p>ciphers · MIT</p>
                    <pre className="license-notice">{classicalCipherLicense}</pre>
                  </details>
                </div>
              )}
            </article>
          </main>
        ) : null}
      </div>
      <footer className="statusbar"><span>◈ Cryptography Playground</span><span role="status">{page === "workbench" && message ? message : (theme.name + " / " + mode)}</span><span>{busy || fileState.busy ? "Working…" : "Ready"}</span></footer>
    </div>
  );
}
export default App;
