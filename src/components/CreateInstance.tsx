import { useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import { LOADER_OPTIONS, loaderHint, loaderLabel } from "../api/loaders";
import type { GameVersion, InstanceTemplate, LoaderVersion, VersionChannel } from "../api/types";
import { filterGameVersions, versionLabel } from "../api/versions";
import { Button, Callout, Dialog, SelectField, TextField } from "./ui";

export function CreateInstance({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: () => void;
}) {
  const [name, setName] = useState("New instance");
  const [loader, setLoader] = useState<string>("vanilla");
  const [gameVersion, setGameVersion] = useState("1.21.1");
  const [versionChannel, setVersionChannel] = useState<VersionChannel>("release");
  const [loaderVersion, setLoaderVersion] = useState("");
  const [versions, setVersions] = useState<GameVersion[]>([]);
  const [loaders, setLoaders] = useState<LoaderVersion[]>([]);
  const [templates, setTemplates] = useState<InstanceTemplate[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .listGameVersions()
      .then((list) => {
        setVersions(list);
        const latest = list.find((v) => v.latest);
        if (latest) setGameVersion(latest.id);
      })
      .catch((e: Error) => setError(e.message));
    api.listTemplates().then(setTemplates).catch(() => undefined);
  }, []);

  const visibleVersions = useMemo(
    () => filterGameVersions(versions, versionChannel),
    [versions, versionChannel],
  );

  useEffect(() => {
    if (visibleVersions.length === 0) return;
    if (visibleVersions.some((v) => v.id === gameVersion)) return;
    const preferred =
      versionChannel === "snapshot" || versionChannel === "prerelease"
        ? (visibleVersions.find((v) => v.latestSnapshot) ?? visibleVersions[0])
        : (visibleVersions.find((v) => v.latest) ?? visibleVersions[0]);
    setGameVersion(preferred.id);
  }, [visibleVersions, gameVersion, versionChannel]);

  useEffect(() => {
    if (loader === "vanilla") {
      setLoaders([]);
      setLoaderVersion("");
      return;
    }
    api
      .listLoaderVersions(loader, gameVersion)
      .then((list) => {
        setLoaders(list);
        setLoaderVersion(list.find((l) => l.stable)?.version ?? list[0]?.version ?? "");
      })
      .catch(() => setLoaders([]));
  }, [loader, gameVersion]);

  const summary =
    loader === "vanilla"
      ? `Vanilla · Minecraft ${gameVersion}`
      : `${loaderLabel(loader)} ${loaderVersion || "…"} · Minecraft ${gameVersion}`;

  async function create() {
    setBusy(true);
    setError(null);
    try {
      if (loader !== "vanilla" && !loaderVersion) {
        setError(`No ${loaderLabel(loader)} builds are published for ${gameVersion}.`);
        return;
      }
      await api.createInstance({
        name,
        loader,
        gameVersion,
        loaderVersion: loader === "vanilla" ? null : loaderVersion,
        keepOpen: true,
      });
      onCreated();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function fromTemplate(id: string) {
    setBusy(true);
    setError(null);
    try {
      await api.applyTemplate(id, name);
      onCreated();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title="Create instance" onClose={onClose}>
      <TextField label="Name" value={name} onChange={(e) => setName(e.target.value)} />

      <div className="field">
        <span>Mod loader</span>
        <div className="loader-picker" role="radiogroup" aria-label="Mod loader">
          {LOADER_OPTIONS.map((l) => (
            <button
              key={l.id}
              type="button"
              role="radio"
              aria-checked={loader === l.id}
              className={`loader-option ${loader === l.id ? "active" : ""}`}
              onClick={() => setLoader(l.id)}
            >
              <strong>{l.label}</strong>
              <span className="muted">{l.hint}</span>
            </button>
          ))}
        </div>
      </div>

      <SelectField
        label="Minecraft channel"
        value={versionChannel}
        onChange={(e) => setVersionChannel(e.target.value as VersionChannel)}
      >
        <option value="release">Releases</option>
        <option value="snapshot">Snapshots</option>
        <option value="prerelease">Pre-releases &amp; RCs</option>
        <option value="legacy">Legacy (Alpha / Beta)</option>
        <option value="all">All</option>
      </SelectField>
      <SelectField
        label="Minecraft version"
        value={gameVersion}
        onChange={(e) => setGameVersion(e.target.value)}
      >
        {visibleVersions.map((v) => (
          <option key={v.id} value={v.id}>
            {versionLabel(v)}
          </option>
        ))}
      </SelectField>
      {loader !== "vanilla" ? (
        <SelectField
          label={`${loaderLabel(loader)} version`}
          value={loaderVersion}
          onChange={(e) => setLoaderVersion(e.target.value)}
        >
          {loaders.length === 0 ? (
            <option value="">No builds for this Minecraft version</option>
          ) : (
            loaders.map((l) => (
              <option key={l.version} value={l.version}>
                {l.version}
                {l.stable ? " (stable)" : ""}
              </option>
            ))
          )}
        </SelectField>
      ) : (
        <Callout tone="info">
          Vanilla cannot load mods. Pick Fabric, Quilt, Forge, or NeoForge if you want a modded
          instance. {loaderHint("vanilla")}
        </Callout>
      )}

      <p className="instance-summary" aria-live="polite">
        This instance: <strong>{summary}</strong>
      </p>

      {error ? <Callout>{error}</Callout> : null}
      <div className="row">
        <Button
          onClick={() => void create()}
          disabled={busy || !name.trim() || (loader !== "vanilla" && !loaderVersion)}
        >
          Create
        </Button>
        <Button variant="text" onClick={onClose}>
          Cancel
        </Button>
      </div>
      <div className="stack">
        <span className="muted">Or start from a template</span>
        <div className="row" style={{ flexWrap: "wrap" }}>
          {templates.map((t) => (
            <Button
              key={t.id}
              variant="tonal"
              small
              disabled={busy}
              onClick={() => void fromTemplate(t.id)}
              title={t.description}
            >
              {t.name}
            </Button>
          ))}
        </div>
      </div>
    </Dialog>
  );
}
