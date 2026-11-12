import { useEffect, useId, useRef } from "react";

type Props = { title: string; children: React.ReactNode; confirmLabel: string; onConfirm: () => void; onCancel: () => void };
export default function ExperimentDialog({ title, children, confirmLabel, onConfirm, onCancel }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const id = useId();
  useEffect(() => { const dialog = ref.current;dialog?.showModal();return () => dialog?.close(); }, []);
  return <dialog ref={ref} className="document-confirm experiment-dialog" aria-labelledby={id} onCancel={event => { event.preventDefault();onCancel(); }}>
    <h2 id={id}>{title}</h2>{children}
    <div className="document-actions"><button type="button" onClick={onConfirm}>{confirmLabel}</button><button type="button" autoFocus onClick={onCancel}>Cancel</button></div>
  </dialog>;
}
