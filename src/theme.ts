import type { CSSProperties } from "react";

export type AppearanceMode = "system" | "light" | "dark";
export type FontChoice = "system" | "sans" | "mono";
export type Palette = {
  background: string;
  sidebar: string;
  surface: string;
  panel: string;
  text: string;
  muted: string;
  border: string;
  accent: string;
  subtle: string;
  success: string;
  warning: string;
};
export type Theme = {
  id: string;
  name: string;
  author: string;
  source: string;
  license: string;
  variant: "light" | "dark";
  palette: Palette;
};
export type Appearance = {
  mode: AppearanceMode;
  lightTheme: string;
  darkTheme: string;
  accent: string | null;
  background: string | null;
  foreground: string | null;
  font: FontChoice;
};
export const DEFAULT_APPEARANCE: Appearance = {
  mode: "system",
  lightTheme: "notebook-light",
  darkTheme: "notebook-dark",
  accent: null,
  background: null,
  foreground: null,
  font: "system",
};

const make = (
  id: string, name: string, author: string, source: string, license: string,
  variant: "light" | "dark", palette: Palette,
): Theme => ({ id, name, author, source, license, variant, palette });

export const THEMES: Theme[] = [
  make("notebook-light", "Notebook Paper", "Cryptography Playground", "", "Project-original", "light", {
    background: "#f7f5ef", sidebar: "#eeece4", surface: "#fffef9", panel: "#faf9f2",
    text: "#2e292e", muted: "#756d73", border: "#d8d1c9", accent: "#72539b",
    subtle: "#e8def1", success: "#49765b", warning: "#a0692b",
  }),
  make("notebook-dark", "Notebook Ink", "Cryptography Playground", "", "Project-original", "dark", {
    background: "#19191f", sidebar: "#242329", surface: "#29272f", panel: "#302e36",
    text: "#f0eaf2", muted: "#beb1c0", border: "#504851", accent: "#c5a2e8",
    subtle: "#44374f", success: "#91c7a3", warning: "#e0b36b",
  }),
  make("catppuccin-latte", "Catppuccin Latte", "Catppuccin", "https://github.com/catppuccin/catppuccin", "MIT", "light", {
    background: "#eff1f5", sidebar: "#e6e9ef", surface: "#ffffff", panel: "#f3f4f8",
    text: "#4c4f69", muted: "#6c6f85", border: "#bcc0cc", accent: "#1e66f5",
    subtle: "#dce6ff", success: "#40a02b", warning: "#df8e1d",
  }),
  make("catppuccin-mocha", "Catppuccin Mocha", "Catppuccin", "https://github.com/catppuccin/catppuccin", "MIT", "dark", {
    background: "#1e1e2e", sidebar: "#181825", surface: "#313244", panel: "#242436",
    text: "#cdd6f4", muted: "#a6adc8", border: "#45475a", accent: "#b4befe",
    subtle: "#454461", success: "#a6e3a1", warning: "#f9e2af",
  }),
  make("dracula", "Dracula", "Dracula Theme", "https://github.com/dracula/dracula-theme", "MIT", "dark", {
    background: "#282a36", sidebar: "#21222c", surface: "#383a4a", panel: "#303240",
    text: "#f8f8f2", muted: "#b5b7cf", border: "#6272a4", accent: "#bd93f9",
    subtle: "#4b3c69", success: "#50fa7b", warning: "#ffb86c",
  }),
  make("gruvbox-light", "Gruvbox Light", "morhetz", "https://github.com/morhetz/gruvbox", "MIT", "light", {
    background: "#fbf1c7", sidebar: "#ebdbb2", surface: "#f9f5d7", panel: "#f2e5bc",
    text: "#3c3836", muted: "#665c54", border: "#d5c4a1", accent: "#9d0006",
    subtle: "#e8cf9d", success: "#79740e", warning: "#b57614",
  }),
  make("gruvbox-dark", "Gruvbox Dark", "morhetz", "https://github.com/morhetz/gruvbox", "MIT", "dark", {
    background: "#282828", sidebar: "#1d2021", surface: "#3c3836", panel: "#32302f",
    text: "#ebdbb2", muted: "#bdae93", border: "#504945", accent: "#d79921",
    subtle: "#52442c", success: "#b8bb26", warning: "#fabd2f",
  }),
  make("ayu-light", "Ayu Light", "ayu-theme", "https://github.com/ayu-theme/ayu-colors", "MIT", "light", {
    background: "#fcfcfc", sidebar: "#f0f0f0", surface: "#ffffff", panel: "#f8f8f8",
    text: "#575f66", muted: "#787b80", border: "#dedede", accent: "#ff9940",
    subtle: "#fff0df", success: "#86b300", warning: "#f2ae49",
  }),
  make("ayu-mirage", "Ayu Mirage", "ayu-theme", "https://github.com/ayu-theme/ayu-colors", "MIT", "dark", {
    background: "#1f2430", sidebar: "#171c28", surface: "#2b3242", panel: "#242c3a",
    text: "#cccac2", muted: "#a6a8b0", border: "#3d4555", accent: "#ffcc66",
    subtle: "#51432e", success: "#bae67e", warning: "#ffae57",
  }),
  make("github-light", "GitHub Light", "GitHub Primer", "https://github.com/primer/primitives", "MIT", "light", {
    background: "#ffffff", sidebar: "#f6f8fa", surface: "#ffffff", panel: "#f6f8fa",
    text: "#24292f", muted: "#57606a", border: "#d0d7de", accent: "#0969da",
    subtle: "#ddf4ff", success: "#1a7f37", warning: "#9a6700",
  }),
  make("github-dark", "GitHub Dark", "GitHub Primer", "https://github.com/primer/primitives", "MIT", "dark", {
    background: "#0d1117", sidebar: "#161b22", surface: "#161b22", panel: "#21262d",
    text: "#c9d1d9", muted: "#8b949e", border: "#30363d", accent: "#58a6ff",
    subtle: "#17365d", success: "#3fb950", warning: "#d29922",
  }),
  make("everforest-light", "Everforest Light", "sainnhe", "https://github.com/sainnhe/everforest", "MIT", "light", {
    background: "#fdf6e3", sidebar: "#f4f0d9", surface: "#fff9e9", panel: "#f6f0dd",
    text: "#5c6a72", muted: "#829181", border: "#d4cbb4", accent: "#8da101",
    subtle: "#e5eac4", success: "#8da101", warning: "#dfa000",
  }),
  make("everforest-dark", "Everforest Dark", "sainnhe", "https://github.com/sainnhe/everforest", "MIT", "dark", {
    background: "#2d353b", sidebar: "#232a2e", surface: "#343f44", panel: "#303b40",
    text: "#d3c6aa", muted: "#9da9a0", border: "#4f585e", accent: "#a7c080",
    subtle: "#3b503e", success: "#a7c080", warning: "#dbbc7f",
  }),
];

