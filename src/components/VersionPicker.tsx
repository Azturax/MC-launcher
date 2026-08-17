import { useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import {
  instanceTargetLabel,
  loaderLabel,
  versionMatchesInstance,
} from "../api/loaders";
import type {
  CatalogVersion,
  Instance,
  ModChannel,
  ProjectHit,
  ProjectType,
} from "../api/types";
import { Button, Callout, Dialog, SelectField, Switch } from "./ui";
import { ignoresInstanceVersion } from "../api/types";

function formatVersionOption(
  v: CatalogVersion,
  instance: Instance | null,
  skipMatch: boolean,
): string {
  const match = skipMatch
    ? "any"
    : versionMatchesInstance(v.loaders, v.gameVersions, instance);
  const badge =
    match === "exact"
      ? " ✓ match"
      : match === "partial"
        ? " ~ partial"
        : match === "mismatch"
          ? " ✗"
          : "";
  const loaders = v.loaders.length ? v.loaders.map(loaderLabel).join(", ") : "any loader";
  const games =
    v.gameVersions.length <= 3
      ? v.gameVersions.join(", ")
      : `${v.gameVersions.slice(0, 2).join(", ")} +${v.gameVersions.length - 2}`;
  return `${v.versionNumber || v.name} · ${v.channel} · ${loaders} · MC ${games || "?"} ${badge}`;
}

export function VersionPicker({
  hit,
  projectType,
  channel,
  loaders,
  gameVersions,
  targetInstance,
  onChannelChange,
  onConfirm,
  onClose,
}: {
  hit: ProjectHit;
  projectType: ProjectType;
  channel: ModChannel;
  loaders?: string[];
  gameVersions?: string[];
  targetInstance?: Instance | null;
  onChannelChange: (channel: ModChannel) => void;
  onConfirm: (versionId: string | null) => void;
  onClose: () => void;
}) {
  const [versions, setVersions] = useState<CatalogVersion[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const skipVersion = ignoresInstanceVersion(projectType);
  const [showAll, setShowAll] = useState(skipVersion);

  useEffect(() => {
    setShowAll(skipVersion);
  }, [skipVersion, hit.id]);

  useEffect(() => {
    let cancelled = false;
    setBusy(true);
    setError(null);
    const filterLoaders = skipVersion || showAll ? undefined : loaders;
    const filterGames = skipVersion || showAll ? undefined : gameVersions;
    void api
      .listCatalogVersions(hit.id, filterLoaders, filterGames, channel)
      .then((list) => {
        if (cancelled) return;
        const ranked = [...list].sort((a, b) => {
          if (skipVersion) return 0;
          const rank = (v: CatalogVersion) => {
            const m = versionMatchesInstance(v.loaders, v.gameVersions, targetInstance);
            return m === "exact" ? 0 : m === "partial" ? 1 : m === "any" ? 2 : 3;
          };
          return rank(a) - rank(b);
        });
        setVersions(ranked);
        const preferred = skipVersion
          ? ranked[0]
          : (ranked.find(
              (v) =>
                versionMatchesInstance(v.loaders, v.gameVersions, targetInstance) === "exact",
            ) ?? ranked[0]);
        setSelected(preferred?.id ?? "");
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
        setVersions([]);
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [
    hit.id,
    channel,
    showAll,
    skipVersion,
    loaders?.join(","),
    gameVersions?.join(","),
    targetInstance?.id,
    targetInstance?.loader,
    targetInstance?.gameVersion,
  ]);

  const action =
    projectType === "modpack"
      ? "Install as new instance"
      : projectType === "mod"
        ? "Install to instance"
        : `Install ${projectType}`;

  const selectedMeta = useMemo(
    () => versions.find((v) => v.id === selected) ?? null,
    [versions, selected],
  );

  return (
    <Dialog title={`Choose version — ${hit.title}`} onClose={onClose}>
      <div className="stack">
        {projectType === "modpack" ? (
          <Callout tone="info">
            Modpacks bring their own loader and Minecraft version from the pack index. Install
            creates a <strong>new</strong> instance — it will not change{" "}
            {targetInstance ? targetInstance.name : "your current instance"}.
          </Callout>
        ) : skipVersion ? (
          <Callout tone="info">
            Resource packs and shaders are not gated by the instance Minecraft version. Pick any
            build (latest is fine).
          </Callout>
        ) : targetInstance ? (
          <p className="muted" style={{ margin: 0 }}>
            Target: <strong>{instanceTargetLabel(targetInstance)}</strong>
          </p>
        ) : (
          <Callout tone="warn">Select a target instance before installing.</Callout>
        )}

        <SelectField
          label="Channel"
          value={channel}
          onChange={(e) => onChannelChange(e.target.value as ModChannel)}
        >
          <option value="stable">Stable</option>
          <option value="beta">Stable + beta</option>
          <option value="all">All channels</option>
        </SelectField>

        {!skipVersion &&
        projectType !== "modpack" &&
        targetInstance &&
        targetInstance.loader !== "vanilla" ? (
          <Switch
            label="Show versions for other loaders / game versions"
            checked={showAll}
            onChange={setShowAll}
          />
        ) : null}

        {error ? <Callout>{error}</Callout> : null}
        {busy ? (
          <p className="muted">Loading versions…</p>
        ) : versions.length === 0 ? (
          <Callout tone="info">
            No versions matched{" "}
            {skipVersion
              ? "this channel"
              : targetInstance
                ? `${loaderLabel(targetInstance.loader)} · ${targetInstance.gameVersion}`
                : "these filters"}
            . Try another channel or show all versions.
          </Callout>
        ) : (
          <>
            <SelectField
              label="Version"
              value={selected}
              onChange={(e) => setSelected(e.target.value)}
            >
              {versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {formatVersionOption(v, targetInstance ?? null, skipVersion)}
                </option>
              ))}
            </SelectField>
            {selectedMeta ? (
              <p className="muted" style={{ margin: 0 }}>
                Supports:{" "}
                {selectedMeta.loaders.length
                  ? selectedMeta.loaders.map(loaderLabel).join(", ")
                  : "any loader"}{" "}
                · MC {selectedMeta.gameVersions.slice(0, 8).join(", ")}
                {selectedMeta.gameVersions.length > 8
                  ? ` (+${selectedMeta.gameVersions.length - 8})`
                  : ""}
              </p>
            ) : null}
          </>
        )}
        <div className="row">
          <Button disabled={busy || !selected} onClick={() => onConfirm(selected || null)}>
            {action}
          </Button>
          <Button
            variant="tonal"
            disabled={busy || versions.length === 0}
            onClick={() => onConfirm(null)}
          >
            Latest matching
          </Button>
          <Button variant="text" onClick={onClose}>
            Cancel
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
