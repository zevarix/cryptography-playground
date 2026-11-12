import type { Recipe, RecipeResult, WordChoice } from "./recipeTypes";
import type { Direction, DocumentInfo, JobInfo, ReadingInfo, DocChoice } from "./documentTypes";

export type ExperimentSummary = { id: string; name: string; createdAt: string; kind: "text" | "file"; recipeName: string };
export type ExperimentList = { entries: ExperimentSummary[]; problems: string[] };
export type OpenedTextExperiment = { kind: "text"; summary: ExperimentSummary; recipe: Recipe; direction: Direction; source: string; result: RecipeResult; choices: WordChoice[]; runId: string };
export type OpenedFileExperiment = { kind: "file"; summary: ExperimentSummary; document: DocumentInfo; job: JobInfo; reading: ReadingInfo | null; choices: DocChoice[] };
export type OpenedExperiment = OpenedTextExperiment | OpenedFileExperiment;
export type ExperimentTarget = { kind: "text"; runId: string } | { kind: "file"; jobId: string; readingId: string | null };
