export type SupportRank = "free" | "supporter" | "patron";

export interface ThemePreset {
  id: string;
  name: string;
  blurb: string;
  primary: string;
  secondary: string;
  rank: SupportRank;
}

export const RANK_ORDER: SupportRank[] = ["free", "supporter", "patron"];

/** Public key shop on the studio site (EUR). Redeemed keys stay local in the launcher. */
export const KEYS_BUY_URL = "https://azturax.github.io/keys/";

export const RANKS: {
  id: SupportRank;
  label: string;
  price: string;
  blurb: string;
}[] = [
  {
    id: "free",
    label: "Free",
    price: "€0",
    blurb: "Launch, accounts, and catalog. Four house accents.",
  },
  {
    id: "supporter",
    label: "Supporter",
    price: "€2.50",
    blurb: "Unlocks extra accents and a rail badge. Nothing gameplay-gated.",
  },
  {
    id: "patron",
    label: "Patron",
    price: "€7.50",
    blurb: "Every shop theme plus a gold mark. Still cosmetics only.",
  },
];

export const THEME_PRESETS: ThemePreset[] = [
  {
    id: "aureum",
    name: "Aureum",
    blurb: "House orange and gold.",
    primary: "#FFA726",
    secondary: "#FFD54F",
    rank: "free",
  },
  {
    id: "ember",
    name: "Ember",
    blurb: "Deep coral heat.",
    primary: "#FF7043",
    secondary: "#FFAB91",
    rank: "free",
  },
  {
    id: "copper",
    name: "Copper",
    blurb: "Warm metal, quiet chrome.",
    primary: "#C48A4A",
    secondary: "#E6C08A",
    rank: "free",
  },
  {
    id: "slate",
    name: "Slate",
    blurb: "Cool stone, no gold.",
    primary: "#78909C",
    secondary: "#B0BEC5",
    rank: "free",
  },
  {
    id: "night-gold",
    name: "Night Gold",
    blurb: "Low-lit brass.",
    primary: "#C9A227",
    secondary: "#F3E3A1",
    rank: "supporter",
  },
  {
    id: "verdant",
    name: "Verdant",
    blurb: "Moss and pale leaf.",
    primary: "#66BB6A",
    secondary: "#C5E1A5",
    rank: "supporter",
  },
  {
    id: "tide",
    name: "Tide",
    blurb: "Teal waterline.",
    primary: "#26A69A",
    secondary: "#80CBC4",
    rank: "supporter",
  },
  {
    id: "cobalt",
    name: "Cobalt",
    blurb: "Clear studio blue.",
    primary: "#42A5F5",
    secondary: "#90CAF9",
    rank: "supporter",
  },
  {
    id: "amethyst",
    name: "Amethyst",
    blurb: "Violet cut glass.",
    primary: "#AB47BC",
    secondary: "#E1BEE7",
    rank: "patron",
  },
  {
    id: "aurora",
    name: "Aurora",
    blurb: "Cyan into orchid.",
    primary: "#26C6DA",
    secondary: "#CE93D8",
    rank: "patron",
  },
  {
    id: "rose-quartz",
    name: "Rose Quartz",
    blurb: "Soft mineral pink.",
    primary: "#EC407A",
    secondary: "#F8BBD0",
    rank: "patron",
  },
  {
    id: "solar-flare",
    name: "Solar Flare",
    blurb: "Hard amber edge.",
    primary: "#FF6F00",
    secondary: "#FFD54F",
    rank: "patron",
  },
];

export function parseRank(raw: string | undefined | null): SupportRank {
  if (raw === "supporter" || raw === "patron") return raw;
  return "free";
}

export function rankAtLeast(have: SupportRank, need: SupportRank): boolean {
  return RANK_ORDER.indexOf(have) >= RANK_ORDER.indexOf(need);
}

export function presetById(id: string | undefined | null): ThemePreset {
  return THEME_PRESETS.find((p) => p.id === id) ?? THEME_PRESETS[0];
}

export function canUsePreset(preset: ThemePreset, rank: SupportRank): boolean {
  return rankAtLeast(rank, preset.rank);
}

/** Local preview keys until real checkout exists. Not a license for the game. */
export function rankFromKey(raw: string): SupportRank | null {
  const key = raw.trim().toUpperCase().replace(/\s+/g, "");
  if (key === "AUREUM-PATRON") return "patron";
  if (key === "AUREUM-SUPPORT") return "supporter";
  return null;
}
