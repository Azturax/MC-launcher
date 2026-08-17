import {
  ArrowDown,
  ArrowUp,
  Copy,
  Download,
  FileArchive,
  FolderOpen,
  Image as ImageIcon,
  Pin,
  Play,
  Plus,
  Puzzle,
  RefreshCw,
  Square,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../api/client";
import type {
  GameVersion,
  InstalledContent,
  InstalledMod,
  Instance,
  InstanceStatus,
  JavaInstall,
  LoaderVersion,
  LogFileEntry,
  MediaFile,
  MemoryInfo,
  VersionChannel,
} from "../api/types";
import { filterGameVersions, versionLabel } from "../api/versions";
import { loaderLabel } from "../api/loaders";
import { useAppStore } from "../store/app";
import { Button, Callout, IconButton, SelectField, Switch, TextField } from "./ui";

function modName(m: InstalledMod) {
  return m.displayName || m.filename.replace(/\.jar$/i, "");
}

function sourceLabel(source: string) {
  if (source === "modrinth") return "Modrinth";
  if (source === "curseforge") return "CurseForge";
  if (source === "local") return "Local";
  if (source.startsWith("mrpack")) return "Pack";
  return source;
}

function contentTypeLabel(t: string) {
  if (t === "resourcepack") return "Resource pack";
  if (t === "shader") return "Shader";
  if (t === "datapack") return "Datapack";
  return t;
}

function isLocalMod(m: InstalledMod) {
  return m.source === "local" || m.projectId.startsWith("local:");
}

type Tab = "mods" | "settings" | "logs" | "packs" | "screenshots";

function formatBytes(n: number) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function InstanceWorkspace({
  instance,
  status,
  busy,
  onPlay,
  onStop,
  onInstall,
  onRefresh,
  onDeleted,
}: {
  instance: Instance;
  status: InstanceStatus | null;
  busy: boolean;
  onPlay: () => void;
  onStop: () => void;
  onInstall: () => void;
  onRefresh: () => void;
  onDeleted: () => void;
}) {
  const {
    workspaceTab,
    setWorkspaceTab,
    setModsIntent,
    setRoute,
    setSelectedId,
    addContent,
    installProgress,
    running,
    liveLogs,
  } = useAppStore();

  const [mods, setMods] = useState<InstalledMod[]>([]);
  const [modsError, setModsError] = useState<string | null>(null);
  const [content, setContent] = useState<InstalledContent[]>([]);
  const [contentError, setContentError] = useState<string | null>(null);
  const [packMsg, setPackMsg] = useState<string | null>(null);
  const [media, setMedia] = useState<MediaFile[]>([]);
  const [mediaError, setMediaError] = useState<string | null>(null);
  const [selectedShot, setSelectedShot] = useState<string | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [mediaBusy, setMediaBusy] = useState(false);
  const [logFolder, setLogFolder] = useState<"logs" | "crash-reports">("logs");
  const [logFiles, setLogFiles] = useState<LogFileEntry[]>([]);
  const [selectedLog, setSelectedLog] = useState<string | null>(null);
  const [logPreview, setLogPreview] = useState<string | null>(null);
  const [logTruncated, setLogTruncated] = useState(false);
  const [logFilesError, setLogFilesError] = useState<string | null>(null);
  const [fileLogs, setFileLogs] = useState<string[]>([]);
  const [javas, setJavas] = useState<JavaInstall[]>([]);
  const [memory, setMemory] = useState<MemoryInfo | null>(null);

  const [name, setName] = useState(instance.name);
  const [memoryMb, setMemoryMb] = useState(instance.memoryMb);
  const [jvmArgs, setJvmArgs] = useState(instance.jvmArgs ?? "");
  const [javaPath, setJavaPath] = useState(instance.javaPath ?? "");
  const [keepOpen, setKeepOpen] = useState(instance.keepOpen);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [settingsSaved, setSettingsSaved] = useState(false);

  const [gameVersions, setGameVersions] = useState<GameVersion[]>([]);
  const [versionChannel, setVersionChannel] = useState<VersionChannel>("release");
  const [upgradeGame, setUpgradeGame] = useState(instance.gameVersion);
  const [upgradeLoader, setUpgradeLoader] = useState(instance.loader);
  const [upgradeLoaderVer, setUpgradeLoaderVer] = useState(instance.loaderVersion ?? "");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>([]);
  const [upgradeBusy, setUpgradeBusy] = useState(false);
  const [upgradeMsg, setUpgradeMsg] = useState<string | null>(null);

  const progress = installProgress[instance.id];
  const isRunning = !!running[instance.id];
  const installLocked = busy || upgradeBusy || !!progress || isRunning;
  const logs = [...fileLogs, ...(liveLogs[instance.id] ?? [])].slice(-200);
  const tab: Tab = workspaceTab;

  useEffect(() => {
    setName(instance.name);
    setMemoryMb(instance.memoryMb);
    setJvmArgs(instance.jvmArgs ?? "");
    setJavaPath(instance.javaPath ?? "");
    setKeepOpen(instance.keepOpen);
    setUpgradeGame(instance.gameVersion);
    setUpgradeLoader(instance.loader);
    setUpgradeLoaderVer(instance.loaderVersion ?? "");
    setSettingsSaved(false);
    setUpgradeMsg(null);
  }, [instance]);

  useEffect(() => {
    void api.listGameVersions().then(setGameVersions).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (upgradeLoader === "vanilla") {
      setLoaderVersions([]);
      setUpgradeLoaderVer("");
      return;
    }
    void api
      .listLoaderVersions(upgradeLoader, upgradeGame)
      .then((list) => {
        setLoaderVersions(list);
        if (list.length && !list.some((v) => v.version === upgradeLoaderVer)) {
          const preferred = list.find((v) => v.stable) ?? list[0];
          setUpgradeLoaderVer(preferred.version);
        }
      })
      .catch(() => setLoaderVersions([]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [upgradeLoader, upgradeGame]);

  const visibleGameVersions = filterGameVersions(gameVersions, versionChannel);

  async function refreshMods() {
    setMods(await api.listInstanceMods(instance.id));
  }

  async function refreshContent() {
    setContent(await api.listInstanceContent(instance.id));
  }

  async function checkContentUpdates() {
    setContentError(null);
    try {
      setContent(await api.checkContentUpdates(instance.id));
    } catch (e) {
      setContentError(e instanceof Error ? e.message : String(e));
      await refreshContent();
    }
  }

  async function applyContentUpdate(contentId: string) {
    setContentError(null);
    try {
      await api.applyContentUpdate(instance.id, contentId);
      setContent(await api.checkContentUpdates(instance.id));
    } catch (e) {
      setContentError(e instanceof Error ? e.message : String(e));
      await refreshContent();
    }
  }

  async function refreshMedia() {
    const list = await api.listInstanceMedia(instance.id);
    setMedia(list);
    if (selectedShot && !list.some((m) => m.name === selectedShot)) {
      setSelectedShot(null);
      setPreviewUrl(null);
    }
  }

  async function selectShot(name: string) {
    setSelectedShot(name);
    setMediaError(null);
    try {
      const preview = await api.readMediaPreview(instance.id, name);
      setPreviewUrl(preview.dataUrl);
    } catch (e) {
      setPreviewUrl(null);
      setMediaError(e instanceof Error ? e.message : String(e));
    }
  }

  async function importShots(paths: string[]) {
    if (!paths.length) return;
    setMediaBusy(true);
    setMediaError(null);
    try {
      await api.importMediaFiles(instance.id, paths);
      await refreshMedia();
    } catch (e) {
      setMediaError(e instanceof Error ? e.message : String(e));
    } finally {
      setMediaBusy(false);
    }
  }

  useEffect(() => {
    if (tab !== "screenshots") return;
    setMediaError(null);
    setThumbs({});
    void refreshMedia().catch((e) => {
      setMediaError(e instanceof Error ? e.message : String(e));
      setMedia([]);
    });
  }, [tab, instance.id]);

  useEffect(() => {
    if (tab !== "screenshots" || media.length === 0) return;
    let cancelled = false;
    void (async () => {
      for (const m of media) {
        if (cancelled) return;
        if (thumbs[m.name]) continue;
        try {
          const thumb = await api.readMediaThumb(instance.id, m.name);
          if (cancelled) return;
          setThumbs((prev) => (prev[m.name] ? prev : { ...prev, [m.name]: thumb.dataUrl }));
        } catch {
          /* skip broken thumbs */
        }
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, instance.id, media]);

  async function refreshLogFiles() {
    const list = await api.listLogFiles(instance.id, logFolder);
    setLogFiles(list);
    if (selectedLog && !list.some((f) => f.name === selectedLog)) {
      setSelectedLog(null);
      setLogPreview(null);
      setLogTruncated(false);
    }
  }

  async function selectLogFile(name: string, previewable: boolean) {
    setSelectedLog(name);
    setLogFilesError(null);
    if (!previewable) {
      setLogPreview(null);
      setLogTruncated(false);
      setLogFilesError("Compressed or non-text file — use Reveal folder to open it.");
      return;
    }
    try {
      const preview = await api.readLogFile(instance.id, logFolder, name);
      setLogPreview(preview.text);
      setLogTruncated(preview.truncated);
    } catch (e) {
      setLogPreview(null);
      setLogFilesError(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    if (tab !== "logs") return;
    setLogFilesError(null);
    setSelectedLog(null);
    setLogPreview(null);
    void refreshLogFiles().catch((e) => {
      setLogFilesError(e instanceof Error ? e.message : String(e));
      setLogFiles([]);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, instance.id, logFolder]);

  useEffect(() => {
    if (tab !== "screenshots") return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      try {
        if (!("__TAURI_INTERNALS__" in window)) return;
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        unlisten = await getCurrentWebview().onDragDropEvent((event) => {
          if (cancelled) return;
          if (event.payload.type !== "drop") return;
          const paths = event.payload.paths.filter((p) =>
            /\.(png|jpe?g|webp|gif)$/i.test(p),
          );
          void importShots(paths);
        });
      } catch {
        /* browser preview */
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [tab, instance.id]);

  useEffect(() => {
    setModsError(null);
    void refreshMods().catch((e) => {
      setModsError(e instanceof Error ? e.message : String(e));
      setMods([]);
    });
    setContentError(null);
    void refreshContent().catch((e) => {
      setContentError(e instanceof Error ? e.message : String(e));
      setContent([]);
    });
  }, [instance.id]);

  useEffect(() => {
    if (tab !== "packs") return;
    setContentError(null);
    void refreshContent().catch((e) => {
      setContentError(e instanceof Error ? e.message : String(e));
      setContent([]);
    });
  }, [tab, instance.id]);

  useEffect(() => {
    void api.getLogTail(instance.id).then(setFileLogs).catch(() => setFileLogs([]));
  }, [instance.id, installProgress[instance.id]?.progress]);

  useEffect(() => {
    if (tab !== "settings") return;
    void api.discoverJava().then(setJavas).catch(() => undefined);
    void api.getSystemMemory().then(setMemory).catch(() => undefined);
  }, [tab]);

  function openCatalog() {
    setSelectedId(instance.id);
    setModsIntent("catalog");
    setRoute("mods");
  }

  async function moveMod(index: number, dir: -1 | 1) {
    const next = index + dir;
    if (next < 0 || next >= mods.length) return;
    const order = mods.map((m) => m.projectId);
    const tmp = order[index];
    order[index] = order[next];
    order[next] = tmp;
    try {
      setMods(await api.reorderMods(instance.id, order));
    } catch (e) {
      setModsError(e instanceof Error ? e.message : String(e));
    }
  }

  async function checkUpdates() {
    try {
      setMods(await api.checkModUpdates(instance.id));
    } catch (e) {
      setModsError(e instanceof Error ? e.message : String(e));
    }
  }

  async function applyUpdate(m: InstalledMod) {
    if (!m.updateVersionId) return;
    try {
      await api.installMod({
        instanceId: instance.id,
        projectId: m.projectId,
        versionId: m.updateVersionId,
      });
      await refreshMods();
    } catch (e) {
      setModsError(e instanceof Error ? e.message : String(e));
    }
  }

  async function saveSettings() {
    const trimmed = name.trim();
    if (!trimmed) {
      setSettingsError("Name is required.");
      return;
    }
    setSettingsBusy(true);
    setSettingsError(null);
    try {
      await api.updateInstance(instance.id, {
        name: trimmed,
        memoryMb,
        jvmArgs: jvmArgs.trim() ? jvmArgs.trim() : null,
        javaPath: javaPath.trim() ? javaPath.trim() : null,
        keepOpen,
      });
      setSettingsSaved(true);
      onRefresh();
    } catch (e) {
      setSettingsError(e instanceof Error ? e.message : String(e));
    } finally {
      setSettingsBusy(false);
    }
  }

  async function applyVersionUpgrade() {
    if (installLocked) return;
    const changed =
      upgradeGame !== instance.gameVersion ||
      upgradeLoader !== instance.loader ||
      (upgradeLoader !== "vanilla" &&
        (upgradeLoaderVer || "") !== (instance.loaderVersion || ""));
    if (!changed) {
      setUpgradeMsg("Already on this Minecraft / loader version.");
      return;
    }
    const warn =
      `Upgrade ${instance.name} to ${loaderLabel(upgradeLoader)}` +
      (upgradeLoader !== "vanilla" && upgradeLoaderVer ? ` ${upgradeLoaderVer}` : "") +
      ` · MC ${upgradeGame}?\n\n` +
      "This refreshes version metadata and libraries. Your mods/ folder is kept, but mods may break on a new Minecraft or loader version. Forge/NeoForge may need processors to finish — watch install progress.";
    if (!confirm(warn)) return;
    setUpgradeBusy(true);
    setUpgradeMsg(null);
    setSettingsError(null);
    try {
      await api.upgradeInstanceVersion(
        instance.id,
        upgradeGame,
        upgradeLoader,
        upgradeLoader === "vanilla" ? null : upgradeLoaderVer || null,
      );
      setUpgradeMsg(
        "Version updated and install finished. Mods were not wiped — re-check compatibility.",
      );
      onRefresh();
    } catch (e) {
      setSettingsError(e instanceof Error ? e.message : String(e));
    } finally {
      setUpgradeBusy(false);
    }
  }

  async function importPack(intoExisting: boolean) {
    setPackMsg(null);
    try {
      const path = await api.pickMrpackFile();
      if (!path) return;
      const result = await api.importMrpack({
        path,
        instanceId: intoExisting ? instance.id : null,
        name: null,
      });
      setPackMsg(
        `Imported ${result.filesInstalled} file${result.filesInstalled === 1 ? "" : "s"} into ${result.instance.name}.`,
      );
      setSelectedId(result.instance.id);
      onRefresh();
      await refreshMods();
      setWorkspaceTab("mods");
    } catch (e) {
      setPackMsg(e instanceof Error ? e.message : String(e));
    }
  }

  async function exportPack() {
    setPackMsg(null);
    try {
      const path = await api.pickMrpackSave(`${instance.name}.mrpack`);
      if (!path) return;
      const written = await api.exportMrpack({
        instanceId: instance.id,
        path,
        name: instance.name,
      });
      setPackMsg(`Exported pack to ${written}`);
    } catch (e) {
      setPackMsg(e instanceof Error ? e.message : String(e));
    }
  }

  const maxMb = Math.max(memory?.totalMb ? memory.totalMb - 1024 : 16384, 1024);

  return (
    <aside className="instance-workspace" aria-label={`${instance.name} workspace`}>
      <div className="workspace-header">
        <div>
          <h2>{instance.name}</h2>
          <div className="row">
            <span className="pill">{instance.loader}</span>
            <span className="muted">{instance.gameVersion}</span>
            <span className="muted">{status?.installed ? "Ready" : "Needs install"}</span>
          </div>
        </div>
        <div className="row">
          {isRunning ? (
            <Button variant="danger" disabled={busy} onClick={onStop}>
              <Square size={16} aria-hidden /> Stop
            </Button>
          ) : (
            <Button disabled={busy} onClick={onPlay}>
              <Play size={16} aria-hidden /> Play
            </Button>
          )}
          <IconButton label="Install" variant="tonal" disabled={busy || isRunning} onClick={onInstall}>
            <Download size={18} aria-hidden />
          </IconButton>
          <IconButton
            label="Open instance folder"
            variant="outline"
            onClick={() => void api.openInstanceFolder(instance.id)}
          >
            <FolderOpen size={18} aria-hidden />
          </IconButton>
        </div>
      </div>

      {progress ? (
        <div className="stack">
          <span className="muted">{progress.message}</span>
          <div className="progress">
            <span style={{ width: `${Math.round(progress.progress * 100)}%` }} />
          </div>
        </div>
      ) : null}

      <div className="seg" role="tablist" aria-label="Instance sections">
        {(
          [
            ["mods", "Mods"],
            ["packs", "Packs"],
            ["screenshots", "Screenshots"],
            ["logs", "Logs"],
            ["settings", "Settings"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className={tab === id ? "active" : ""}
            onClick={() => setWorkspaceTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "mods" ? (
        <div className="workspace-body stack">
          {instance.loader === "vanilla" ? (
            <Callout tone="info">
              Vanilla instances cannot load mods. Import a Fabric/Quilt/Forge pack or create a
              modded instance.
            </Callout>
          ) : null}
          {modsError ? <Callout>{modsError}</Callout> : null}
          <div className="row">
            <Button small onClick={openCatalog} disabled={instance.loader === "vanilla"}>
              <Plus size={14} aria-hidden /> Add mods
            </Button>
            <Button small variant="outline" onClick={() => void checkUpdates()}>
              <RefreshCw size={14} aria-hidden /> Check updates
            </Button>
            <IconButton
              label="Open mods folder"
              variant="outline"
              small
              onClick={() => void api.openModsFolder(instance.id)}
            >
              <FolderOpen size={16} aria-hidden />
            </IconButton>
            <IconButton
              label="Export .mrpack"
              variant="outline"
              small
              onClick={() => void exportPack()}
            >
              <FileArchive size={16} aria-hidden />
            </IconButton>
          </div>
          <p className="muted" style={{ margin: 0 }}>
            Load order is persisted for Aureum. Fabric and Quilt still control class loading.
          </p>
          {mods.length === 0 ? (
            <div className="empty-mods">
              <h3>No mods yet</h3>
              <p className="muted">Add from the catalog, import a .mrpack, or drop jars into mods/.</p>
              <Button onClick={openCatalog} disabled={instance.loader === "vanilla"}>
                <Puzzle size={16} aria-hidden /> Add mods
              </Button>
            </div>
          ) : (
            <div className="mod-table-wrap">
              <table className="mod-table">
                <thead>
                  <tr>
                    <th className="mod-col-on">On</th>
                    <th className="mod-col-on">Pin</th>
                    <th>Name</th>
                    <th>Version</th>
                    <th>Source</th>
                    <th className="mod-col-actions">Order</th>
                    <th className="mod-col-actions">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {mods.map((m, idx) => (
                    <tr key={m.id} className={m.enabled ? "" : "disabled-mod"}>
                      <td>
                        <input
                          type="checkbox"
                          checked={m.enabled}
                          aria-label={`Enable ${modName(m)}`}
                          onChange={() =>
                            void api
                              .setModEnabled(instance.id, m.projectId, !m.enabled)
                              .then(refreshMods)
                              .catch((e) =>
                                setModsError(e instanceof Error ? e.message : String(e)),
                              )
                          }
                        />
                      </td>
                      <td>
                        <IconButton
                          label={m.pinned ? `Unpin ${modName(m)}` : `Pin ${modName(m)}`}
                          variant={m.pinned ? "tonal" : "outline"}
                          small
                          disabled={isLocalMod(m) || m.projectId.startsWith("mrpack:")}
                          onClick={() =>
                            void api
                              .setModPin(instance.id, m.projectId, !m.pinned)
                              .then(refreshMods)
                              .catch((e) =>
                                setModsError(e instanceof Error ? e.message : String(e)),
                              )
                          }
                        >
                          <Pin size={14} aria-hidden />
                        </IconButton>
                      </td>
                      <td>
                        <div className="mod-name">{modName(m)}</div>
                        <div className="muted mod-file">{m.filename}</div>
                      </td>
                      <td>{m.versionNumber || "—"}</td>
                      <td>
                        <span className="pill">{sourceLabel(m.source)}</span>
                      </td>
                      <td className="mod-actions">
                        <IconButton
                          label="Move up"
                          variant="outline"
                          small
                          disabled={idx === 0}
                          onClick={() => void moveMod(idx, -1)}
                        >
                          <ArrowUp size={14} aria-hidden />
                        </IconButton>
                        <IconButton
                          label="Move down"
                          variant="outline"
                          small
                          disabled={idx === mods.length - 1}
                          onClick={() => void moveMod(idx, 1)}
                        >
                          <ArrowDown size={14} aria-hidden />
                        </IconButton>
                      </td>
                      <td className="mod-actions">
                        {m.updateVersionId ? (
                          <Button small variant="tonal" onClick={() => void applyUpdate(m)}>
                            Update
                          </Button>
                        ) : null}
                        <IconButton
                          label={`Remove ${modName(m)}`}
                          variant="danger"
                          small
                          onClick={() =>
                            void api
                              .removeMod(instance.id, m.projectId)
                              .then(refreshMods)
                              .catch((e) =>
                                setModsError(e instanceof Error ? e.message : String(e)),
                              )
                          }
                        >
                          <Trash2 size={14} aria-hidden />
                        </IconButton>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {packMsg && tab === "mods" ? <Callout tone="info">{packMsg}</Callout> : null}
        </div>
      ) : null}

      {tab === "settings" ? (
        <div className="workspace-body stack">
          <p className="muted" style={{ margin: 0 }}>
            {loaderLabel(instance.loader)}
            {instance.loaderVersion ? ` ${instance.loaderVersion}` : ""} · MC{" "}
            {instance.gameVersion}
            {memory ? ` · ${memory.totalMb} MB system memory` : ""}
          </p>
          <TextField label="Name" value={name} onChange={(e) => setName(e.target.value)} />
          <label className="field">
            <span>Memory ({memoryMb} MB)</span>
            <input
              type="range"
              min={512}
              max={maxMb}
              step={256}
              value={Math.min(memoryMb, maxMb)}
              onChange={(e) => setMemoryMb(Number(e.target.value))}
            />
          </label>
          <TextField
            label="JVM arguments"
            value={jvmArgs}
            placeholder="Optional extras for this instance"
            onChange={(e) => setJvmArgs(e.target.value)}
          />
          <SelectField
            label="Java runtime"
            value={javaPath}
            onChange={(e) => setJavaPath(e.target.value)}
          >
            <option value="">Use global / PATH</option>
            {javas.map((j) => (
              <option key={j.path} value={j.path}>
                {j.version} — {j.path}
              </option>
            ))}
            {javaPath && !javas.some((j) => j.path === javaPath) ? (
              <option value={javaPath}>Custom — {javaPath}</option>
            ) : null}
          </SelectField>
          <TextField
            label="Java path override"
            value={javaPath}
            placeholder="Leave empty for global default"
            onChange={(e) => setJavaPath(e.target.value)}
          />
          <Switch label="Keep launcher open" checked={keepOpen} onChange={setKeepOpen} />

          <div className="settings-section stack">
            <h3 style={{ margin: 0, fontSize: 16 }}>Minecraft &amp; loader version</h3>
            <Callout tone="warn">
              Upgrading may break mods. Aureum keeps <code>mods/</code> and content folders; it
              reinstalls version JSON, libraries, and loader bits in place. A full wipe is not
              required for Fabric/Quilt/Vanilla. Forge/NeoForge runs processors during install —
              if that fails, try Install again without deleting mods.
            </Callout>
            <SelectField
              label="Version channel"
              value={versionChannel}
              disabled={installLocked}
              onChange={(e) => setVersionChannel(e.target.value as VersionChannel)}
            >
              <option value="release">Releases</option>
              <option value="snapshot">Snapshots</option>
              <option value="prerelease">Pre-releases &amp; RCs</option>
              <option value="legacy">Legacy</option>
              <option value="all">All</option>
            </SelectField>
            <SelectField
              label="Minecraft version"
              value={upgradeGame}
              disabled={installLocked}
              onChange={(e) => setUpgradeGame(e.target.value)}
            >
              {!visibleGameVersions.some((v) => v.id === upgradeGame) ? (
                <option value={upgradeGame}>{upgradeGame}</option>
              ) : null}
              {visibleGameVersions.map((v) => (
                <option key={v.id} value={v.id}>
                  {versionLabel(v)}
                </option>
              ))}
            </SelectField>
            <SelectField
              label="Loader"
              value={upgradeLoader}
              disabled={installLocked}
              onChange={(e) => setUpgradeLoader(e.target.value)}
            >
              <option value="vanilla">Vanilla</option>
              <option value="fabric">Fabric</option>
              <option value="forge">Forge</option>
              <option value="neoforge">NeoForge</option>
              <option value="quilt">Quilt</option>
            </SelectField>
            {upgradeLoader !== "vanilla" ? (
              <SelectField
                label="Loader version"
                value={upgradeLoaderVer}
                disabled={installLocked || loaderVersions.length === 0}
                onChange={(e) => setUpgradeLoaderVer(e.target.value)}
              >
                {loaderVersions.length === 0 ? (
                  <option value={upgradeLoaderVer || ""}>
                    {upgradeLoaderVer || "Loading…"}
                  </option>
                ) : null}
                {loaderVersions.map((v) => (
                  <option key={v.version} value={v.version}>
                    {v.version}
                    {v.stable ? " (stable)" : ""}
                  </option>
                ))}
              </SelectField>
            ) : null}
            <div className="row">
              <Button
                onClick={() => void applyVersionUpgrade()}
                disabled={
                  installLocked ||
                  !upgradeGame ||
                  (upgradeLoader !== "vanilla" && !upgradeLoaderVer)
                }
              >
                {upgradeBusy ? "Upgrading…" : "Update version & install"}
              </Button>
            </div>
            {installLocked && !upgradeBusy ? (
              <p className="muted" style={{ margin: 0 }}>
                Version changes are locked while install or launch is in progress.
              </p>
            ) : null}
            {upgradeMsg ? <Callout tone="info">{upgradeMsg}</Callout> : null}
          </div>

          {settingsError ? <Callout>{settingsError}</Callout> : null}
          {settingsSaved ? <Callout tone="info">Settings saved.</Callout> : null}
          <div className="row">
            <Button onClick={() => void saveSettings()} disabled={settingsBusy || !name.trim()}>
              Save
            </Button>
            <Button
              variant="outline"
              small
              onClick={() => void api.discoverJava().then(setJavas)}
            >
              Rediscover Java
            </Button>
            <IconButton
              label="Clone"
              variant="outline"
              small
              onClick={() => void api.cloneInstance(instance.id).then(onRefresh)}
            >
              <Copy size={16} aria-hidden />
            </IconButton>
            <IconButton
              label="Delete"
              variant="danger"
              small
              onClick={() => {
                if (confirm(`Delete ${instance.name}?`)) {
                  void api.deleteInstance(instance.id, true).then(onDeleted);
                }
              }}
            >
              <Trash2 size={16} aria-hidden />
            </IconButton>
          </div>
        </div>
      ) : null}

      {tab === "logs" ? (
        <div className="workspace-body stack">
          <div className="row">
            <Button
              small
              variant="outline"
              onClick={() =>
                void api
                  .getLogTail(instance.id, 200)
                  .then(setFileLogs)
                  .catch(() => setFileLogs([]))
              }
            >
              Refresh live tail
            </Button>
            <IconButton
              label="Open instance folder"
              variant="outline"
              small
              onClick={() => void api.openInstanceFolder(instance.id)}
            >
              <FolderOpen size={16} aria-hidden />
            </IconButton>
          </div>
          <div className="log-tail workspace-log">{logs.length ? logs.join("\n") : "No live log output yet."}</div>

          <div className="seg" role="tablist" aria-label="Log folders">
            {(
              [
                ["logs", "logs/"],
                ["crash-reports", "crash-reports/"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={logFolder === id}
                className={logFolder === id ? "active" : ""}
                onClick={() => setLogFolder(id)}
              >
                {label}
              </button>
            ))}
          </div>
          <div className="row">
            <Button
              small
              variant="outline"
              onClick={() =>
                void refreshLogFiles().catch((e) =>
                  setLogFilesError(e instanceof Error ? e.message : String(e)),
                )
              }
            >
              <RefreshCw size={14} aria-hidden /> Refresh files
            </Button>
            <IconButton
              label={`Reveal ${logFolder}`}
              variant="outline"
              small
              onClick={() =>
                void api
                  .openLogFolder(instance.id, logFolder)
                  .catch((e) => setLogFilesError(e instanceof Error ? e.message : String(e)))
              }
            >
              <FolderOpen size={16} aria-hidden />
            </IconButton>
          </div>
          {logFilesError ? <Callout>{logFilesError}</Callout> : null}
          <div className="media-layout">
            <div className="media-list">
              {logFiles.length === 0 ? (
                <p className="muted" style={{ margin: 0 }}>
                  No files in <code>{logFolder}/</code> yet.
                </p>
              ) : (
                <ul className="media-grid">
                  {logFiles.map((f) => (
                    <li key={f.name}>
                      <button
                        type="button"
                        className={`media-tile ${selectedLog === f.name ? "active" : ""}`}
                        onClick={() => void selectLogFile(f.name, f.previewable)}
                      >
                        <span className="media-tile-name">{f.name}</span>
                        <span className="muted">
                          {formatBytes(f.size)}
                          {f.previewable ? "" : " · archive"}
                        </span>
                      </button>
                      <IconButton
                        label={`Delete ${f.name}`}
                        variant="danger"
                        small
                        onClick={() => {
                          if (!confirm(`Delete ${f.name}?`)) return;
                          void api
                            .deleteLogFile(instance.id, logFolder, f.name)
                            .then(async () => {
                              if (selectedLog === f.name) {
                                setSelectedLog(null);
                                setLogPreview(null);
                              }
                              await refreshLogFiles();
                            })
                            .catch((e) =>
                              setLogFilesError(e instanceof Error ? e.message : String(e)),
                            );
                        }}
                      >
                        <Trash2 size={14} aria-hidden />
                      </IconButton>
                    </li>
                  ))}
                </ul>
              )}
            </div>
            <div className="log-file-preview">
              {logPreview != null ? (
                <>
                  {logTruncated ? (
                    <p className="muted" style={{ margin: "0 0 8px" }}>
                      Showing end of file (size/line capped).
                    </p>
                  ) : null}
                  <pre className="log-tail workspace-log">{logPreview}</pre>
                </>
              ) : (
                <p className="muted" style={{ margin: 0 }}>
                  Select a .log / .txt file to preview.
                </p>
              )}
            </div>
          </div>
        </div>
      ) : null}

      {tab === "packs" ? (
        <div className="workspace-body stack">
          <p className="muted" style={{ margin: 0 }}>
            Import or export Modrinth <code>.mrpack</code> packs, and manage resource packs,
            shaders, and datapacks. CurseForge is not scraped.
          </p>
          {!addContent ? (
            <Callout tone="info">
              Turn on Add Content in Settings to install content from the catalog. Import, export,
              and local folder management still work here.
            </Callout>
          ) : null}
          <div className="row">
            <Button onClick={() => void importPack(false)}>
              <FileArchive size={16} aria-hidden /> Import as new instance
            </Button>
            <Button variant="outline" onClick={() => void importPack(true)}>
              Import into this instance
            </Button>
            <Button variant="tonal" onClick={() => void exportPack()}>
              Export .mrpack
            </Button>
          </div>
          {packMsg ? <Callout tone="info">{packMsg}</Callout> : null}

          <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <h3 style={{ margin: 0, fontSize: 16 }}>Installed content</h3>
            <div className="row">
              <Button small variant="outline" onClick={() => void checkContentUpdates()}>
                <RefreshCw size={14} aria-hidden /> Check updates
              </Button>
              <Button small variant="outline" onClick={() => void refreshContent()}>
                Refresh
              </Button>
              <IconButton
                label="Open resource packs"
                variant="outline"
                small
                onClick={() => void api.openContentFolder(instance.id, "resourcepack")}
              >
                <FolderOpen size={16} aria-hidden />
              </IconButton>
            </div>
          </div>
          <p className="muted" style={{ margin: 0 }}>
            Datapack updates use this instance Minecraft version. Resource packs and shaders ignore
            MC version filters.
          </p>
          {contentError ? <Callout>{contentError}</Callout> : null}
          {content.length === 0 ? (
            <p className="muted" style={{ margin: 0 }}>
              No resource packs, shaders, or datapacks yet. Install from the Mods catalog (Add
              Content) or drop files into the folders.
            </p>
          ) : (
            <div className="mod-table-wrap">
              <table className="mod-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Type</th>
                    <th>Source</th>
                    <th>Status</th>
                    <th className="mod-col-actions">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {content.map((c) => (
                    <tr key={c.id}>
                      <td>
                        <div className="mod-name">{c.filename}</div>
                        <div className="muted mod-file">{c.kind}</div>
                      </td>
                      <td>
                        <span className="pill">{contentTypeLabel(c.projectType)}</span>
                      </td>
                      <td>
                        <span className="pill">{sourceLabel(c.source)}</span>
                      </td>
                      <td>
                        {c.compatStatus === "update" || c.updateVersionId ? (
                          <span className="pill">Update</span>
                        ) : c.compatStatus === "incompatible" ? (
                          <span className="pill pill-bad">Wrong MC</span>
                        ) : c.compatStatus === "ok" ? (
                          <span className="muted">OK</span>
                        ) : (
                          <span className="muted">—</span>
                        )}
                      </td>
                      <td className="mod-actions">
                        {c.updateVersionId && c.projectId ? (
                          <Button
                            small
                            variant="tonal"
                            onClick={() => void applyContentUpdate(c.id)}
                          >
                            Update
                          </Button>
                        ) : null}
                        <IconButton
                          label={`Open ${contentTypeLabel(c.projectType)} folder`}
                          variant="outline"
                          small
                          onClick={() =>
                            void api.openContentFolder(instance.id, c.projectType)
                          }
                        >
                          <FolderOpen size={14} aria-hidden />
                        </IconButton>
                        <IconButton
                          label={`Remove ${c.filename}`}
                          variant="danger"
                          small
                          onClick={() =>
                            void api
                              .removeContent(instance.id, c.id)
                              .then(refreshContent)
                              .catch((e) =>
                                setContentError(e instanceof Error ? e.message : String(e)),
                              )
                          }
                        >
                          <Trash2 size={14} aria-hidden />
                        </IconButton>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <div className="row">
            <Button
              small
              variant="outline"
              disabled={!addContent}
              onClick={() => {
                setModsIntent("catalog");
                setRoute("mods");
              }}
            >
              <Plus size={14} aria-hidden /> Browse catalog
            </Button>
            <Button
              small
              variant="text"
              onClick={() => void api.openContentFolder(instance.id, "shader")}
            >
              Shader packs folder
            </Button>
            <Button
              small
              variant="text"
              onClick={() => void api.openContentFolder(instance.id, "datapack")}
            >
              Datapacks folder
            </Button>
          </div>
        </div>
      ) : null}

      {tab === "screenshots" ? (
        <div className="workspace-body stack">
          {mediaError ? <Callout>{mediaError}</Callout> : null}
          <div className="row">
            <Button
              small
              disabled={mediaBusy}
              onClick={() =>
                void api
                  .pickMediaFiles()
                  .then((paths) => importShots(paths))
                  .catch((e) => setMediaError(e instanceof Error ? e.message : String(e)))
              }
            >
              <Plus size={14} aria-hidden /> Import
            </Button>
            <Button
              small
              variant="outline"
              disabled={mediaBusy}
              onClick={() =>
                void refreshMedia().catch((e) =>
                  setMediaError(e instanceof Error ? e.message : String(e)),
                )
              }
            >
              <RefreshCw size={14} aria-hidden /> Refresh
            </Button>
            <IconButton
              label="Reveal screenshots folder"
              variant="outline"
              small
              onClick={() =>
                void api
                  .openMediaFolder(instance.id)
                  .catch((e) => setMediaError(e instanceof Error ? e.message : String(e)))
              }
            >
              <FolderOpen size={16} aria-hidden />
            </IconButton>
          </div>
          <p className="muted" style={{ margin: 0 }}>
            Minecraft F2 shots land in <code>screenshots/</code>. Drag-drop images onto this
            window or use Import.
          </p>
          <div className="media-layout">
            <div className="media-preview">
              {previewUrl ? (
                <img src={previewUrl} alt={selectedShot ?? "Screenshot preview"} />
              ) : (
                <div className="media-preview-empty">
                  <ImageIcon size={32} aria-hidden />
                  <span className="muted">Select a screenshot to preview</span>
                </div>
              )}
            </div>
            <div className="media-list">
              {media.length === 0 ? (
                <div className="empty-mods">
                  <h3>No screenshots yet</h3>
                  <p className="muted">Take one in-game (F2) or import an image.</p>
                </div>
              ) : (
                <ul className="media-grid media-thumb-grid">
                  {media.map((m) => (
                    <li key={m.name}>
                      <button
                        type="button"
                        className={`media-tile media-thumb-tile ${selectedShot === m.name ? "active" : ""}`}
                        onClick={() => void selectShot(m.name)}
                      >
                        {thumbs[m.name] ? (
                          <img src={thumbs[m.name]} alt="" className="media-thumb" />
                        ) : (
                          <span className="media-thumb media-thumb-placeholder">
                            <ImageIcon size={18} aria-hidden />
                          </span>
                        )}
                        <span className="media-tile-name">{m.name}</span>
                        <span className="muted">{formatBytes(m.size)}</span>
                      </button>
                      <IconButton
                        label={`Delete ${m.name}`}
                        variant="danger"
                        small
                        onClick={() => {
                          if (!confirm(`Delete ${m.name}?`)) return;
                          void api
                            .deleteMediaFile(instance.id, m.name)
                            .then(async () => {
                              if (selectedShot === m.name) {
                                setSelectedShot(null);
                                setPreviewUrl(null);
                              }
                              setThumbs((prev) => {
                                const next = { ...prev };
                                delete next[m.name];
                                return next;
                              });
                              await refreshMedia();
                            })
                            .catch((e) =>
                              setMediaError(e instanceof Error ? e.message : String(e)),
                            );
                        }}
                      >
                        <Trash2 size={14} aria-hidden />
                      </IconButton>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
      ) : null}
    </aside>
  );
}
