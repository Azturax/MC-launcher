import { Download, Home, Info, Palette, PenLine, Puzzle, Settings as SettingsIcon, Users } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "./api/client";
import type { AppInfo, AuthStatus, InstallProgress, LaunchLog, Route } from "./api/types";
import { FirstRun } from "./components/FirstRun";
import { Button, Callout } from "./components/ui";
import { About } from "./screens/About";
import { Accounts } from "./screens/Accounts";
import { Author } from "./screens/Author";
import { Downloads } from "./screens/Downloads";
import { Home as HomeScreen } from "./screens/Home";
import { Mods } from "./screens/Mods";
import { Settings } from "./screens/Settings";
import { Shop } from "./screens/Shop";
import { useAppStore } from "./store/app";
import { canUsePreset, parseRank, presetById } from "./theme/presets";
import { applyTokens, resolveScheme } from "./theme/tokens";

const NAV: { id: Route; label: string; icon: typeof Home; disabled?: boolean }[] = [
  { id: "home", label: "Home", icon: Home },
  { id: "mods", label: "Mods", icon: Puzzle },
  { id: "author", label: "Author", icon: PenLine },
  { id: "shop", label: "Shop", icon: Palette },
  { id: "accounts", label: "Accounts", icon: Users },
  { id: "downloads", label: "Downloads", icon: Download },
  { id: "settings", label: "Settings", icon: SettingsIcon },
  { id: "about", label: "About", icon: Info },
];

