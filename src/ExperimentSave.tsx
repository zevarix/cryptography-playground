import { useEffect, useId, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ExperimentSummary, ExperimentTarget } from "./experimentTypes";

type Props = { target: ExperimentTarget; defaultName: string; disabled: boolean; onSaved: (entry: ExperimentSummary) => void; onBusy: (value: boolean) => void };
export default function ExperimentSave({ target, defaultName, disabled, onSaved, onBusy }: Props) {
  const id = useId();
  const [name, setName] = useState(defaultName);
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const pending = useRef(false);
  const identity = JSON.stringify(target);
  useEffect(() => { setName(defaultName);setNotice("");setError(""); }, [identity, defaultName]);
  async function save() {
    if (disabled || pending.current) return;
    if (!name.trim()) { setError("Enter a name for this experiment.");return; }
    pending.current = true;setSaving(true);onBusy(true);setError("");setNotice("");
    try {
      const { kind, ...reference } = target;
      const entry = await invoke<ExperimentSummary>(kind === "text" ? "save_text_experiment" : "save_file_experiment", { ...reference, name: name.trim() });
      setNotice("Saved “" + entry.name + "” in Experiments.");onSaved(entry);
    } catch (reason) { setError(String(reason)); }
    finally { pending.current = false;setSaving(false);onBusy(false); }
  }
  return <form className="experiment-save" aria-label="Save a local experiment" onSubmit={event => { event.preventDefault();void save(); }}>
    <h3>Keep this experiment</h3>
    <p id={id + "-hint"}>Save the completed run, its stages and selected reading on this computer. Passphrases are kept separate.</p>
    <label htmlFor={id}>Experiment name</label>
    <div className="experiment-save-row"><input id={id} name="experimentName" value={name} required maxLength={100} disabled={disabled || saving} aria-describedby={id + "-hint"} aria-invalid={error && !name.trim() ? true : undefined} onChange={event => { setName(event.target.value);setError(""); }} />
      <button type="submit" disabled={disabled || saving}>{saving ? "Saving experiment…" : "Save experiment"}</button></div>
    {saving && <progress aria-label="Saving experiment" />}
    {error && <p role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
  </form>;
}
