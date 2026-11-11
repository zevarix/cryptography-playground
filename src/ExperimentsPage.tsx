import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ExperimentDialog from "./ExperimentDialog";
import type { ExperimentList, ExperimentSummary } from "./experimentTypes";

type Props = { busy: boolean; revision: number; onOpen: (entry: ExperimentSummary) => Promise<void> };
function date(seconds: string): string {
  const value = new Date(Number(seconds) * 1000);
  return Number.isNaN(value.valueOf()) ? "Saved locally" : value.toLocaleString();
}
export default function ExperimentsPage({ busy, revision, onOpen }: Props) {
  const [list, setList] = useState<ExperimentList>({ entries: [], problems: [] });
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState<ExperimentSummary | null>(null);
  const pending = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let current = true;setLoading(true);setError("");
    void invoke<ExperimentList>("list_experiments").then(value => { if (current) setList(value); })
      .catch(reason => { if (current) setError(String(reason)); }).finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [revision, refresh]);
  async function open(entry: ExperimentSummary) {
    if (pending.current || busy) return;
    pending.current = true;setWorking(true);setError("");
    try { await onOpen(entry); } catch (reason) { setError(String(reason)); }
    finally { pending.current = false;setWorking(false); }
  }
  async function remove(entry: ExperimentSummary) {
    if (pending.current || busy) return;
    setDeleting(null);pending.current = true;setWorking(true);setError("");
    try { await invoke("delete_experiment", { id: entry.id });setRefresh(value => value + 1); }
    catch (reason) { setError(String(reason)); }
    finally { pending.current = false;setWorking(false); }
  }
  const disabled = busy || working || loading;
  return <main className="experiments-page">
    <div className="experiments-heading"><div><h1 ref={heading} tabIndex={-1}>Experiments</h1><p>Saved runs on this computer. Open one to inspect it or continue working.</p></div><button type="button" disabled={disabled} onClick={() => setRefresh(value => value + 1)}>Refresh experiments</button></div>
    <p className="helper-text">Inputs, results, stages and chosen readings are retained only when you save. Passphrases are never included. Saving another run creates another entry.</p>
    {loading && <progress aria-label="Loading experiments" />}
    {working && <progress aria-label="Opening or deleting experiment" />}
    {error && <p role="alert">{error}</p>}
    {list.problems.length > 0 && <details className="experiment-problems" open><summary>Some saved experiments need attention</summary><ul>{list.problems.map((problem, index) => <li key={index}>{problem}</li>)}</ul></details>}
    {!loading && list.entries.length === 0 && <p>No saved experiments yet.</p>}
    <ul className="experiment-list">{list.entries.map(entry => <li key={entry.id}>
      <div><h2>{entry.name}</h2><p>{entry.recipeName} · {entry.kind === "file" ? "File" : "Text"} · {date(entry.createdAt)}</p></div>
      <div className="document-actions"><button type="button" disabled={disabled} aria-label={"Open " + entry.name} onClick={() => void open(entry)}>Open</button><button type="button" disabled={disabled} aria-label={"Delete " + entry.name} onClick={() => setDeleting(entry)}>Delete</button></div>
    </li>)}</ul>
    {deleting && <ExperimentDialog title="Delete saved experiment?" confirmLabel="Delete experiment" onConfirm={() => void remove(deleting)} onCancel={() => setDeleting(null)}><p>Delete “{deleting.name}” from this computer's experiment library? Its original input and any open working copy are kept.</p></ExperimentDialog>}
  </main>;
}
