export type Rule =
  | { type: "phonetic_rotate" }
  | { type: "remove_vowels" }
  | { type: "collapse_doubles" }
  | { type: "word_mirror" }
  | { type: "pair_swap" }
  | { type: "rotate_vowels"; steps: number }
  | { type: "position_shift"; base: number; keyed: boolean }
  | { type: "caesar"; shift: number }
  | { type: "atbash" }
  | { type: "vigenere" }
  | { type: "beaufort" }
  | { type: "affine"; a: number; b: number }
  | { type: "rail_fence"; rails: number }
  | { type: "derived_caesar" }
  | { type: "derived_rail_fence" };

export type Step = { enabled: boolean; rule: Rule };
export type Recipe = {
  schemaVersion: 1 | 2;
  id: string;
  name: string;
  description: string;
  steps: Step[];
};
export type RuleKind = Rule["type"];
export type RecipeFingerprint = {
  algorithm: "SHA-256";
  version: 1 | 2;
  value: string;
  display: string;
};
export type StageOutput = { label: string; text: string; lossy: boolean };
export type WordChoice = { index: number; candidate: string };
export type RecoveryWord = {
  transformed: string;
  suggested: string;
  alternatives: string[];
  compatibleCount: number;
};
export type Reconstruction = {
  suggested: string | null;
  words: RecoveryWord[];
  note: string;
};
export type RecipeResult = {
  output: string;
  intermediate: string;
  exact: boolean;
  reconstruction: Reconstruction | null;
  note: string;
  stages: StageOutput[];
};

export const RULE_CATALOG: Array<{
  type: RuleKind; label: string; detail: string; lossy: boolean;
}> = [
  { type: "phonetic_rotate", label: "Phonetic rotation", detail: "CMU pronunciation + key-seeded sound families and vowel runs", lossy: true },
  { type: "remove_vowels", label: "Remove vowels", detail: "Delete A/E/I/O/U from words", lossy: true },
  { type: "collapse_doubles", label: "Collapse doubled consonants", detail: "Reduce consecutive repeated consonants", lossy: true },
  { type: "word_mirror", label: "Word mirror", detail: "Reverse letters within each word", lossy: false },
  { type: "pair_swap", label: "Voiced/unvoiced pairs", detail: "Swap B/P, D/T, G/K, V/F, Z/S", lossy: false },
  { type: "rotate_vowels", label: "Vowel rotation", detail: "Cycle A→E→I→O→U", lossy: false },
  { type: "position_shift", label: "Position shift", detail: "Shift using word, character, and optionally key position", lossy: false },
  { type: "caesar", label: "Caesar / ROT13", detail: "Classical alphabet shift (13 is ROT13).", lossy: false },
  { type: "atbash", label: "Atbash", detail: "Reverse A-Z using a classical Affine substitution.", lossy: false },
  { type: "vigenere", label: "Vigenere", detail: "Repeating-key substitution using the workbench passphrase.", lossy: false },
  { type: "beaufort", label: "Beaufort", detail: "Reciprocal repeating-key substitution using the workbench passphrase.", lossy: false },
  { type: "affine", label: "Affine", detail: "Alphabet transformation (a × letter + b) mod 26.", lossy: false },
  { type: "rail_fence", label: "Rail Fence", detail: "Zigzag transposition of every ASCII character, including punctuation.", lossy: false },
  { type: "derived_caesar", label: "Derived Caesar shift", detail: "Deterministic Caesar shift (0-25) derived from this recipe's rules, rule position, and your passphrase.", lossy: false },
  { type: "derived_rail_fence", label: "Derived Rail Fence", detail: "Deterministic Rail Fence count (2-16) derived from this recipe's rules, rule position, and your passphrase.", lossy: false },
];

export function labelForRule(rule: Rule): string {
  return RULE_CATALOG.find(x => x.type === rule.type)?.label ?? rule.type;
}
export function lossyRule(rule: Rule): boolean {
  return RULE_CATALOG.some(x => x.type === rule.type && x.lossy);
}
export function createRule(type: RuleKind): Rule {
  if (type === "rotate_vowels") return { type, steps: 1 };
  if (type === "position_shift") return { type, base: 5, keyed: false };
  if (type === "caesar") return { type, shift: 13 };
  if (type === "affine") return { type, a: 5, b: 8 };
  if (type === "rail_fence") return { type, rails: 3 };
  return { type } as Rule;
}
export function recipeNeedsKey(recipe: Recipe | null): boolean {
  return recipe?.steps.some(s => s.enabled &&
    (s.rule.type === "phonetic_rotate" ||
      (s.rule.type === "position_shift" && s.rule.keyed) ||
      s.rule.type === "vigenere" ||
      s.rule.type === "beaufort" ||
      s.rule.type === "derived_caesar" ||
      s.rule.type === "derived_rail_fence")) ?? false;
}
export function newUserRecipe(): Recipe {
  return {
    schemaVersion: 1,
    id: "user-" + crypto.randomUUID(),
    name: "My experiment",
    description: "A custom experimental cipher",
    steps: [{ enabled: true, rule: createRule("word_mirror") }],
  };
}
export function copyRecipe(recipe: Recipe): Recipe {
  return JSON.parse(JSON.stringify(recipe)) as Recipe;
}
