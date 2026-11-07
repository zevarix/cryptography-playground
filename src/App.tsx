import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  COLOR_ACCENTS, DEFAULT_APPEARANCE, THEMES, getTheme, paletteVars,
  readAppearance, type Appearance, type AppearanceMode, type FontChoice,
} from "./theme";
import "./App.css";

type Page = "workbench" | "appearance" | "general" | "about";
type Direction = "encode" | "decode";
type Encoded = { ciphertext: string; intermediate: string; words: number; phonemes: number };
type RecoveryWord = {
  transformed: string;
  suggested: string;
  alternatives: string[];
  compatibleCount: number;
};
type Reconstruction = {
  suggested: string | null;
  words: RecoveryWord[];
  note: string;
};
type Decoded = { intermediate: string; reconstruction: Reconstruction };
type WordChoice = { index: number; candidate: string };

function wordOptions(word: RecoveryWord): string[] {
  return Array.from(new Set([word.suggested, ...word.alternatives]));
}

const STAGES = [
  { title: "Pronunciation lookup", detail: "Resolve dictionary-supported US-English phonemes", reversible: false },
  { title: "Rotating sound families", detail: "Choose key-derived spellings for each consonant phoneme", reversible: false },
  { title: "Vowel-run collapse", detail: "Merge consecutive vowel phonemes into a rotating vowel", reversible: false },
  { title: "Position-dependent shift", detail: "Shift letters using word index, letter index and passphrase", reversible: true },
];
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
  const [lockedChoices, setLockedChoices] = useState<WordChoice[]>([]);
  const [selectedWord, setSelectedWord] = useState<number | null>(null);
  const [choosing, setChoosing] = useState(false);
  const operationId = useRef(0);
  const [detail, setDetail] = useState("");
  const [busy, setBusy] = useState(false);
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

  const mode = appearance.mode === "system" ? (systemDark ? "dark" : "light") : appearance.mode;
  const theme = getTheme(mode === "light" ? appearance.lightTheme : appearance.darkTheme, mode);
  const style = paletteVars(theme, appearance);
  const availableThemes = THEMES.filter(t => t.variant === mode);
  const source = direction === "encode" ? plaintext : ciphertext;
  const inspectText = inspected === "intermediate" ? intermediate : output;
  const bars = useMemo(() => frequencies(inspectText), [inspectText]);
  const total = bars.reduce((sum, count) => sum + count, 0);

  function setLook(patch: Partial<Appearance>) {
    setAppearance(current => ({ ...current, ...patch }));
  }
  function clearResults() {
    operationId.current += 1;
    setOutput(""); setIntermediate(""); setReconstruction(null);
    setLockedChoices([]); setSelectedWord(null); setChoosing(false);
    setDetail(""); setMessage("");
  }
  async function run() {
    if (busy || choosing) return;
    const operation = ++operationId.current;
    setBusy(true); setChoosing(false); setMessage("");
    try {
      if (direction === "encode") {
        const result = await invoke<Encoded>("encode_message", { text: plaintext, key: passphrase });
        if (operation !== operationId.current) return;
        setOutput(result.ciphertext);
        setReconstruction(null);
        setLockedChoices([]); setSelectedWord(null);
        setCiphertext(result.ciphertext);
        setIntermediate(result.intermediate);
        setInspected("intermediate");
        setDetail(String(result.words) + " words / " + String(result.phonemes) + " phonemes");
        setMessage("Encoded successfully.");
      } else {
        const result = await invoke<Decoded>("decode_message", { ciphertext, key: passphrase });
        if (operation !== operationId.current) return;
        setLockedChoices([]); setSelectedWord(null);
        setOutput(result.reconstruction.suggested ?? "");
        setIntermediate(result.intermediate);
        setReconstruction(result.reconstruction);
        setInspected("ciphertext");
        setDetail(result.reconstruction.note);
        setMessage(result.reconstruction.suggested
          ? "Shift reversed; English is only a suggested reconstruction."
          : "Shift reversed, but no English reconstruction could be verified against the phonetic rules.");
      }
    } catch (error) {
      if (operation !== operationId.current) return;
      setOutput(""); setIntermediate(""); setReconstruction(null); setDetail("");
      setMessage(String(error));
    } finally {
      if (operation === operationId.current) setBusy(false);
    }
  }

  async function reviseChoices(next: WordChoice[], changed?: WordChoice) {
    if (!reconstruction || direction !== "decode" || choosing || busy) return;
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
      const result = await invoke<Decoded>("choose_reconstruction", {
        ciphertext, key: passphrase, choices: next,
      });
      if (operation !== operationId.current) return;
      if (!result.reconstruction.suggested) throw new Error("No compatible English reconstruction was found.");
      if (result.intermediate !== intermediate) throw new Error("The recovered intermediate changed unexpectedly.");
      setOutput(result.reconstruction.suggested);
      setReconstruction(result.reconstruction);
      setLockedChoices(next);
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

  const navItems: Array<{ page: Exclude<Page, "workbench">; title: string; icon: string }> = [
    { page: "general", title: "General", icon: "⚙" },
    { page: "appearance", title: "Appearance", icon: "☼" },
    { page: "about", title: "About", icon: "ⓘ" },
  ];
  return (
    <div className={"app-root " + mode} style={style}>
      <header className="topbar">
        <div className="brand"><span className="brand-icon">⌁</span><strong>Cryptography Playground</strong><span className="brand-division">/ Notebook Lab</span></div>
        <div className="top-controls">
          <span className="recipe-name">Recipe 001 — Phonetic Rotation</span>
          <button className="top-run" onClick={() => void run()} disabled={!source.trim() || !passphrase.trim() || busy || choosing}>▶ {busy ? "Running" : "Run"}</button>
          <button onClick={() => setPage("appearance")} aria-label="Appearance settings" title="Appearance settings">⚙</button>
        </div>
      </header>
      <div className="app-flex">
        <nav className="icon-rail" aria-label="Primary navigation">
          <button title="Workbench" aria-label="Workbench" className={page === "workbench" ? "active" : ""} onClick={() => setPage("workbench")}>▤</button>
          <button title="Appearance" aria-label="Appearance" className={page === "appearance" ? "active" : ""} onClick={() => setPage("appearance")}>◐</button>
          <div className="spacer" />
          <button title="About" aria-label="About" className={page === "about" ? "active" : ""} onClick={() => setPage("about")}>ⓘ</button>
        </nav>
        {page === "workbench" ? (
          <main className="workbench">
            <aside className="source-sidebar">
              <div className="sidebar-title"><h2>Experiment</h2><small>001 · unsaved</small></div>
              <label className="input-caption" htmlFor="operation-mode">Direction</label>
              <select id="operation-mode" value={direction} onChange={e => { setDirection(e.target.value as Direction); clearResults(); }}>
                <option value="encode">Encode plaintext</option><option value="decode">Decode / reconstruct English</option>
              </select>
              <label htmlFor="source-input" className="input-caption">{direction === "encode" ? "Plaintext" : "Ciphertext"}</label>
              <textarea id="source-input" rows={8} spellCheck={false} value={source} onChange={e => {
                if (direction === "encode") setPlaintext(e.target.value);
                else setCiphertext(e.target.value);
                clearResults();
              }} placeholder="Enter text" />
              <label className="input-caption" htmlFor="passphrase">Passphrase</label>
              <input id="passphrase" value={passphrase} maxLength={64} spellCheck={false} autoComplete="off"
                onChange={e => { setPassphrase(e.target.value); clearResults(); }} />
              <p className="helper-text">The current prototype accepts a manually selected letter-only key.</p>
              <button className="sidebar-run" onClick={() => void run()} disabled={busy || choosing || !source.trim() || !passphrase.trim()}>▶ {direction === "encode" ? "Encode" : "Decode / suggest English"}</button>
              <div className="sidebar-note">
                <strong>Experimental only</strong>
                <p>The lossy recipe cannot restore the exact original English. Never use it for real data security.</p>
              </div>
            </aside>
            <section className="notebook">
              <div className="notebook-tabs"><span className="active">◈ Phonetic Rotation / 001</span></div>
              <article className="notebook-page">
                <div className="punch-holes" aria-hidden="true">◦<span>◦</span><span>◦</span></div>
                <h1>Experiment: Phonetic Rotation</h1>
                <div className="meta-line"><span>Recipe 001</span><span>phonetics</span><span>lossy</span><span>keyed shift</span></div>
                <h2>Goal</h2>
                <p>Explore how pronunciation-based collapse makes the intermediate text ambiguous before it is hidden behind a reversible positional shift.</p>
                <h2>Recipe · transformation sequence</h2>
                <ol className="stage-list">
                  {STAGES.map((stage, i) => (
                    <li key={stage.title}>
                      <span className="stage-index">{String(i + 1).padStart(2, "0")}</span>
                      <div><strong>{stage.title}</strong><p>{stage.detail}</p></div>
                      <span className={"stage-type " + (stage.reversible ? "reversible" : "lossy")}>{stage.reversible ? "REV" : "LOSS"}</span>
                    </li>
                  ))}
                </ol>
                <h2>Transformation results</h2>
                <div className="result-grid">
                  <div><h3>{direction === "encode" ? "Phonetic intermediate" : "Exact recovered intermediate"}</h3><pre id="intermediate-result">{intermediate || "Run the recipe to inspect the intermediate text."}</pre></div>
                  <div>
                    <h3>{direction === "encode" ? "Final ciphertext" : "Suggested English (not verified)"}</h3>
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
                                disabled={choosing || busy} title={"Word " + (index + 1) + ": " + word.compatibleCount + " possible readings"}>
                                {word.suggested}
                              </button>
                            ) : word.suggested}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <pre id="cipher-result">{output || (direction === "decode" && intermediate ? "No compatible English suggestion found." : "No result yet.")}</pre>
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
                              disabled={choosing || busy}>{candidate}</button>
                          ))}
                        </div>
                        <small>Selections are checked against the rest of the message.</small>
                      </div>
                    )}
                    {direction === "decode" && intermediate && (
                      <>
                        <p className="reconstruction-warning">Click an underlined word in the English suggestion, or choose a candidate in the inspector. Each selection is checked against the full phonetic rotation; the original English still cannot be proven.</p>
                        <label htmlFor="english-edit" className="english-edit-label">Manual interpretation (candidate selections replace manual edits)</label>
                        <textarea id="english-edit" aria-label="Edit suggested English" value={output}
                          onChange={event => setOutput(event.target.value)} spellCheck={true} rows={2}
                          placeholder="Enter your preferred English interpretation..." />
                      </>
                    )}
                  </div>
                </div>
                <div className="notebook-actions">
                  <span>{choosing ? "Rechecking the selected words and subsequent rotations…" : (detail || "No run recorded")}</span>
                  {direction === "decode" && lockedChoices.length > 0 && (
                    <button disabled={choosing || busy} onClick={() => void reviseChoices([])}>Reset word selections ({lockedChoices.length})</button>
                  )}
                  <button disabled={!output || choosing} onClick={() => void copy(output)}>
                    {direction === "decode" ? "Copy English suggestion" : "Copy result"}
                  </button>
                </div>
              </article>
            </section>
            <aside className="inspector">
              <h2>Inspector</h2>
              <div className="inspector-tabs" role="group" aria-label="Inspected result">
                <button className={inspected === "intermediate" ? "active" : ""} onClick={() => setInspected("intermediate")}>Intermediate</button>
                <button className={inspected === "ciphertext" ? "active" : ""} onClick={() => setInspected("ciphertext")}>{direction === "decode" ? "English guess" : "Output"}</button>
              </div>
              <div className="inspector-block"><h3>Selected result</h3><pre>{inspectText || "No result to inspect."}</pre></div>
              <div className="inspector-block"><h3>Properties</h3>
                <dl className="data-rows">
                  <div><dt>Characters</dt><dd>{inspectText.length}</dd></div>
                  <div><dt>Letters</dt><dd>{total}</dd></div>
                  <div><dt>Original length</dt><dd>{source.length}</dd></div>
                  <div><dt>Recipe</dt><dd>001</dd></div>
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
                            disabled={busy || choosing}
                            onClick={() => chooseCandidate(index, candidate)}>
                            {candidate}
                          </button>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
              <div className="inspector-block"><h3>Reversibility</h3><p>The shift is reversible with the key. Phonetic normalization and vowel runs are lossy; the English suggestion is speculative, even when re-encoding a candidate matches.</p></div>
            </aside>
          </main>
        ) : (
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
                    <div className="settings-row"><span className="setting-label">Recipe</span><span>001 — Phonetic Rotation</span></div>
                    <div className="settings-row"><span className="setting-label">Preferences</span><span>Saved locally</span></div>
                  </section><p className="helper-text">No cloud account or synchronization is configured.</p>
                </div>
              ) : (
                <div className="settings-inner"><h1>About</h1><h2>Cryptography Playground</h2>
                  <section className="settings-group"><div className="settings-row"><span>Current experiment</span><span>Recipe 001</span></div>
                    <div className="settings-row"><span>Technology</span><span>Rust · React · Tauri 2</span></div>
                  </section><p className="helper-text">An independent, experimental cryptography playground. It is not suitable for protecting sensitive data.</p>
                </div>
              )}
            </article>
          </main>
        )}
      </div>
      <footer className="statusbar"><span>◈ Cryptography Playground</span><span role="status">{page === "workbench" && message ? message : (theme.name + " / " + mode)}</span><span>{busy ? "Working…" : "Ready"}</span></footer>
    </div>
  );
}
export default App;