export default function App() {
  const {
    route,
    setRoute,
    themeMode,
    contrast,
    themePresetId,
    supportRank,
    activeProfile,
    setInstances,
    setProfiles,
    setActiveProfile,
    setThemeMode,
    setContrast,
    setThemePresetId,
    setSupportRank,
    setAddContent,
    setInstallProgress,
    appendLog,
    setRunning,
    modrinthReconnectPrompt,
    setModrinthReconnectPrompt,
  } = useAppStore();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [auth, setAuth] = useState<AuthStatus | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const preset = presetById(themePresetId);
    const scheme = resolveScheme(themeMode);
    applyTokens(scheme, contrast, preset.primary, preset.secondary);
  }, [themeMode, contrast, themePresetId]);

  useEffect(() => {
    if (themeMode !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      const preset = presetById(themePresetId);
      applyTokens(resolveScheme("system"), contrast, preset.primary, preset.secondary);
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [themeMode, contrast, themePresetId]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const [appInfo, settings, instances, profiles, active, status] = await Promise.all([
        api.getAppInfo(),
        api.getSettings(),
        api.listInstances(),
        api.listProfiles(),
        api.getActiveProfile(),
        api.authStatus(),
      ]);
      if (cancelled) return;
      setInfo(appInfo);
      setAuth(status);
      setInstances(instances);
      setProfiles(profiles);
      setActiveProfile(active);
      if (settings.theme === "light" || settings.theme === "dark" || settings.theme === "system") {
        setThemeMode(settings.theme);
      }
      if (settings.contrast === "high" || settings.contrast === "normal") {
        setContrast(settings.contrast);
      }
      const rank = parseRank(settings.support_rank);
      setSupportRank(rank);
      const preset = presetById(settings.theme_preset);
      setThemePresetId(canUsePreset(preset, rank) ? preset.id : "aureum");
      setAddContent(settings.add_content === "1" || settings.add_content === "true");
      const savedId = settings.selected_instance_id?.trim();
      if (savedId && instances.some((i) => i.id === savedId)) {
        // Avoid double-write on boot: set state only.
        useAppStore.setState({ selectedId: savedId });
      }
      void api
        .authorStatus()
        .then((s) => {
          if (s.expired) setModrinthReconnectPrompt(true);
        })
        .catch(() => undefined);
      setReady(true);
    })().catch((e: Error) => {
      console.error(e);
      setReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, [setActiveProfile, setAddContent, setContrast, setInstances, setModrinthReconnectPrompt, setProfiles, setSupportRank, setThemeMode, setThemePresetId]);

  useEffect(() => {
    if (!api.isTauri()) return;
    let unlistenProgress: (() => void) | undefined;
    let unlistenLog: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    let unlistenStarted: (() => void) | undefined;
    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      // Rehydrate running map after hot reload / window open.
      try {
        const ids = await api.listRunningInstances();
        for (const id of ids) setRunning(id, true);
      } catch {
        /* ignore */
      }
      unlistenProgress = await listen<InstallProgress>("install-progress", (e) => {
        setInstallProgress(e.payload.instanceId, e.payload.message, e.payload.progress);
      });
      unlistenStarted = await listen<{ instanceId: string; pid: number }>("launch-started", (e) => {
        setRunning(e.payload.instanceId, true);
      });
      unlistenLog = await listen<LaunchLog>("launch-log", (e) => {
        appendLog(e.payload.instanceId, e.payload.line);
        setRunning(e.payload.instanceId, true);
      });
      unlistenExit = await listen<{ instanceId: string }>("launch-exited", (e) => {
        setRunning(e.payload.instanceId, false);
        appendLog(e.payload.instanceId, "— process exited —");
      });
    })();
    return () => {
      unlistenProgress?.();
      unlistenLog?.();
      unlistenExit?.();
      unlistenStarted?.();
    };
  }, [appendLog, setInstallProgress, setRunning]);

  const page = useMemo(() => {
    switch (route) {
      case "mods":
        return <Mods />;
      case "author":
        return <Author />;
      case "accounts":
        return <Accounts auth={auth} />;
      case "shop":
        return <Shop />;
      case "downloads":
        return <Downloads />;
      case "settings":
        return <Settings />;
      case "about":
        return <About info={info} />;
      default:
        return <HomeScreen />;
    }
  }, [route, auth, info]);

  return (
    <div className="app-shell">
      <nav className="nav-rail" aria-label="Primary">
        <div className={`nav-logo rank-${supportRank} with-tooltip`} data-tooltip="Aureum" data-tooltip-side="right" aria-label="Aureum">
          A
        </div>
        {NAV.map((item) => {
          const Icon = item.icon;
          const label = item.disabled ? "Coming in v1" : item.label;
          return (
            <button
              key={item.id}
              type="button"
              className={`nav-item with-tooltip ${route === item.id ? "active" : ""}`}
              disabled={item.disabled}
              data-tooltip={label}
              data-tooltip-side="right"
              aria-label={label}
              onClick={() => setRoute(item.id)}
            >
              <Icon size={22} aria-hidden />
            </button>
          );
        })}
        <div className="nav-grow" />
        <button
          type="button"
          className="account-chip with-tooltip"
          data-tooltip={activeProfile?.displayName ?? "Sign in"}
          data-tooltip-side="right"
          aria-label={activeProfile?.displayName ?? "Sign in"}
          onClick={() => setRoute("accounts")}
        >
          <span className="avatar">
            {(activeProfile?.displayName ?? "A").slice(0, 1).toUpperCase()}
          </span>
        </button>
      </nav>
      <div className="main">
        {ready && modrinthReconnectPrompt && route !== "author" ? (
          <div style={{ padding: "12px 20px 0" }}>
            <Callout tone="warn">
              Modrinth author session expired. Catalog browse still works.{" "}
              <Button
                small
                onClick={() => {
                  setModrinthReconnectPrompt(false);
                  setRoute("author");
                }}
              >
                Reconnect
              </Button>
              <Button
                small
                variant="text"
                onClick={() => setModrinthReconnectPrompt(false)}
              >
                Dismiss
              </Button>
            </Callout>
          </div>
        ) : null}
        {ready ? page : null}
      </div>
      <nav className="nav-bar" aria-label="Primary mobile">
        {NAV.map((item) => {
          const Icon = item.icon;
          const label = item.disabled ? "Coming in v1" : item.label;
          return (
            <button
              key={item.id}
              type="button"
              className={`nav-item with-tooltip ${route === item.id ? "active" : ""}`}
              disabled={item.disabled}
              data-tooltip={label}
              data-tooltip-side="top"
              aria-label={label}
              onClick={() => setRoute(item.id)}
            >
              <Icon size={20} aria-hidden />
            </button>
          );
        })}
      </nav>
      {ready && info && !info.disclaimerAccepted ? (
        <FirstRun
          disclaimer={info.disclaimer}
          onAccept={() => {
            void api.acceptDisclaimer().then(() =>
              setInfo((prev) => (prev ? { ...prev, disclaimerAccepted: true } : prev)),
            );
          }}
        />
      ) : null}
    </div>
  );
}
