import { useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import {
  instanceTargetLabel,
  loaderLabel,
  supportsGameVersion,
  supportsLoader,
} from "../api/loaders";
import type {
  CatalogVersion,
  Instance,
  ModChannel,
  ProjectDetail,
  ProjectHit,
  ProjectType,
} from "../api/types";
import { ignoresInstanceVersion } from "../api/types";
import { Button, Callout, Dialog } from "./ui";
import { MarkdownBody } from "./MarkdownBody";

function categoryLabel(name: string) {
  return name
    .split(/[-_]/)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}

export function ProjectDetailPanel({
  hit,
  targetInstance,
  channel,
  onChannelChange,
  onInstall,
  onClose,
}: {
  hit: ProjectHit;
  targetInstance: Instance | null;
  channel: ModChannel;
  onChannelChange: (c: ModChannel) => void;
  onInstall: (versionId: string | null) => void;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [versions, setVersions] = useState<CatalogVersion[]>([]);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [galleryIndex, setGalleryIndex] = useState(0);
  const projectType = (hit.projectType || "mod") as ProjectType;
  const skipVersion = ignoresInstanceVersion(projectType);

  useEffect(() => {
    let cancelled = false;
    setBusy(true);
    setError(null);
    const loaders =
      skipVersion || projectType === "modpack" || !targetInstance || targetInstance.loader === "vanilla"
        ? undefined
        : [targetInstance.loader];
    const games =
      skipVersion || projectType === "modpack" || !targetInstance?.gameVersion
        ? undefined
        : [targetInstance.gameVersion];
    void Promise.all([
      api.getCatalogProject(hit.id),
      api.listCatalogVersions(hit.id, loaders, games, channel),
    ])
      .then(([d, vs]) => {
        if (cancelled) return;
        setDetail(d);
        setVersions(vs);
        setGalleryIndex(0);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
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
    skipVersion,
    projectType,
    targetInstance?.id,
    targetInstance?.loader,
    targetInstance?.gameVersion,
  ]);

  const featured = useMemo(() => {
    if (!detail?.gallery.length) return null;
    return (
      detail.gallery.find((g) => g.featured) ??
      detail.gallery[galleryIndex] ??
      detail.gallery[0]
    );
  }, [detail, galleryIndex]);

  const installLabel =
    projectType === "modpack"
      ? "Install as new instance"
      : projectType === "mod"
        ? "Install to instance"
        : `Install ${projectType}`;

  return (
    <Dialog title={detail?.title ?? hit.title} onClose={onClose}>
      <div className="project-detail stack">
        {error ? <Callout>{error}</Callout> : null}
        {busy && !detail ? <p className="muted">Loading project…</p> : null}

        {detail ? (
          <>
            <div className="project-detail-hero row">
              {detail.iconUrl ? (
                <img className="project-icon" src={detail.iconUrl} alt="" width={72} height={72} />
              ) : (
                <div className="project-icon placeholder" aria-hidden />
              )}
              <div className="stack" style={{ gap: 6, flex: 1 }}>
                <p className="muted" style={{ margin: 0 }}>
                  {detail.description}
                </p>
                <div className="row">
                  <span className="pill">{detail.projectType}</span>
                  <span className="muted">{detail.downloads.toLocaleString()} downloads</span>
                  {detail.license ? <span className="pill">{detail.license}</span> : null}
                </div>
                {targetInstance && !skipVersion && projectType !== "modpack" ? (
                  <p className="muted" style={{ margin: 0 }}>
                    Target: <strong>{instanceTargetLabel(targetInstance)}</strong>
                  </p>
                ) : null}
                {skipVersion ? (
                  <p className="muted" style={{ margin: 0 }}>
                    Resource/shader installs ignore Minecraft version filters.
                  </p>
                ) : null}
              </div>
            </div>

            {featured ? (
              <div className="project-gallery">
                <img src={featured.url} alt={featured.title ?? "Screenshot"} />
                {detail.gallery.length > 1 ? (
                  <div className="row">
                    {detail.gallery.slice(0, 8).map((g, i) => (
                      <button
                        key={g.url}
                        type="button"
                        className={`pill pill-btn ${i === galleryIndex ? "active" : ""}`}
                        onClick={() => setGalleryIndex(i)}
                      >
                        {i + 1}
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            ) : null}

            <div className="row">
              {detail.categories.slice(0, 12).map((c) => (
                <span key={c} className="pill">
                  {categoryLabel(c)}
                </span>
              ))}
              {detail.loaders.slice(0, 6).map((l) => (
                <span
                  key={l}
                  className={`pill ${
                    targetInstance && !skipVersion && supportsLoader([l], targetInstance.loader)
                      ? "pill-match"
                      : ""
                  }`}
                >
                  {loaderLabel(l)}
                </span>
              ))}
              {!skipVersion
                ? detail.gameVersions.slice(0, 6).map((v) => (
                    <span
                      key={v}
                      className={`pill ${
                        targetInstance && supportsGameVersion([v], targetInstance.gameVersion)
                          ? "pill-match"
                          : ""
                      }`}
                    >
                      {v}
                    </span>
                  ))
                : null}
            </div>

            {detail.members.length ? (
              <div className="row">
                <span className="muted">Team:</span>
                {detail.members.map((m) => (
                  <span key={m.userId} className="pill">
                    {m.name}
                    {m.role ? ` · ${m.role}` : ""}
                  </span>
                ))}
              </div>
            ) : null}

            <div className="row">
              <Button
                variant="outline"
                small
                onClick={() => void api.openExternal(detail.projectUrl)}
              >
                Modrinth
              </Button>
              {detail.sourceUrl ? (
                <Button
                  variant="text"
                  small
                  onClick={() => void api.openExternal(detail.sourceUrl!)}
                >
                  Source
                </Button>
              ) : null}
              {detail.issuesUrl ? (
                <Button
                  variant="text"
                  small
                  onClick={() => void api.openExternal(detail.issuesUrl!)}
                >
                  Issues
                </Button>
              ) : null}
              {detail.wikiUrl ? (
                <Button
                  variant="text"
                  small
                  onClick={() => void api.openExternal(detail.wikiUrl!)}
                >
                  Wiki
                </Button>
              ) : null}
              {detail.discordUrl ? (
                <Button
                  variant="text"
                  small
                  onClick={() => void api.openExternal(detail.discordUrl!)}
                >
                  Discord
                </Button>
              ) : null}
              {detail.donationUrls.map((d) => (
                <Button
                  key={d.id}
                  variant="text"
                  small
                  onClick={() => void api.openExternal(d.url)}
                >
                  {d.platform}
                </Button>
              ))}
            </div>

            {detail.body ? <MarkdownBody source={detail.body} /> : null}

            <label className="field">
              <span>Channel</span>
              <select
                value={channel}
                onChange={(e) => onChannelChange(e.target.value as ModChannel)}
              >
                <option value="stable">Stable</option>
                <option value="beta">Stable + beta</option>
                <option value="all">All channels</option>
              </select>
            </label>

            <div className="stack" style={{ gap: 8 }}>
              <strong>Versions</strong>
              {versions.length === 0 ? (
                <Callout tone="info">
                  {skipVersion
                    ? "No versions listed for this channel."
                    : "No versions matched the instance filters. Open the version picker to browse all."}
                </Callout>
              ) : (
                <ul className="version-list">
                  {versions.slice(0, 12).map((v) => (
                    <li key={v.id} className="row" style={{ justifyContent: "space-between" }}>
                      <span>
                        {v.versionNumber || v.name} · {v.channel}
                        {!skipVersion && v.gameVersions.length
                          ? ` · MC ${v.gameVersions.slice(0, 3).join(", ")}`
                          : ""}
                        {!skipVersion && v.loaders.length
                          ? ` · ${v.loaders.map(loaderLabel).join(", ")}`
                          : ""}
                      </span>
                      <Button small onClick={() => onInstall(v.id)}>
                        Install
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            <div className="row">
              <Button
                disabled={projectType !== "modpack" && !targetInstance}
                onClick={() => onInstall(versions[0]?.id ?? null)}
              >
                {installLabel}
              </Button>
              <Button variant="tonal" onClick={() => onInstall(null)}>
                Latest matching
              </Button>
              <Button variant="text" onClick={onClose}>
                Close
              </Button>
            </div>
          </>
        ) : null}
      </div>
    </Dialog>
  );
}
