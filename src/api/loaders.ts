import type { Instance } from "./types";

export const LOADER_OPTIONS = [
  { id: "vanilla", label: "Vanilla", hint: "No mods — official Minecraft only" },
  { id: "fabric", label: "Fabric", hint: "Lightweight mod loader" },
  { id: "forge", label: "Forge", hint: "Classic mod ecosystem" },
  { id: "neoforge", label: "NeoForge", hint: "Forge fork for modern versions" },
  { id: "quilt", label: "Quilt", hint: "Fabric-compatible fork" },
] as const;

export type LoaderId = (typeof LOADER_OPTIONS)[number]["id"];

export function loaderLabel(id: string): string {
  return LOADER_OPTIONS.find((l) => l.id === id)?.label ?? id;
}

export function loaderHint(id: string): string {
  return LOADER_OPTIONS.find((l) => l.id === id)?.hint ?? "";
}

export function instanceTargetLabel(inst: Instance): string {
  const loader = loaderLabel(inst.loader);
  const lv = inst.loaderVersion ? ` ${inst.loaderVersion}` : "";
  return `${inst.name} · ${loader}${lv} · MC ${inst.gameVersion}`;
}

export function supportsLoader(loaders: string[] | undefined, loader: string): boolean {
  if (!loaders || loaders.length === 0) return true;
  const want = loader.toLowerCase();
  return loaders.some((l) => l.toLowerCase() === want);
}

export function supportsGameVersion(
  gameVersions: string[] | undefined,
  gameVersion: string,
): boolean {
  if (!gameVersions || gameVersions.length === 0) return true;
  return gameVersions.includes(gameVersion);
}

export function versionMatchesInstance(
  loaders: string[] | undefined,
  gameVersions: string[] | undefined,
  instance: Instance | null | undefined,
): "exact" | "partial" | "mismatch" | "any" {
  if (!instance || instance.loader === "vanilla") return "any";
  const loaderOk = supportsLoader(loaders, instance.loader);
  const gameOk = supportsGameVersion(gameVersions, instance.gameVersion);
  if (loaderOk && gameOk) return "exact";
  if (loaderOk || gameOk) return "partial";
  return "mismatch";
}
