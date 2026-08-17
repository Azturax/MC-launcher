import {
  argbFromHex,
  hexFromArgb,
  themeFromSourceColor,
  TonalPalette,
  type Scheme,
} from "@material/material-color-utilities";

export const SEED_PRIMARY = "#FFA726";
export const SEED_SECONDARY = "#FFD54F";

export type Contrast = "normal" | "high";
export type SchemeName = "light" | "dark";

function kebab(key: string) {
  return key.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
}

function schemeVars(scheme: Scheme): Record<string, string> {
  const json = scheme.toJSON() as Record<string, number>;
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(json)) {
    out[`--md-sys-color-${kebab(key)}`] = hexFromArgb(value);
  }
  return out;
}

function applySecondary(
  vars: Record<string, string>,
  palette: TonalPalette,
  dark: boolean,
  high: boolean,
) {
  const tone = (t: number) => hexFromArgb(palette.tone(t));
  if (dark) {
    vars["--md-sys-color-secondary"] = tone(high ? 90 : 80);
    vars["--md-sys-color-on-secondary"] = tone(high ? 10 : 20);
    vars["--md-sys-color-secondary-container"] = tone(high ? 40 : 30);
    vars["--md-sys-color-on-secondary-container"] = tone(high ? 98 : 90);
  } else {
    vars["--md-sys-color-secondary"] = tone(high ? 30 : 40);
    vars["--md-sys-color-on-secondary"] = tone(100);
    vars["--md-sys-color-secondary-container"] = tone(high ? 80 : 90);
    vars["--md-sys-color-on-secondary-container"] = tone(10);
  }
}

function applySurfaces(
  vars: Record<string, string>,
  theme: ReturnType<typeof themeFromSourceColor>,
  dark: boolean,
  high: boolean,
) {
  const n = theme.palettes.neutral;
  const nv = theme.palettes.neutralVariant;
  const tone = (p: TonalPalette, t: number) => hexFromArgb(p.tone(t));
  if (dark) {
    vars["--md-sys-color-background"] = tone(n, high ? 0 : 6);
    vars["--md-sys-color-on-background"] = tone(n, high ? 100 : 90);
    vars["--md-sys-color-surface"] = tone(n, high ? 0 : 6);
    vars["--md-sys-color-on-surface"] = tone(n, high ? 100 : 90);
    vars["--md-sys-color-surface-variant"] = tone(nv, high ? 20 : 30);
    vars["--md-sys-color-on-surface-variant"] = tone(nv, high ? 95 : 80);
    vars["--md-sys-color-surface-container-lowest"] = tone(n, 4);
    vars["--md-sys-color-surface-container-low"] = tone(n, 10);
    vars["--md-sys-color-surface-container"] = tone(n, 12);
    vars["--md-sys-color-surface-container-high"] = tone(n, 17);
    vars["--md-sys-color-surface-container-highest"] = tone(n, 22);
    vars["--md-sys-color-outline"] = tone(nv, high ? 100 : 60);
    vars["--md-sys-color-outline-variant"] = tone(nv, high ? 80 : 30);
  } else {
    vars["--md-sys-color-background"] = tone(n, high ? 100 : 98);
    vars["--md-sys-color-on-background"] = tone(n, high ? 0 : 10);
    vars["--md-sys-color-surface"] = tone(n, high ? 100 : 98);
    vars["--md-sys-color-on-surface"] = tone(n, high ? 0 : 10);
    vars["--md-sys-color-surface-variant"] = tone(nv, high ? 90 : 90);
    vars["--md-sys-color-on-surface-variant"] = tone(nv, high ? 10 : 30);
    vars["--md-sys-color-surface-container-lowest"] = tone(n, 100);
    vars["--md-sys-color-surface-container-low"] = tone(n, 96);
    vars["--md-sys-color-surface-container"] = tone(n, 94);
    vars["--md-sys-color-surface-container-high"] = tone(n, 92);
    vars["--md-sys-color-surface-container-highest"] = tone(n, 90);
    vars["--md-sys-color-outline"] = tone(nv, high ? 0 : 50);
    vars["--md-sys-color-outline-variant"] = tone(nv, high ? 20 : 80);
  }
}

export function buildTokens(
  scheme: SchemeName,
  contrast: Contrast,
  primary = SEED_PRIMARY,
  secondary = SEED_SECONDARY,
): Record<string, string> {
  const theme = themeFromSourceColor(argbFromHex(primary));
  const dark = scheme === "dark";
  const high = contrast === "high";
  const vars = schemeVars(dark ? theme.schemes.dark : theme.schemes.light);
  applySecondary(vars, TonalPalette.fromInt(argbFromHex(secondary)), dark, high);
  applySurfaces(vars, theme, dark, high);
  vars["--md-sys-shape-corner-small"] = "8px";
  vars["--md-sys-shape-corner-medium"] = "12px";
  vars["--md-sys-shape-corner-large"] = "16px";
  vars["--md-sys-motion-duration"] = "240ms";
  vars["--md-sys-motion-easing"] = "cubic-bezier(0.2, 0, 0, 1)";
  vars["--md-sys-space"] = "8px";
  return vars;
}

export function applyTokens(
  scheme: SchemeName,
  contrast: Contrast,
  primary = SEED_PRIMARY,
  secondary = SEED_SECONDARY,
  target = document.documentElement,
) {
  const vars = buildTokens(scheme, contrast, primary, secondary);
  for (const [key, value] of Object.entries(vars)) {
    target.style.setProperty(key, value);
  }
  target.dataset.scheme = scheme;
  target.dataset.contrast = contrast;
}

export function resolveScheme(mode: "light" | "dark" | "system"): SchemeName {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode;
}
