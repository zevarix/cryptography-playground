export type Rule =
  | { type: "phonetic_rotate" }
  | { type: "remove_vowels" }
  | { type: "collapse_doubles" }
  | { type: "word_mirror" }
  | { type: "pair_swap" }
  | { type: "rotate_vowels"; steps: number }
  | { type: "position_shift"; base: number; keyed: boolean };

export type Step = { enabled: boolean; rule: Rule };
export type Recipe = {
  schemaVersion: 1;
  id: string;
  name: string;
  description: string;
  steps: Step[];
};
export type RuleKind = Rule["type"];
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
  return { type } as Rule;
}
export function recipeNeedsKey(recipe: Recipe | null): boolean {
  return recipe?.steps.some(s => s.enabled &&
    (s.rule.type === "phonetic_rotate" || (s.rule.type === "position_shift" && s.rule.keyed))) ?? false;
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
