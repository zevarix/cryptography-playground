import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  copyRecipe, createRule, lossyRule, newUserRecipe,
  RULE_CATALOG, type Recipe, type RecipeFingerprint, type Rule, type RuleKind, type Step,
} from "./recipeTypes";

type Props = {
  builtins: Recipe[];
  saved: Recipe[];
  active: Recipe | null;
  busy: boolean;
  onActivate: (recipe: Recipe) => void;
  onSave: (recipe: Recipe) => Promise<Recipe>;
  onDelete: (id: string) => Promise<void>;
  onNotice: (message: string) => void;
  onWorkbench: () => void;
};

// Keep engine rule implementations independent of saved recipe examples.
function RuleOptions() {
  const groups: Array<{ name: string; types: RuleKind[] }> = [
    { name: "Linguistic & experimental", types: [
      "phonetic_rotate", "remove_vowels", "collapse_doubles", "word_mirror",
      "pair_swap", "rotate_vowels", "position_shift",
    ] },
    { name: "Classical substitution (ciphers)", types: [
      "caesar", "atbash", "vigenere", "beaufort", "affine",
    ] },
    { name: "Classical transposition (ciphers)", types: ["rail_fence"] },
    { name: "Passphrase-derived parameters", types: ["derived_caesar", "derived_rail_fence"] },
  ];
  return <>{groups.map(group => (
    <optgroup key={group.name} label={group.name}>
      {RULE_CATALOG.filter(item => group.types.includes(item.type)).map(item =>
        <option key={item.type} value={item.type}>{item.label}</option>)}
    </optgroup>
  ))}</>;
}

