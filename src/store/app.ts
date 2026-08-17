import { create } from "zustand";
import { api } from "../api/client";
import type { Contrast } from "../theme/tokens";
import type { SupportRank } from "../theme/presets";
import type { DownloadHistoryEntry, Instance, Profile, Route, ThemeMode } from "../api/types";

const DOWNLOAD_HISTORY_CAP = 20;

interface AppState {
  route: Route;
  themeMode: ThemeMode;
  contrast: Contrast;
  themePresetId: string;
  supportRank: SupportRank;
  selectedId: string | null;
  instances: Instance[];
  profiles: Profile[];
  activeProfile: Profile | null;
  search: string;
  addContent: boolean;
  modsIntent: "installed" | "catalog" | null;
  workspaceTab: "mods" | "settings" | "logs" | "packs" | "screenshots";
  installProgress: Record<string, { message: string; progress: number }>;
  downloadHistory: DownloadHistoryEntry[];
  liveLogs: Record<string, string[]>;
  running: Record<string, boolean>;
  /** Soft prompt when Modrinth author session expired (401); catalog stays anonymous. */
  modrinthReconnectPrompt: boolean;
  setRoute: (route: Route) => void;
  setThemeMode: (mode: ThemeMode) => void;
  setContrast: (contrast: Contrast) => void;
  setThemePresetId: (id: string) => void;
  setSupportRank: (rank: SupportRank) => void;
  setSelectedId: (id: string | null) => void;
  setInstances: (instances: Instance[]) => void;
  setProfiles: (profiles: Profile[]) => void;
  setActiveProfile: (profile: Profile | null) => void;
  setSearch: (search: string) => void;
  setAddContent: (addContent: boolean) => void;
  setModsIntent: (intent: "installed" | "catalog" | null) => void;
  setWorkspaceTab: (tab: "mods" | "settings" | "logs" | "packs" | "screenshots") => void;
  setInstallProgress: (id: string, message: string, progress: number) => void;
  appendLog: (id: string, line: string) => void;
  setRunning: (id: string, running: boolean) => void;
  setModrinthReconnectPrompt: (prompt: boolean) => void;
  notifyModrinthAuthError: (message: string) => void;
}

export const useAppStore = create<AppState>((set) => ({
  route: "home",
  themeMode: "system",
  contrast: "normal",
  themePresetId: "aureum",
  supportRank: "free",
  selectedId: null,
  instances: [],
  profiles: [],
  activeProfile: null,
  search: "",
  addContent: false,
  modsIntent: null,
  workspaceTab: "mods",
  installProgress: {},
  downloadHistory: [],
  liveLogs: {},
  running: {},
  modrinthReconnectPrompt: false,
  setRoute: (route) => set({ route }),
  setThemeMode: (themeMode) => set({ themeMode }),
  setContrast: (contrast) => set({ contrast }),
  setThemePresetId: (themePresetId) => set({ themePresetId }),
  setSupportRank: (supportRank) => set({ supportRank }),
  setSelectedId: (selectedId) => {
    set({ selectedId });
    void api.setSetting("selected_instance_id", selectedId ?? "").catch(() => undefined);
  },
  setInstances: (instances) => set({ instances }),
  setProfiles: (profiles) => set({ profiles }),
  setActiveProfile: (activeProfile) => set({ activeProfile }),
  setSearch: (search) => set({ search }),
  setAddContent: (addContent) => set({ addContent }),
  setModsIntent: (modsIntent) => set({ modsIntent }),
  setWorkspaceTab: (workspaceTab) => set({ workspaceTab }),
  setInstallProgress: (id, message, progress) =>
    set((s) => {
      const entry: DownloadHistoryEntry = {
        instanceId: id,
        message,
        progress,
        updatedAt: Date.now(),
      };
      const rest = s.downloadHistory.filter((h) => h.instanceId !== id);
      return {
        installProgress: { ...s.installProgress, [id]: { message, progress } },
        downloadHistory: [entry, ...rest].slice(0, DOWNLOAD_HISTORY_CAP),
      };
    }),
  appendLog: (id, line) =>
    set((s) => {
      const prev = s.liveLogs[id] ?? [];
      return { liveLogs: { ...s.liveLogs, [id]: [...prev.slice(-200), line] } };
    }),
  setRunning: (id, running) =>
    set((s) => ({ running: { ...s.running, [id]: running } })),
  setModrinthReconnectPrompt: (modrinthReconnectPrompt) => set({ modrinthReconnectPrompt }),
  notifyModrinthAuthError: (message) => {
    if (/401|session expired|Reconnect from the Author/i.test(message)) {
      set({ modrinthReconnectPrompt: true });
    }
  },
}));