export const COLOR_ACCENTS = ["#72539b", "#1e66f5", "#a7c080", "#d79921", "#d75f69", "#09a0a4"];

export function getTheme(id: string, mode: "light" | "dark"): Theme {
  return THEMES.find((theme) => theme.id === id && theme.variant === mode)
    ?? THEMES.find((theme) => theme.id === (mode === "light" ? "notebook-light" : "notebook-dark"))!;
}
export function paletteVars(
  theme: Theme, settings: Appearance,
): CSSProperties {
  const palette = theme.palette;
  const accent = settings.accent ?? palette.accent;
  const textOnAccent = isLight(accent) ? "#202127" : "#ffffff";
  return {
    "--app-bg": settings.background ?? palette.background,
    "--app-sidebar": palette.sidebar,
    "--app-surface": palette.surface,
    "--app-panel": palette.panel,
    "--app-text": settings.foreground ?? palette.text,
    "--app-muted": palette.muted,
    "--app-border": palette.border,
    "--app-accent": accent,
    "--app-accent-ink": textOnAccent,
    "--app-subtle": palette.subtle,
    "--app-success": palette.success,
    "--app-warning": palette.warning,
    "--app-font": settings.font === "mono"
      ? '"Cascadia Code", "SFMono-Regular", Consolas, monospace'
      : settings.font === "sans"
        ? '"Inter", "Segoe UI", system-ui, sans-serif'
        : 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  } as CSSProperties;
}
export function isLight(hex: string): boolean {
  const c = hex.replace("#", "");
  const rgb = [0, 2, 4].map(i => parseInt(c.slice(i, i + 2), 16));
  return ((rgb[0] * 299 + rgb[1] * 587 + rgb[2] * 114) / 1000) > 155;
}
export function readAppearance(): Appearance {
  try {
    const raw = localStorage.getItem("crypto-playground-appearance-v1");
    if (!raw) return { ...DEFAULT_APPEARANCE };
    const state = JSON.parse(raw) as Partial<Appearance>;
    const mode = ["system", "light", "dark"].includes(state.mode ?? "")
      ? state.mode! : DEFAULT_APPEARANCE.mode;
    const lightTheme = getTheme(state.lightTheme ?? "", "light").id;
    const darkTheme = getTheme(state.darkTheme ?? "", "dark").id;
    const validColor = (value: unknown) => typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value) ? value : null;
    return {
      mode, lightTheme, darkTheme,
      accent: validColor(state.accent),
      background: validColor(state.background),
      foreground: validColor(state.foreground),
      font: (["system", "sans", "mono"] as const).includes(state.font as FontChoice)
        ? state.font! : "system",
    };
  } catch {
    return { ...DEFAULT_APPEARANCE };
  }
}