export default function RecipePage({
  builtins, saved, active, busy, onActivate, onSave, onDelete, onNotice, onWorkbench,
}: Props) {
  const [draft, setDraft] = useState<Recipe>(() => active ? copyRecipe(active) : newUserRecipe());
  const [typeToAdd, setTypeToAdd] = useState<RuleKind>("word_mirror");
  const [jsonImport, setJsonImport] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [localError, setLocalError] = useState("");
  const [saving, setSaving] = useState(false);
  const [fingerprint, setFingerprint] = useState<RecipeFingerprint | null>(null);

  useEffect(() => {
    if (active) setDraft(copyRecipe(active));
  }, [active]);
  useEffect(() => {
    let current = true;
    setFingerprint(null);
    const job = window.setTimeout(() => {
      void invoke<RecipeFingerprint>("recipe_fingerprint", { recipe: draft })
        .then(result => { if (current) setFingerprint(result); })
        .catch(() => { if (current) setFingerprint(null); });
    }, 180);
    return () => { current = false; window.clearTimeout(job); };
  }, [draft]);

  const isBuiltin = draft.id.startsWith("builtin-");
  const referenceRecipe = saved.find(recipe => recipe.id === draft.id)
    ?? builtins.find(recipe => recipe.id === draft.id);
  const isSaved = saved.some(recipe => recipe.id === draft.id);
  const hasUnsavedChanges = !referenceRecipe ||
    JSON.stringify(draft) !== JSON.stringify(referenceRecipe);
  const recipeSaveLabel = hasUnsavedChanges
    ? (isSaved || isBuiltin ? "Unsaved changes" : "Unsaved recipe")
    : isBuiltin ? "Bundled example" : "Saved locally";
  const processing = saving || busy;
  const update = (change: Partial<Recipe>) => { setDraft(current => ({ ...current, ...change })); setLocalError(""); };
  const updateStep = (index: number, change: Partial<Step>) => {
    setDraft(current => ({
      ...current,
      steps: current.steps.map((step, i) => i === index ? { ...step, ...change } : step),
    }));
  };
  const updateRule = (index: number, next: Rule) => updateStep(index, { rule: next });
  const removeStep = (index: number) => update({ steps: draft.steps.filter((_, i) => i !== index) });
  const moveStep = (index: number, by: -1 | 1) => {
    const target = index + by;
    if (target < 0 || target >= draft.steps.length) return;
    const steps = draft.steps.slice();
    [steps[index], steps[target]] = [steps[target], steps[index]];
    update({ steps });
  };
  const duplicate = () => {
    const next = copyRecipe(draft);
    next.id = newUserRecipe().id;
    next.name = draft.name + " (copy)";
    setDraft(next);
    onNotice("Working on a separate copy. Save it when ready.");
  };
  const save = async () => {
    setSaving(true); setLocalError("");
    try {
      const data = copyRecipe(draft);
      if (data.id.startsWith("builtin-")) {
        data.id = newUserRecipe().id;
        data.name = data.name + " (custom)";
      }
      const verified = await onSave(data);
      setDraft(copyRecipe(verified));
      onNotice("Saved " + verified.name + " locally. It is now the active recipe.");
    } catch (err) { setLocalError(String(err)); }
    finally { setSaving(false); }
  };
  const testDraft = () => {
    onActivate(copyRecipe(draft));
    onWorkbench();
    onNotice("Using the current recipe configuration. Save it to retain your changes.");
  };
  const exportRecipe = async () => {
    const json = JSON.stringify(draft, null, 2) + "\n";
    try {
      await navigator.clipboard.writeText(json);
      onNotice("Recipe JSON copied to the clipboard. Save it as a .json file to share.");
    } catch {
      setJsonImport(json);
      setImportOpen(true);
      onNotice("Select and copy the JSON shown in Import / Export.");
    }
  };
  const importRecipe = async () => {
    setSaving(true); setLocalError("");
    try {
      const parsed = JSON.parse(jsonImport) as Recipe;
      parsed.id = newUserRecipe().id;
      const savedRecipe = await onSave(parsed);
      setDraft(copyRecipe(savedRecipe));
      setJsonImport("");
      setImportOpen(false);
      onNotice("Imported recipe saved locally.");
    } catch (err) { setLocalError("Import failed: " + String(err)); }
    finally {setSaving(false);}
  };
  const pickFile = async (file: File | undefined) => {
    if (!file) return;
    if (file.size > 64 * 1024) { setLocalError("Recipe file exceeds 64 KiB."); return; }
    const content = await file.text();
    setJsonImport(content);
    setImportOpen(true);
  };
  return (
    <main className="recipe-layout">
      <aside className="recipe-library">
        <div className="pane-heading"><h2>Recipe Library</h2><button aria-label="Create a new recipe" title="New recipe"
          onClick={() => {setDraft(newUserRecipe());setLocalError("");}}>＋</button></div>
        <p className="recipe-sidebar-note">Bundled recipes are editable as copies. Your recipes are saved privately on this computer.</p>
        <div className="recipe-section-label">BUILT-IN EXAMPLES</div>
        {builtins.map(recipe => (
          <button key={recipe.id} className={"recipe-library-entry" + (draft.id === recipe.id ? " active" : "")}
            onClick={() => {setDraft(copyRecipe(recipe));setLocalError("");}}>
            <strong>{recipe.name}</strong><small>{recipe.id.replace("builtin-","Recipe ")}</small>
          </button>
        ))}
        <div className="recipe-section-label">MY RECIPES · {saved.length}</div>
        {saved.length === 0 && <p className="recipe-sidebar-note">No saved recipes yet.</p>}
        {saved.map(recipe => (
          <button key={recipe.id} className={"recipe-library-entry" + (draft.id === recipe.id ? " active" : "")}
            onClick={() => {setDraft(copyRecipe(recipe));setLocalError("");}}>
            <strong>{recipe.name}</strong><small>{recipe.steps.filter(s => s.enabled).length} active rules</small>
          </button>
        ))}
      </aside>
      <section className="recipe-editor">
        <div className="recipe-editor-top">
          <div>
            <span className="section-kicker">RECIPE EDITOR · SCHEMA 1</span>
            <h1>{isBuiltin ? "Customize a bundled recipe" : "Build your own recipe"}</h1>
            <p>Compose and configure rules; the order determines the transformation.</p>
            <p className={"recipe-edit-status" + (hasUnsavedChanges ? " unsaved" : "")}
              role="status" aria-label="Recipe save status">
              {recipeSaveLabel}
              {hasUnsavedChanges && " · Save to keep this version after restarting."}
            </p>
          </div>
          <div className="recipe-editor-actions">
            <button onClick={duplicate} disabled={processing}>Duplicate</button>
            <button onClick={() => void save()} className="primary" disabled={processing}>
              {saving ? "Saving…" : isBuiltin ? "Save a copy" : "Save recipe"}
            </button>
            <button onClick={testDraft} disabled={processing}>Use in workbench</button>
          </div>
        </div>
        <div className="recipe-basics">
          <label htmlFor="recipe-name">Name
            <input id="recipe-name" value={draft.name} maxLength={100}
              onChange={event => update({ name: event.target.value })} />
          </label>
          <label htmlFor="recipe-description">Description
            <textarea id="recipe-description" rows={2} value={draft.description} maxLength={1024}
              onChange={event => update({ description: event.target.value })} />
          </label>
          {isBuiltin && <p className="recipe-notice">Bundled examples cannot be overwritten. Saving creates an independent, editable copy.</p>}
        </div>
        <section className="recipe-fingerprint" aria-label="Recipe fingerprint">
          <div>
            <strong>Recipe configuration fingerprint</strong>
            <p>Identifies the ordered rules and parameters, not its name or passphrase.
              It is a comparison code, not a secret or proof of authenticity.</p>
          </div>
          <code title={fingerprint?.value ?? "Recipe configuration is invalid"} data-testid="recipe-fingerprint">
            {fingerprint?.display ?? "Unavailable"}
          </code>
          <button type="button" disabled={!fingerprint} aria-label="Copy complete recipe fingerprint"
            onClick={() => {
              if (!fingerprint) return;
              void navigator.clipboard.writeText(fingerprint.value).then(
                () => onNotice("Full SHA-256 recipe fingerprint copied."),
                () => onNotice("Unable to copy fingerprint. You can inspect it by hovering over the code.")
              );
            }}>Copy hash</button>
        </section>
        <div className="recipe-rule-heading"><h2>Rules · {draft.steps.filter(step => step.enabled).length} enabled</h2>
          <span>Run top to bottom · Reverse bottom to top when possible</span></div>
        <ol className="recipe-step-list" aria-label="Ordered recipe rules">
          {draft.steps.map((step, index) => (
            <li key={index} className={"recipe-step" + (!step.enabled ? " disabled" : "")}>
              <div className="recipe-step-controls">
                <span className="recipe-number">{String(index + 1).padStart(2,"0")}</span>
                <label className="recipe-toggle"><input aria-label={"Enable rule " + (index + 1)} type="checkbox"
                  checked={step.enabled} onChange={event => updateStep(index,{enabled:event.target.checked})} /> Enabled</label>
                <button aria-label={"Move rule " + (index + 1) + " up"} title="Move up"
                  disabled={index === 0} onClick={() => moveStep(index,-1)}>↑</button>
                <button aria-label={"Move rule " + (index + 1) + " down"} title="Move down"
                  disabled={index === draft.steps.length-1} onClick={() => moveStep(index,1)}>↓</button>
                <button aria-label={"Remove rule " + (index + 1)} title="Remove rule"
                  onClick={() => removeStep(index)}>×</button>
              </div>
              <div className="recipe-step-body">
                <div className="recipe-rule-main">
                  <label htmlFor={"rule-" + index}>Rule</label>
                  <select id={"rule-" + index} value={step.rule.type} onChange={event =>
                    updateRule(index,createRule(event.target.value as RuleKind))}>
                    <RuleOptions />
                  </select>
                  <span className={"recipe-rule-kind" + (lossyRule(step.rule) ? " loss" : "")}>
                    {lossyRule(step.rule) ? "LOSS" : "REV"}
                  </span>
                </div>
                <p>{RULE_CATALOG.find(r => r.type === step.rule.type)?.detail}</p>
                {step.rule.type === "position_shift" && (
                  <div className="rule-settings">
                    <label htmlFor={"shift-base-" + index}>Base offset
                      <input id={"shift-base-" + index} type="number" min={0} max={25}
                        value={step.rule.base} onChange={event =>
                          updateRule(index, {type:"position_shift",base:Number(event.target.value),keyed:(step.rule as Extract<Rule,{type:"position_shift"}>).keyed})} />
                    </label>
                    <label className="inline-checkbox">
                      <input type="checkbox" aria-label={"Use passphrase in shift " + (index + 1)}
                        checked={step.rule.keyed} onChange={event =>
                          updateRule(index,{type:"position_shift",base:(step.rule as Extract<Rule,{type:"position_shift"}>).base,keyed:event.target.checked})} />
                      Include passphrase
                    </label>
                  </div>
                )}
                {step.rule.type === "rotate_vowels" && (
                  <div className="rule-settings">
                    <label htmlFor={"vowel-steps-" + index}>Vowel positions
                      <input id={"vowel-steps-" + index} type="number" min={0} max={4}
                        value={step.rule.steps} onChange={event =>
                          updateRule(index,{type:"rotate_vowels",steps:Number(event.target.value)})} />
                    </label>
                  </div>
                )}
                {step.rule.type === "caesar" && (
                  <div className="rule-settings">
                    <label htmlFor={"caesar-shift-" + index}>Alphabet shift
                      <input id={"caesar-shift-" + index} type="number" min={0} max={25}
                        value={step.rule.shift} onChange={event =>
                          updateRule(index,{type:"caesar",shift:Number(event.target.value)})} />
                    </label>
                    <span>13 = ROT13</span>
                  </div>
                )}
                {step.rule.type === "affine" && (
                  <div className="rule-settings">
                    <label htmlFor={"affine-a-" + index}>Multiplier (coprime to 26)
                      <select id={"affine-a-" + index} value={step.rule.a}
                        onChange={event => updateRule(index,{
                          type:"affine",a:Number(event.target.value),
                          b:(step.rule as Extract<Rule,{type:"affine"}>).b
                        })}>
                        {[1,3,5,7,9,11,15,17,19,21,23,25].map(a =>
                          <option key={a} value={a}>{a}</option>)}
                      </select>
                    </label>
                    <label htmlFor={"affine-b-" + index}>Offset
                      <input id={"affine-b-" + index} type="number" min={0} max={25}
                        value={step.rule.b} onChange={event => updateRule(index,{
                          type:"affine",a:(step.rule as Extract<Rule,{type:"affine"}>).a,
                          b:Number(event.target.value)
                        })} />
                    </label>
                  </div>
                )}
                {step.rule.type === "rail_fence" && (
                  <div className="rule-settings">
                    <label htmlFor={"rail-count-" + index}>Rails
                      <input id={"rail-count-" + index} type="number" min={2} max={16}
                        value={step.rule.rails} onChange={event =>
                          updateRule(index,{type:"rail_fence",rails:Number(event.target.value)})} />
                    </label>
                  </div>
                )}
                {(step.rule.type === "vigenere" || step.rule.type === "beaufort") && (
                  <p className="rule-key-note">Uses the workbench passphrase. No keyword is embedded in this recipe JSON.</p>
                )}
                {(step.rule.type === "derived_caesar" || step.rule.type === "derived_rail_fence") && (
                  <p className="rule-key-note">Rule value is derived at runtime from the recipe configuration,
                    this step's position, and the workbench passphrase. The passphrase is never saved in JSON.
                    Resolved values appear in the transformation stages; this is for puzzles, not security.</p>
                )}
              </div>
            </li>
          ))}
        </ol>
        <div className="recipe-add-row">
          <select aria-label="Rule to add" value={typeToAdd} onChange={e => setTypeToAdd(e.target.value as RuleKind)}>
            <RuleOptions />
          </select>
          <button disabled={draft.steps.length >= 16} onClick={() => update({steps:[...draft.steps,{enabled:true,rule:createRule(typeToAdd)}]})}>＋ Add rule</button>
        </div>
        {localError && <p role="alert" className="recipe-error">{localError}</p>}
        <div className="recipe-actions-footer">
          <button onClick={() => void exportRecipe()}>Copy JSON / Export</button>
          <button onClick={() => setImportOpen(!importOpen)}>Import JSON</button>
          {isSaved && <button className="danger-action" onClick={() => {
            if (confirm("Delete this locally saved recipe? This cannot be undone.")) {
              void onDelete(draft.id).then(() => {setDraft(newUserRecipe());}).catch(e=>setLocalError(String(e)));
            }
          }}>Delete saved recipe</button>}
        </div>
        {importOpen && (
          <div className="recipe-import">
            <label htmlFor="recipe-json">Portable recipe JSON</label>
            <textarea id="recipe-json" rows={6} value={jsonImport} onChange={e=>setJsonImport(e.target.value)}
              placeholder="Paste a recipe JSON document here…" spellCheck={false} />
            <label className="recipe-file-select" htmlFor="recipe-file-input">Or choose a .json file</label>
            <input id="recipe-file-input" type="file" accept=".json,application/json"
              onChange={event => { void pickFile(event.target.files?.[0]); }} />
            <button className="primary" disabled={processing || !jsonImport.trim()}
              onClick={() => void importRecipe()}>Import and save locally</button>
          </div>
        )}
      </section>
    </main>
  );
}
