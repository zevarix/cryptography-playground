import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Mode = "encode" | "decode";
type Encoded = {
  ciphertext: string;
  intermediate: string;
  words: number;
  phonemes: number;
};
type Decoded = { intermediate: string };

const pipeline = [
  { number: "01", name: "Pronunciation lookup", detail: "US English phoneme dictionary" },
  { number: "02", name: "Rotating sound families", detail: "Key-seeded spellings for every consonant phoneme" },
  { number: "03", name: "Vowel-run collapse", detail: "One rotating vowel per consecutive vowel sound group" },
  { number: "04", name: "Position shifting", detail: "Letter + word position + repeating key" },
  { number: "05", name: "Structure reduction", detail: "Keep spaces; discard punctuation and casing" },
];

function App() {
  const [mode, setMode] = useState<Mode>("encode");
  const [plaintext, setPlaintext] = useState(
    "The quick brown fox jumps over the lazy dog."
  );
  const [ciphertext, setCiphertext] = useState("");
  const [passphrase, setPassphrase] = useState("spectacular");
  const [output, setOutput] = useState("");
  const [intermediate, setIntermediate] = useState("");
  const [details, setDetails] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [copyState, setCopyState] = useState("");

  const input = mode === "encode" ? plaintext : ciphertext;
  const edit = (value: string) => {
    if (mode === "encode") setPlaintext(value);
    else setCiphertext(value);
    setOutput("");
    setIntermediate("");
    setError("");
    setDetails("");
    setCopyState("");
  };
  const changeMode = (nextMode: Mode) => {
    setMode(nextMode);
    setOutput("");
    setIntermediate("");
    setDetails("");
    setError("");
    setCopyState("");
  };

  async function run() {
    setBusy(true);
    setError("");
    setCopyState("");
    try {
      if (mode === "encode") {
        const result = await invoke<Encoded>("encode_message", {
          text: plaintext,
          key: passphrase,
        });
        setOutput(result.ciphertext);
        setCiphertext(result.ciphertext);
        setIntermediate(result.intermediate);
        setDetails(result.words + " words / " + result.phonemes + " pronunciation units");
      } else {
        const result = await invoke<Decoded>("decode_message", {
          ciphertext,
          key: passphrase,
        });
        setOutput(result.intermediate);
        setIntermediate("");
        setDetails("Recovered pre-shift text only");
      }
    } catch (cause) {
      setOutput("");
      setIntermediate("");
      setDetails("");
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function copyOutput() {
    try {
      await navigator.clipboard.writeText(output);
      setCopyState("Copied");
    } catch {
      setCopyState("Clipboard unavailable; select the output to copy.");
    }
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">CP</div>
          <div>
            <div className="brand-name">Cryptography Playground</div>
            <div className="brand-subtitle">Experimental cipher laboratory</div>
          </div>
        </div>
        <div className="lab-status"><span className="status-light" /> Local experiment</div>
      </header>

      <div className="hero">
        <div className="eyebrow">RECIPE ENGINE / EXPERIMENT 001</div>
        <h1>Turn language into a puzzle.</h1>
        <p>Build, inspect, and reverse intentionally lossy text transformations.
          Try to recover what is mathematically reversible, then see what language has lost.</p>
      </div>

      <div className="workspace">
        <aside className="pipeline-panel">
          <div className="section-kicker">ACTIVE RECIPE</div>
          <h2>Phonetic rotation</h2>
          <p className="muted">Complete ARPABET phoneme inventory with independent cycling spelling families.</p>
          <div className="pipeline-list">
            {pipeline.map((step) => (
              <div className="pipeline-item" key={step.number}>
                <span className="step-number">{step.number}</span>
                <div>
                  <strong>{step.name}</strong>
                  <p>{step.detail}</p>
                </div>
              </div>
            ))}
          </div>
          <div className="panel-footnote">
            <span className="dot" /> Fixed Recipe 001 prototype. Configurable rule selection comes next.
          </div>
        </aside>

        <main className="editor-panel">
          <div className="editor-top">
            <div>
              <div className="section-kicker">WORKBENCH</div>
              <h2>Encoder / Decoder</h2>
            </div>
            <span className="pill">Rust engine</span>
          </div>
          <div className="tabs" role="group" aria-label="Cipher operation">
            <button className={mode === "encode" ? "tab active" : "tab"}
              onClick={() => changeMode("encode")} aria-pressed={mode === "encode"}>
              Encode text
            </button>
            <button className={mode === "decode" ? "tab active" : "tab"}
              onClick={() => changeMode("decode")} aria-pressed={mode === "decode"}>
              Decode ciphertext
            </button>
          </div>

          <label className="field-label" htmlFor="message">
            {mode === "encode" ? "PLAINTEXT" : "CIPHERTEXT"}
          </label>
          <textarea id="message" value={input} onChange={(event) => edit(event.target.value)}
            spellCheck={false} placeholder={mode === "encode" ? "Write an English message..." : "Paste a ciphertext..."} />
          <div className="field-label key-label">
            <label htmlFor="passphrase">KEY / PASSPHRASE</label>
            <span>Manual for this prototype</span>
          </div>
          <input id="passphrase" value={passphrase} onChange={(event) => {
            setPassphrase(event.target.value);
            setOutput("");
            setIntermediate("");
          }} placeholder="Enter a letter-only key" maxLength={64} autoComplete="off" />
          <p className="help">Automatic longest-adjective key selection is planned. No message is sent to a server.</p>

          <button className="run-button" disabled={busy || !input.trim()} onClick={run}>
            {busy ? "Processing..." : mode === "encode" ? "Encode with Recipe 001" : "Reverse the shift"}
            <span aria-hidden="true"> &rarr;</span>
          </button>
          {error && <p role="alert" className="error">{error}</p>}

          <div className="output-heading">
            <label className="field-label" htmlFor="result">RESULT</label>
            <span>{details || "Output will appear after running the engine"}</span>
          </div>
          <textarea id="result" className="output" value={output} readOnly
            placeholder="Encrypted output will appear here." aria-label="Result" />
          <div className="result-actions">
            <button disabled={!output} onClick={copyOutput}>Copy output</button>
            {mode === "encode" && (
              <button disabled={!output} onClick={() => changeMode("decode")}>Inspect in decoder</button>
            )}
            {copyState && <span className="copy-state" role="status">{copyState}</span>}
          </div>
          {intermediate && (
            <details className="intermediate">
              <summary>Reveal pre-shift phonetic transformation</summary>
              <pre>{intermediate}</pre>
            </details>
          )}
        </main>

        <aside className="notes-panel">
          <div className="section-kicker">RESEARCH NOTES</div>
          <h2>Loss is the point.</h2>
          <p>Phonetic spelling choices, vowels, punctuation, and casing are discarded
            before the reversible position shift.</p>
          <div className="note-block">
            <strong>What the decoder knows</strong>
            <p>With the correct key it can recover the exact transformed stream, not the original English.</p>
          </div>
          <div className="note-block">
            <strong>Pronunciation coverage</strong>
            <p>Uses a bundled, attributed US-English pronunciation dictionary. Unknown words fail explicitly.</p>
          </div>
          <div className="warning-note">
            <strong>Not secure encryption</strong>
            <p>For experiments and puzzle-making only. Never protect passwords or sensitive data with these recipes.</p>
          </div>
        </aside>
      </div>
      <footer>CRYPT / PLAYGROUND <span>Independent desktop application &middot; experimental build</span></footer>
    </div>
  );
}

export default App;
