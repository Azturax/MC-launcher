import { FolderOpen, Pin, Plus, RefreshCw, RotateCcw, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import type {
  CatalogCategory,
  CatalogProvider,
  CatalogSort,
  GameVersion,
  InstalledMod,
  ModChannel,
  ProjectHit,
  ProjectType,
  VersionChannel,
} from "../api/types";
import { ignoresInstanceVersion } from "../api/types";
import { filterGameVersions, versionLabel } from "../api/versions";
import {
  instanceTargetLabel,
  loaderLabel,
  supportsGameVersion,
  supportsLoader,
} from "../api/loaders";
import { ProjectDetailPanel } from "../components/ProjectDetailPanel";
import { Button, Callout, IconButton, SelectField, Switch } from "../components/ui";
import { VersionPicker } from "../components/VersionPicker";
import { useAppStore } from "../store/app";

const LOADERS = ["fabric", "forge", "neoforge", "quilt"] as const;

const PROJECT_TYPES: { id: ProjectType; label: string }[] = [
  { id: "mod", label: "Mods" },
  { id: "shader", label: "Shaders" },
  { id: "resourcepack", label: "Resource packs" },
  { id: "datapack", label: "Data packs" },
  { id: "modpack", label: "Modpacks" },
];

type ModsTab = "installed" | "catalog";

function categoryLabel(name: string) {
  return name
    .split(/[-_]/)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}

function projectPage(hit: ProjectHit) {
  const kind = hit.projectType || "mod";
  return `https://modrinth.com/${kind}/${hit.slug || hit.id}`;
}

function modName(m: InstalledMod) {
  return m.displayName || m.filename.replace(/\.jar$/i, "");
}

function sourceLabel(source: string) {
  if (source === "modrinth") return "Modrinth";
  if (source === "curseforge") return "CurseForge";
  if (source === "local") return "Local";
  return source;
}

function isLocalMod(m: InstalledMod) {
  return m.source === "local" || m.projectId.startsWith("local:");
}

function isPackType(type: string) {
  return (
    type === "modpack" ||
    type === "resourcepack" ||
    type === "datapack" ||
    type === "shader"
  );
}

export function Mods() {
  const {
    instances,
    selectedId,
    setSelectedId,
    modsIntent,
    setModsIntent,
    addContent,
    setRoute,
    setWorkspaceTab,
    setInstances,
  } = useAppStore();
  // Install target is only the explicitly selected instance — never silently fall back.
  const selected = instances.find((i) => i.id === selectedId) ?? null;
  const [tab, setTab] = useState<ModsTab>(modsIntent === "catalog" ? "catalog" : "installed");
  const [providers, setProviders] = useState<CatalogProvider[]>([]);
  const [source, setSource] = useState("modrinth");
  /** When true (default), catalog filters track the selected instance's loader + MC version. */
  const [followInstance, setFollowInstance] = useState(true);
  const [loader, setLoader] = useState("all");
  const [gameVersion, setGameVersion] = useState("all");
  const [versionChannel, setVersionChannel] = useState<VersionChannel>("release");
  const [projectType, setProjectType] = useState<ProjectType>("mod");
  const [category, setCategory] = useState("all");
  const [sort, setSort] = useState<CatalogSort>("relevance");
  const [query, setQuery] = useState("");
  const [installedFilter, setInstalledFilter] = useState("");
  const [channel, setChannel] = useState<ModChannel>("stable");
  const [versions, setVersions] = useState<GameVersion[]>([]);
  const [hits, setHits] = useState<ProjectHit[]>([]);
  const [total, setTotal] = useState(0);
  const [installed, setInstalled] = useState<InstalledMod[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [providerNote, setProviderNote] = useState<string | null>(null);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [pickerHit, setPickerHit] = useState<ProjectHit | null>(null);
  const [detailHit, setDetailHit] = useState<ProjectHit | null>(null);
  const [catalogCategories, setCatalogCategories] = useState<CatalogCategory[]>([]);

  const installedIds = useMemo(
    () => new Set(installed.map((m) => m.projectId)),
    [installed],
  );

  const visibleInstalled = useMemo(() => {
    const q = installedFilter.trim().toLowerCase();
    if (!q) return installed;
    return installed.filter((m) => {
      const hay = `${modName(m)} ${m.filename} ${m.source}`.toLowerCase();
      return hay.includes(q);
    });
  }, [installed, installedFilter]);

  const categories = useMemo(
    () =>
      catalogCategories
        .filter((c) => c.projectType === projectType)
        .map((c) => ({ id: c.name, label: categoryLabel(c.name), header: c.header })),
    [catalogCategories, projectType],
  );
  const ignoreVersion = ignoresInstanceVersion(projectType);
  const visibleVersions = useMemo(
    () => filterGameVersions(versions, versionChannel),
    [versions, versionChannel],
  );

  const browseProviders = useMemo<CatalogProvider[]>(
    () => [
      ...providers,
      {
        id: "local",
        label: "Local files",
        enabled: true,
        reason: "Drop jars into the instance mods folder, like Prism or MultiMC.",
      },
    ],
    [providers],
  );

  useEffect(() => {
    if (modsIntent === "catalog") {
      setTab("catalog");
      setProjectType("mod");
      setModsIntent(null);
    }
  }, [modsIntent, setModsIntent]);

  useEffect(() => {
    void api.listCatalogProviders().then(setProviders).catch(() => {
      setProviders([
        { id: "modrinth", label: "Modrinth", enabled: true },
        {
          id: "curseforge",
          label: "CurseForge",
          enabled: false,
          reason: "Hidden until a licensed API key is configured.",
        },
      ]);
    });
    void api.listGameVersions().then(setVersions).catch(() => undefined);
    void api.listCatalogCategories().then(setCatalogCategories).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (ignoreVersion) return;
    if (!followInstance || !selected) return;
    if (selected.loader === "vanilla") {
      setLoader("all");
    } else {
      setLoader(selected.loader);
    }
    setGameVersion(selected.gameVersion || "all");
  }, [followInstance, selected?.id, selected?.loader, selected?.gameVersion, ignoreVersion]);

  useEffect(() => {
    if (
      projectType === "mod" ||
      projectType === "datapack"
    ) {
      setFollowInstance(true);
    }
    if (ignoreVersion) {
      setFollowInstance(false);
      setLoader("all");
      setGameVersion("all");
    }
  }, [projectType, ignoreVersion]);

  async function refreshInstalled(id: string) {
    setInstalled(await api.listInstanceMods(id));
  }

  useEffect(() => {
    if (!selected) {
      setInstalled([]);
      return;
    }
    // Prefer instance-scoped update/compat evaluation when opening installed list.
    void api
      .checkModUpdates(selected.id)
      .then(setInstalled)
      .catch(() =>
        refreshInstalled(selected.id).catch((e) => {
          setError(e instanceof Error ? e.message : String(e));
          setInstalled([]);
        }),
      );
  }, [selected?.id]);

  async function runSearch() {
    if (source === "local") return;
    setError(null);
    try {
      // Resource packs & shaders: never send versions/loaders facets.
      // Mods / datapacks / modpacks: prioritize selected instance when followInstance.
      let loaderFilter: string[] | undefined;
      let gameFilter: string[] | undefined;
      if (!ignoreVersion) {
        const effectiveLoader =
          followInstance && selected && selected.loader !== "vanilla"
            ? selected.loader
            : loader;
        const effectiveGame =
          followInstance && selected?.gameVersion ? selected.gameVersion : gameVersion;
        if (
          effectiveLoader !== "all" &&
          (projectType === "mod" || projectType === "datapack" || followInstance)
        ) {
          loaderFilter = [effectiveLoader];
        }
        if (effectiveGame !== "all") {
          gameFilter = [effectiveGame];
        }
      }
      const page = await api.searchCatalog({
        query,
        source,
        loaders: loaderFilter,
        gameVersions: gameFilter,
        projectTypes: [projectType],
        categories: category !== "all" ? [category] : undefined,
        index: sort,
        channel,
        limit: 24,
      });
      setHits(page.hits);
      setTotal(page.total);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    if (tab !== "catalog" || source === "local") return;
    void runSearch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, source, loader, gameVersion, projectType, category, sort, channel, selected?.id, followInstance]);

  async function install(projectId: string, versionId?: string, projectType: ProjectType = "mod") {
    setBusy(projectId);
    setError(null);
    try {
      if (projectType === "modpack") {
        const result = await api.installModpack({
          projectId,
          versionId,
          channel,
        });
        setInstances(await api.listInstances());
        setSelectedId(result.instance.id);
        setWorkspaceTab("mods");
        setRoute("home");
        return;
      }
      if (!selected) {
        setError("Select an instance first.");
        return;
      }
      if (projectType === "mod") {
        if (selected.loader === "vanilla") {
          setError("Vanilla instances cannot load mods.");
          return;
        }
        await api.installMod({
          instanceId: selected.id,
          projectId,
          versionId,
          channel,
        });
        await refreshInstalled(selected.id);
        return;
      }
      if (!addContent) {
        setError("Enable Add Content in Settings to install resource packs, shaders, and datapacks.");
        return;
      }
      const result = await api.installContent({
        instanceId: selected.id,
        projectId,
        versionId,
        projectType,
        channel,
      });
      setError(null);
      // Reuse error callout as success-ish info via a soft message
      setProviderNote(`Installed ${result.filename} → ${result.projectType}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }

  function canInstallHit(hit: ProjectHit): boolean {
    if (busy === hit.id) return false;
    if (hit.projectType === "modpack") return true;
    if (!selected) return false;
    if (hit.projectType === "mod") {
      if (selected.loader === "vanilla") return false;
      // Facet soft-check: only block clear loader mismatches. Game version is
      // refined in the version picker (search facets can be incomplete).
      if (hit.loaders.length && !supportsLoader(hit.loaders, selected.loader)) return false;
      return true;
    }
    if (isPackType(hit.projectType) && hit.projectType !== "modpack") {
      return addContent;
    }
    return false;
  }

  function installLabel(hit: ProjectHit): string {
    if (hit.projectType === "modpack") return "Install as new instance";
    if (!selected) return "Select instance first";
    if (hit.projectType === "mod") {
      if (selected.loader === "vanilla") return "Needs modded instance";
      return installedIds.has(hit.id) ? "Reinstall" : "Install to instance";
    }
    if (hit.projectType === "resourcepack") return "Install resource pack";
    if (hit.projectType === "shader") return "Install shader";
    if (hit.projectType === "datapack") return "Install datapack";
    return "Install";
  }

  function hitCompatibility(hit: ProjectHit): "ok" | "warn" | "bad" | null {
    if (!selected || hit.projectType === "modpack") return null;
    // Resource packs & shaders: never mark incompatible for MC/loader mismatch.
    if (ignoresInstanceVersion(hit.projectType)) return null;
    if (hit.projectType === "datapack") {
      const gameOk =
        !hit.gameVersions.length ||
        supportsGameVersion(hit.gameVersions, selected.gameVersion);
      return gameOk ? "ok" : "warn";
    }
    if (hit.projectType !== "mod") return null;
    if (selected.loader === "vanilla") return "bad";
    const loaderOk = !hit.loaders.length || supportsLoader(hit.loaders, selected.loader);
    const gameOk =
      !hit.gameVersions.length || supportsGameVersion(hit.gameVersions, selected.gameVersion);
    if (loaderOk && gameOk) return "ok";
    if (loaderOk || gameOk) return "warn";
    return "bad";
  }

  async function checkUpdates() {
    if (!selected) return;
    setCheckingUpdates(true);
    setError(null);
    try {
      setInstalled(await api.checkModUpdates(selected.id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      await refreshInstalled(selected.id);
    } finally {
      setCheckingUpdates(false);
    }
  }

  function openVersionPicker(hit: ProjectHit) {
    if (hit.projectType !== "modpack" && !selected) {
      setError("Select a target instance before installing.");
      return;
    }
    if (!canInstallHit(hit) && hit.projectType !== "modpack") return;
    setPickerHit(hit);
  }

  function openDetail(hit: ProjectHit) {
    setDetailHit(hit);
  }

  function pickerLoaders(): string[] | undefined {
    if (!pickerHit) return undefined;
    if (ignoresInstanceVersion(pickerHit.projectType) || pickerHit.projectType === "modpack") {
      return undefined;
    }
    if (selected && selected.loader !== "vanilla") return [selected.loader];
    if (loader !== "all") return [loader];
    return undefined;
  }

  function pickerGames(): string[] | undefined {
    if (!pickerHit) return undefined;
    if (ignoresInstanceVersion(pickerHit.projectType) || pickerHit.projectType === "modpack") {
      return undefined;
    }
    if (selected?.gameVersion) return [selected.gameVersion];
    if (gameVersion !== "all") return [gameVersion];
    return undefined;
  }

  function compatStatusLabel(m: InstalledMod): string {
    if (m.pinned) return "Pinned";
    switch (m.compatStatus) {
      case "update":
        return "Update for this instance";
      case "incompatible":
        return "Wrong loader / MC";
      case "ok":
        return "Matches instance";
      case "local":
        return "Local";
      case "pinned":
        return "Pinned";
      default:
        if (m.updateVersionId) return "Update";
        return m.enabled ? "Ready" : "Disabled";
    }
  }

  function pickProvider(p: CatalogProvider) {
    if (!p.enabled) {
      setProviderNote(p.reason ?? "This source is not available yet.");
      return;
    }
    setProviderNote(p.id === "local" ? (p.reason ?? null) : null);
    setSource(p.id);
  }

  const updateCount = installed.filter((m) => m.updateVersionId && !m.pinned).length;

  return (
    <>
      <div className="topbar">
        <h1>Mods</h1>
        <SelectField
          label="Target instance"
          value={selected?.id ?? ""}
          onChange={(e) => setSelectedId(e.target.value || null)}
        >
          <option value="">Select instance…</option>
          {instances.map((i) => (
            <option key={i.id} value={i.id}>
              {i.name} · {loaderLabel(i.loader)} · MC {i.gameVersion}
            </option>
          ))}
        </SelectField>
        <div className="seg" role="tablist" aria-label="Mods views">
          <button
            type="button"
            role="tab"
            className={tab === "installed" ? "active" : ""}
            aria-selected={tab === "installed"}
            onClick={() => setTab("installed")}
          >
            Installed
            {installed.length ? ` (${installed.length})` : ""}
          </button>
          <button
            type="button"
            role="tab"
            className={tab === "catalog" ? "active" : ""}
            aria-selected={tab === "catalog"}
            onClick={() => setTab("catalog")}
          >
            Add from catalog
          </button>
        </div>
      </div>
      <div className="content stack">
        {!selected ? (
          <Callout tone="info">
            Select a target instance in the dropdown above. Mods, resource packs, shaders, and
            datapacks install into that instance only. Modpacks create a new instance from the pack.
          </Callout>
        ) : null}

        {selected ? (
          <div className="target-banner">
            <div>
              <strong>Target instance</strong>
              <p className="muted" style={{ margin: 0 }}>
                {instanceTargetLabel(selected)}
              </p>
            </div>
            {selected.loader === "vanilla" && projectType === "mod" ? (
              <Callout tone="warn">
                This instance is Vanilla — it cannot load mods. Switch instance or create a Fabric /
                Forge / NeoForge / Quilt one.
              </Callout>
            ) : null}
          </div>
        ) : null}

        {error ? <Callout>{error}</Callout> : null}

        {tab === "catalog" && isPackType(projectType) ? (
          <div className={`add-content-notice ${addContent || projectType === "modpack" ? "" : "needs-enable"}`}>
            <strong>
              {projectType === "modpack"
                ? "One-click modpack install"
                : addContent
                  ? "Content packs install to this instance"
                  : "Enable Add Content to add packs"}
            </strong>
            <p>
              {projectType === "modpack"
                ? "Install downloads the Modrinth .mrpack and creates a new instance with the pack’s own loader and Minecraft version. It does not change your selected instance’s loader. CurseForge is not scraped."
                : addContent
                  ? "Resource packs → resourcepacks/, shaders → shaderpacks/, datapacks → datapacks/ on the selected instance. Hashes are verified from the Modrinth CDN."
                  : "Packs are gated behind Add Content. Turn it on in Settings to install resource packs, shaders, and datapacks."}
            </p>
            <div className="row">
              {!addContent && projectType !== "modpack" ? (
                <Button variant="filled" small onClick={() => setRoute("settings")}>
                  Enable Add Content
                </Button>
              ) : null}
              {projectType === "modpack" ? (
                <Button
                  variant="outline"
                  small
                  onClick={() => {
                    setWorkspaceTab("packs");
                    setRoute("home");
                  }}
                >
                  Open Packs tab
                </Button>
              ) : null}
            </div>
          </div>
        ) : null}

        {tab === "installed" && selected ? (
          <>
            <div className="mods-toolbar">
              <IconButton label="Add mods" onClick={() => setTab("catalog")}>
                <Plus size={18} aria-hidden />
              </IconButton>
              <IconButton
                label="Open folder"
                variant="outline"
                onClick={() => void api.openModsFolder(selected.id)}
              >
                <FolderOpen size={18} aria-hidden />
              </IconButton>
              <IconButton
                label={checkingUpdates ? "Checking updates…" : "Check updates"}
                variant="tonal"
                disabled={checkingUpdates}
                onClick={() => void checkUpdates()}
              >
                <RefreshCw size={18} aria-hidden className={checkingUpdates ? "spin" : undefined} />
              </IconButton>
              <IconButton
                label="Rescan"
                variant="text"
                onClick={() => void refreshInstalled(selected.id)}
              >
                <RotateCcw size={18} aria-hidden />
              </IconButton>
              {updateCount ? (
                <span className="pill">{updateCount} update{updateCount === 1 ? "" : "s"}</span>
              ) : null}
              <div className="search row" style={{ marginLeft: "auto", maxWidth: 280 }}>
                <Search size={16} />
                <input
                  style={{ flex: 1, border: 0, background: "transparent", color: "inherit" }}
                  placeholder="Filter installed"
                  value={installedFilter}
                  onChange={(e) => setInstalledFilter(e.target.value)}
                />
              </div>
            </div>
            <p className="muted">
              {selected.name} · {installed.length} file{installed.length === 1 ? "" : "s"}
              {" · "}
              Catalog installs stay in the lockfile. Jars you drop into the mods
              folder show up as Local after Rescan.
            </p>
            {installed.length === 0 ? (
              <div className="empty-mods">
                <h3>No mods on this instance</h3>
                <p className="muted">
                  Add from Modrinth, or copy jars into the mods folder like MultiMC.
                </p>
                <div className="row">
                  <Button onClick={() => setTab("catalog")}>Browse Modrinth</Button>
                  <Button variant="outline" onClick={() => void api.openModsFolder(selected.id)}>
                    Open mods folder
                  </Button>
                </div>
              </div>
            ) : (
              <div className="mod-table-wrap">
                <table className="mod-table">
                  <thead>
                    <tr>
                      <th className="mod-col-on">On</th>
                      <th>Name</th>
                      <th>Version</th>
                      <th>Source</th>
                      <th>Status</th>
                      <th className="mod-col-actions">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {visibleInstalled.map((m) => (
                      <tr key={m.id} className={m.enabled ? "" : "disabled-mod"}>
                        <td>
                          <input
                            type="checkbox"
                            checked={m.enabled}
                            aria-label={`Enable ${modName(m)}`}
                            onChange={() =>
                              void api
                                .setModEnabled(selected.id, m.projectId, !m.enabled)
                                .then(() => refreshInstalled(selected.id))
                                .catch((e) =>
                                  setError(e instanceof Error ? e.message : String(e)),
                                )
                            }
                          />
                        </td>
                        <td>
                          <div className="mod-name">{modName(m)}</div>
                          <div className="muted mod-file">{m.filename}</div>
                        </td>
                        <td>{m.versionNumber || "—"}</td>
                        <td>
                          <span className="pill">{sourceLabel(m.source)}</span>
                        </td>
                        <td>
                          {m.compatStatus === "incompatible" ? (
                            <span className="pill pill-bad">{compatStatusLabel(m)}</span>
                          ) : m.updateVersionId && !m.pinned ? (
                            <span className="pill">{compatStatusLabel(m)}</span>
                          ) : m.compatStatus === "ok" ? (
                            <span className="pill pill-match">{compatStatusLabel(m)}</span>
                          ) : (
                            <span className="muted">{compatStatusLabel(m)}</span>
                          )}
                        </td>
                        <td className="mod-actions">
                          {!isLocalMod(m) ? (
                            <Button
                              variant="tonal"
                              small
                              title={m.pinned ? "Unpin version" : "Pin this version"}
                              onClick={() =>
                                void api
                                  .setModPin(selected.id, m.projectId, !m.pinned)
                                  .then(() => refreshInstalled(selected.id))
                              }
                            >
                              <Pin size={14} />
                            </Button>
                          ) : null}
                          {m.updateVersionId && !m.pinned ? (
                            <Button
                              small
                              disabled={busy === m.projectId}
                              onClick={() =>
                                void install(m.projectId, m.updateVersionId ?? undefined)
                              }
                            >
                              Update
                            </Button>
                          ) : null}
                          <Button
                            variant="danger"
                            small
                            onClick={() =>
                              void api
                                .removeMod(selected.id, m.projectId)
                                .then(() => refreshInstalled(selected.id))
                            }
                          >
                            Remove
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        ) : null}

        {tab === "catalog" ? (
          <>
            <section className="settings-section stack">
              <div className="stack" style={{ gap: 8 }}>
                <span className="field-label">Provider</span>
                <div className="row">
                  {browseProviders.map((p) => (
                    <button
                      key={p.id}
                      type="button"
                      className={`pill pill-btn ${source === p.id ? "active" : ""}`}
                      title={p.enabled ? p.label : (p.reason ?? "Unavailable")}
                      onClick={() => pickProvider(p)}
                    >
                      {p.label}
                      {!p.enabled ? " · locked" : ""}
                    </button>
                  ))}
                </div>
              </div>
              {providerNote ? <Callout tone="info">{providerNote}</Callout> : null}

              {source === "local" ? (
                <div className="empty-mods">
                  <h3>Add from disk</h3>
                  <p className="muted">
                    Prism and MultiMC treat the instance <code>mods</code> folder as
                    the list. Copy <code>.jar</code> files in, then Rescan. Disable
                    a jar by renaming it to <code>.jar.disabled</code>, or use the
                    checkbox on Installed.
                  </p>
                  <div className="row">
                    <Button
                      disabled={!selected}
                      onClick={() => selected && void api.openModsFolder(selected.id)}
                    >
                      <FolderOpen size={16} /> Open mods folder
                    </Button>
                    <Button
                      variant="outline"
                      disabled={!selected}
                      onClick={() => {
                        if (!selected) return;
                        void refreshInstalled(selected.id).then(() => setTab("installed"));
                      }}
                    >
                      Rescan and show list
                    </Button>
                  </div>
                </div>
              ) : (
                <>
                  {ignoreVersion ? (
                    <Callout tone="info">
                      Resource packs and shaders are not filtered by Minecraft or loader version —
                      pick any build. Mods, datapacks, and modpacks still prioritize the selected
                      instance.
                    </Callout>
                  ) : (
                    <Switch
                      label={
                        selected
                          ? `Match ${selected.name} (${loaderLabel(selected.loader)} · ${selected.gameVersion})`
                          : "Match selected instance loader + Minecraft version"
                      }
                      checked={followInstance}
                      onChange={(v) => {
                        setFollowInstance(v);
                        if (!v && projectType === "mod") {
                          setProviderNote(
                            "Browsing without instance filters may show incompatible mods. Install still requires a matching selected instance.",
                          );
                        } else {
                          setProviderNote(null);
                        }
                      }}
                    />
                  )}
                  <div className="filters-bar">
                    <SelectField
                      label="Loader"
                      value={loader}
                      disabled={ignoreVersion || followInstance}
                      onChange={(e) => setLoader(e.target.value)}
                    >
                      <option value="all">Any loader</option>
                      {LOADERS.map((l) => (
                        <option key={l} value={l}>
                          {loaderLabel(l)}
                        </option>
                      ))}
                    </SelectField>
                    <SelectField
                      label="Version channel"
                      value={versionChannel}
                      disabled={ignoreVersion || followInstance}
                      onChange={(e) => {
                        const next = e.target.value as VersionChannel;
                        setVersionChannel(next);
                        if (gameVersion !== "all") {
                          const nextList = filterGameVersions(versions, next);
                          if (!nextList.some((v) => v.id === gameVersion)) {
                            setGameVersion("all");
                          }
                        }
                      }}
                    >
                      <option value="release">Releases</option>
                      <option value="snapshot">Snapshots</option>
                      <option value="prerelease">Pre-releases &amp; RCs</option>
                      <option value="legacy">Legacy</option>
                      <option value="all">All</option>
                    </SelectField>
                    <SelectField
                      label="Game version"
                      value={gameVersion}
                      disabled={ignoreVersion || followInstance}
                      onChange={(e) => setGameVersion(e.target.value)}
                    >
                      <option value="all">Any version</option>
                      {selected && !visibleVersions.some((v) => v.id === selected.gameVersion) ? (
                        <option value={selected.gameVersion}>{selected.gameVersion}</option>
                      ) : null}
                      {visibleVersions.map((v) => (
                        <option key={v.id} value={v.id}>
                          {versionLabel(v)}
                        </option>
                      ))}
                    </SelectField>
                    <SelectField
                      label="Type"
                      value={projectType}
                      onChange={(e) => {
                        setProjectType(e.target.value as ProjectType);
                        setCategory("all");
                      }}
                    >
                      {PROJECT_TYPES.map((t) => (
                        <option key={t.id} value={t.id}>
                          {t.label}
                        </option>
                      ))}
                    </SelectField>
                    <SelectField
                      label="Sort"
                      value={sort}
                      onChange={(e) => setSort(e.target.value as CatalogSort)}
                    >
                      <option value="relevance">Relevance</option>
                      <option value="downloads">Downloads</option>
                      <option value="updated">Updated</option>
                      <option value="newest">Newest</option>
                    </SelectField>
                    <SelectField
                      label="Channel"
                      value={channel}
                      onChange={(e) => setChannel(e.target.value as ModChannel)}
                    >
                      <option value="stable">Stable</option>
                      <option value="beta">Beta</option>
                      <option value="all">All</option>
                    </SelectField>
                  </div>
                  <div className="stack" style={{ gap: 8 }}>
                    <span className="field-label">Category</span>
                    <div className="row" style={{ flexWrap: "wrap" }}>
                      <button
                        type="button"
                        className={`pill pill-btn ${category === "all" ? "active" : ""}`}
                        onClick={() => setCategory("all")}
                      >
                        Any
                      </button>
                      {categories.map((c) => (
                        <button
                          key={c.id}
                          type="button"
                          className={`pill pill-btn ${category === c.id ? "active" : ""}`}
                          onClick={() => setCategory(c.id)}
                        >
                          {c.label}
                        </button>
                      ))}
                    </div>
                  </div>
                  <div className="search row" style={{ maxWidth: 520 }}>
                    <Search size={16} />
                    <input
                      style={{ flex: 1, border: 0, background: "transparent", color: "inherit" }}
                      placeholder={`Search ${browseProviders.find((p) => p.id === source)?.label ?? "catalog"}`}
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void runSearch();
                      }}
                    />
                    <Button variant="tonal" small onClick={() => void runSearch()}>
                      Search
                    </Button>
                  </div>
                  <p className="muted">
                    {total} projects · {browseProviders.find((p) => p.id === source)?.label ?? source}
                    {loader !== "all" ? ` · ${loaderLabel(loader)}` : ""}
                    {gameVersion !== "all" ? ` · MC ${gameVersion}` : ""}
                    {category !== "all" ? ` · ${category}` : ""}. Installs target the selected
                    instance (except modpacks, which create a new one). CDN hashes verified.
                  </p>
                  <div className="card-grid">
                    {hits.map((hit) => {
                      const compat = hitCompatibility(hit);
                      return (
                      <article
                        key={`${hit.source}-${hit.id}`}
                        className={`instance-card ${compat === "bad" ? "incompatible" : ""}`}
                      >
                        <div className="row">
                          <span className="pill">{hit.source}</span>
                          <span className="muted">{hit.projectType}</span>
                          {hit.loaders.slice(0, 3).map((l) => (
                            <span
                              key={l}
                              className={`pill ${selected && supportsLoader([l], selected.loader) ? "pill-match" : ""}`}
                            >
                              {loaderLabel(l)}
                            </span>
                          ))}
                          {compat === "ok" ? (
                            <span className="pill pill-match">Matches instance</span>
                          ) : null}
                          {compat === "bad" ? (
                            <span className="pill pill-bad">Incompatible</span>
                          ) : null}
                          {compat === "warn" ? (
                            <span className="pill pill-warn">Check version</span>
                          ) : null}
                        </div>
                        <h3
                          style={{ cursor: "pointer" }}
                          onClick={() => openDetail(hit)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") openDetail(hit);
                          }}
                          role="link"
                          tabIndex={0}
                        >
                          {hit.title}
                        </h3>
                        <p className="muted" style={{ margin: 0 }}>
                          {hit.description}
                        </p>
                        <div className="row">
                          <Button
                            small
                            disabled={!canInstallHit(hit)}
                            onClick={() => openVersionPicker(hit)}
                          >
                            {installLabel(hit)}
                          </Button>
                          <Button variant="tonal" small onClick={() => openDetail(hit)}>
                            Details
                          </Button>
                          <Button
                            variant="text"
                            small
                            onClick={() => void api.openExternal(projectPage(hit))}
                          >
                            Page
                          </Button>
                        </div>
                      </article>
                      );
                    })}
                  </div>
                </>
              )}
            </section>
          </>
        ) : null}
      </div>
      {pickerHit ? (
        <VersionPicker
          hit={pickerHit}
          projectType={(pickerHit.projectType as ProjectType) || projectType}
          channel={channel}
          loaders={pickerLoaders()}
          gameVersions={pickerGames()}
          targetInstance={selected}
          onChannelChange={setChannel}
          onClose={() => setPickerHit(null)}
          onConfirm={(versionId) => {
            const hit = pickerHit;
            const type = (hit.projectType as ProjectType) || projectType;
            setPickerHit(null);
            void install(hit.id, versionId ?? undefined, type);
          }}
        />
      ) : null}
      {detailHit ? (
        <ProjectDetailPanel
          hit={detailHit}
          targetInstance={selected}
          channel={channel}
          onChannelChange={setChannel}
          onClose={() => setDetailHit(null)}
          onInstall={(versionId) => {
            const hit = detailHit;
            const type = (hit.projectType as ProjectType) || projectType;
            setDetailHit(null);
            void install(hit.id, versionId ?? undefined, type);
          }}
        />
      ) : null}
    </>
  );
}
