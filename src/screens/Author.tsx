import { useEffect, useState } from "react";
import { api } from "../api/client";
import type {
  AuthorProject,
  AuthorStatus,
  PublishChecklistItem,
  RemoteGalleryImage,
  RemoteModrinthProject,
} from "../api/types";
import { Button, Callout, SelectField, TextField } from "../components/ui";
import { useAppStore } from "../store/app";

const TYPES = [
  { id: "mod", label: "Mod" },
  { id: "modpack", label: "Modpack" },
  { id: "resourcepack", label: "Resource pack" },
  { id: "shader", label: "Shader" },
  { id: "datapack", label: "Datapack" },
];

export function Author() {
  const [status, setStatus] = useState<AuthorStatus | null>(null);
  const [projects, setProjects] = useState<AuthorProject[]>([]);
  const [remote, setRemote] = useState<RemoteModrinthProject[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [checklist, setChecklist] = useState<PublishChecklistItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pat, setPat] = useState("");
  const [showPat, setShowPat] = useState(false);
  const [versionNumber, setVersionNumber] = useState("0.1.0");
  const [versionName, setVersionName] = useState("");
  const [changelog, setChangelog] = useState("");
  const [gameVersions, setGameVersions] = useState("1.21.1");
  const [loaders, setLoaders] = useState("fabric");
  const [filePath, setFilePath] = useState<string | null>(null);
  const [publishMsg, setPublishMsg] = useState<string | null>(null);
  const [galleryTitle, setGalleryTitle] = useState("");
  const [gallery, setGallery] = useState<RemoteGalleryImage[]>([]);
  const selected = projects.find((p) => p.id === selectedId) ?? null;
  const notifyModrinthAuthError = useAppStore((s) => s.notifyModrinthAuthError);
  const setModrinthReconnectPrompt = useAppStore((s) => s.setModrinthReconnectPrompt);

  async function noteAuthError(e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    setError(msg);
    notifyModrinthAuthError(msg);
    if (/401|expired|Reconnect/i.test(msg)) {
      try {
        setStatus(await api.authorStatus());
      } catch {
        /* ignore */
      }
    }
  }

  async function refresh() {
    const [s, list] = await Promise.all([api.authorStatus(), api.listAuthorProjects()]);
    setStatus(s);
    setProjects(list);
    if (s.expired) {
      setModrinthReconnectPrompt(true);
    } else if (s.connected) {
      setModrinthReconnectPrompt(false);
    }
    if (selectedId && !list.some((p) => p.id === selectedId)) {
      setSelectedId(list[0]?.id ?? null);
    } else if (!selectedId && list[0]) {
      setSelectedId(list[0].id);
    }
    if (s.connected) {
      try {
        setRemote(await api.listMyModrinthProjects());
      } catch (e) {
        await noteAuthError(e);
        setRemote([]);
      }
    } else {
      setRemote([]);
    }
  }

  async function refreshGallery(projectId: string) {
    try {
      setGallery(await api.listAuthorGallery(projectId));
    } catch (e) {
      setGallery([]);
      await noteAuthError(e);
    }
  }

  useEffect(() => {
    void refresh().catch((e) => setError(e instanceof Error ? e.message : String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!selectedId) {
      setChecklist([]);
      return;
    }
    void api
      .authorPublishChecklist(selectedId)
      .then(setChecklist)
      .catch(() => setChecklist([]));
  }, [selectedId, selected?.updatedAt, status?.connected]);

  useEffect(() => {
    if (!selected?.modrinthId || !status?.connected) {
      setGallery([]);
      return;
    }
    void refreshGallery(selected.modrinthId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected?.modrinthId, status?.connected]);

  async function createDraft() {
    setBusy(true);
    setError(null);
    try {
      const p = await api.createAuthorProject({
        title: "Untitled project",
        projectType: "mod",
        summary: "",
        description: "",
      });
      await refresh();
      setSelectedId(p.id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function savePatch(patch: Partial<AuthorProject>) {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await api.updateAuthorProject(selected.id, {
        title: patch.title,
        slug: patch.slug,
        summary: patch.summary,
        description: patch.description,
        projectType: patch.projectType,
        status: patch.status,
        modrinthId: patch.modrinthId,
      });
      setProjects((prev) => prev.map((p) => (p.id === updated.id ? updated : p)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function removeSelected() {
    if (!selected) return;
    if (!confirm(`Delete draft “${selected.title}”?`)) return;
    setBusy(true);
    try {
      await api.deleteAuthorProject(selected.id);
      setSelectedId(null);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function connectOAuth() {
    setBusy(true);
    setError(null);
    try {
      const s = await api.startModrinthLogin();
      setStatus(s);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function connectPat() {
    setBusy(true);
    setError(null);
    try {
      const s = await api.connectModrinthPat(pat);
      setStatus(s);
      setPat("");
      setShowPat(false);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function disconnect() {
    setBusy(true);
    try {
      setStatus(await api.disconnectModrinth());
      setRemote([]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function linkRemote(remoteId: string) {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await api.linkAuthorDraft(selected.id, remoteId);
      setProjects((prev) => prev.map((p) => (p.id === updated.id ? updated : p)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function importRemote(remoteId: string) {
    setBusy(true);
    setError(null);
    try {
      const p = await api.importModrinthProject(remoteId);
      await refresh();
      setSelectedId(p.id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function createOnModrinth() {
    if (!selected) return;
    if (!selected.slug || selected.slug.trim().length < 3) {
      setError("Set a slug (3+ characters) before creating on Modrinth.");
      return;
    }
    if (!confirm(`Create Modrinth draft project “${selected.title}” (${selected.slug})?`)) {
      return;
    }
    setBusy(true);
    setError(null);
    setPublishMsg(null);
    try {
      const updated = await api.createModrinthProject({ draftId: selected.id });
      setProjects((prev) => prev.map((p) => (p.id === updated.id ? updated : p)));
      setPublishMsg(
        `Created on Modrinth as ${updated.modrinthId}. You can upload a version next.`,
      );
      await refresh();
    } catch (e) {
      await noteAuthError(e);
    } finally {
      setBusy(false);
    }
  }

  async function uploadIcon() {
    if (!selected?.modrinthId) return;
    setBusy(true);
    setError(null);
    setPublishMsg(null);
    try {
      const path = await api.pickAuthorImage();
      if (!path) return;
      const res = await api.uploadAuthorIcon({
        projectId: selected.modrinthId,
        filePath: path,
      });
      setPublishMsg(res.note);
    } catch (e) {
      await noteAuthError(e);
    } finally {
      setBusy(false);
    }
  }

  async function uploadGallery() {
    if (!selected?.modrinthId) return;
    setBusy(true);
    setError(null);
    setPublishMsg(null);
    try {
      const path = await api.pickAuthorImage();
      if (!path) return;
      const res = await api.uploadAuthorGallery({
        projectId: selected.modrinthId,
        filePath: path,
        featured: false,
        title: galleryTitle.trim() || undefined,
      });
      setPublishMsg(res.note);
      await refreshGallery(selected.modrinthId);
    } catch (e) {
      await noteAuthError(e);
    } finally {
      setBusy(false);
    }
  }

  async function pickFile() {
    const path = await api.pickPublishFile();
    if (path) setFilePath(path);
  }

  async function publishVersion() {
    if (!selected?.modrinthId) {
      setError("Link this draft to a Modrinth project before publishing a version.");
      return;
    }
    if (!filePath) {
      setError("Choose a .jar / .zip / .mrpack file to upload.");
      return;
    }
    setBusy(true);
    setError(null);
    setPublishMsg(null);
    try {
      const result = await api.publishAuthorVersion({
        projectId: selected.modrinthId,
        draftId: selected.id,
        name: versionName || versionNumber,
        versionNumber,
        changelog,
        gameVersions: gameVersions
          .split(/[,\s]+/)
          .map((s) => s.trim())
          .filter(Boolean),
        loaders: loaders
          .split(/[,\s]+/)
          .map((s) => s.trim())
          .filter(Boolean),
        versionType: "release",
        filePath,
      });
      setPublishMsg(`Published ${result.versionNumber} → ${result.projectUrl}`);
      await refresh();
    } catch (e) {
      await noteAuthError(e);
    } finally {
      setBusy(false);
    }
  }

  function openOnModrinth() {
    if (!selected) return;
    const slug = selected.slug || selected.modrinthId;
    if (slug) {
      void api.openExternal(`https://modrinth.com/${selected.projectType}/${slug}`);
    } else {
      void api.openExternal("https://modrinth.com/dashboard");
    }
  }

  const canOAuth = !!(status?.oauthConfigured && status?.secretConfigured) || !!status?.dryRun;

  return (
    <>
      <div className="topbar">
        <h1>Author</h1>
        <Button small onClick={() => void createDraft()} disabled={busy}>
          New draft
        </Button>
      </div>
      <div className="content stack">
        {status?.expired ? (
          <Callout tone="warn">
            {status.note}{" "}
            <Button small disabled={busy || !canOAuth} onClick={() => void connectOAuth()}>
              Reconnect
            </Button>
          </Callout>
        ) : status ? (
          <Callout tone="info">{status.note}</Callout>
        ) : null}
        {status?.redirectUri ? (
          <p className="muted" style={{ margin: 0 }}>
            OAuth redirect allowlist: <code>{status.redirectUri}</code>
            {status.scopes ? (
              <>
                {" "}
                · scopes <code>{status.scopes}</code>
              </>
            ) : null}
          </p>
        ) : null}
        {error ? <Callout>{error}</Callout> : null}
        {publishMsg ? <Callout tone="info">{publishMsg}</Callout> : null}

        <div className="author-layout">
          <aside className="author-list stack">
            <strong>Local drafts</strong>
            {projects.length === 0 ? (
              <p className="muted">No local drafts yet.</p>
            ) : (
              projects.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  className={`author-list-item ${p.id === selectedId ? "active" : ""}`}
                  onClick={() => setSelectedId(p.id)}
                >
                  <span>{p.title}</span>
                  <span className="muted">
                    {p.projectType} · {p.status}
                    {p.modrinthId ? " · linked" : ""}
                  </span>
                </button>
              ))
            )}

            <strong style={{ marginTop: 8 }}>Modrinth account</strong>
            {status?.expired ? (
              <>
                <p className="muted" style={{ margin: 0 }}>
                  Session expired
                  {status.username ? (
                    <>
                      {" "}
                      for <strong>{status.username}</strong>
                    </>
                  ) : null}
                  .
                </p>
                <Button small disabled={busy || !canOAuth} onClick={() => void connectOAuth()}>
                  Reconnect Modrinth
                </Button>
                <Button
                  variant="text"
                  small
                  disabled={busy}
                  onClick={() => setShowPat((v) => !v)}
                >
                  {showPat ? "Hide PAT" : "Or use a personal access token"}
                </Button>
                {showPat ? (
                  <div className="stack" style={{ gap: 8 }}>
                    <TextField
                      label="PAT"
                      type="password"
                      value={pat}
                      onChange={(e) => setPat(e.target.value)}
                      placeholder="mrp_…"
                    />
                    <Button small disabled={busy || !pat.trim()} onClick={() => void connectPat()}>
                      Save PAT in keychain
                    </Button>
                  </div>
                ) : null}
              </>
            ) : status?.connected ? (
              <>
                <p className="muted" style={{ margin: 0 }}>
                  Signed in as <strong>{status.username ?? "creator"}</strong>
                  {status.dryRun ? " (dry-run)" : ""}
                  {status.expiresAt ? (
                    <>
                      {" "}
                      · expires {new Date(status.expiresAt).toLocaleString()}
                    </>
                  ) : null}
                </p>
                <Button variant="outline" small disabled={busy} onClick={() => void disconnect()}>
                  Disconnect
                </Button>
              </>
            ) : (
              <>
                <Button
                  small
                  disabled={busy || !canOAuth}
                  title={
                    canOAuth
                      ? "Open browser to authorize Aureum"
                      : "Set MODRINTH_CLIENT_ID + MODRINTH_CLIENT_SECRET"
                  }
                  onClick={() => void connectOAuth()}
                >
                  Connect Modrinth
                </Button>
                <Button
                  variant="text"
                  small
                  disabled={busy}
                  onClick={() => setShowPat((v) => !v)}
                >
                  {showPat ? "Hide PAT" : "Use personal access token"}
                </Button>
                {showPat ? (
                  <div className="stack" style={{ gap: 8 }}>
                    <TextField
                      label="PAT"
                      type="password"
                      value={pat}
                      onChange={(e) => setPat(e.target.value)}
                      placeholder="mrp_…"
                    />
                    <Button small disabled={busy || !pat.trim()} onClick={() => void connectPat()}>
                      Save PAT in keychain
                    </Button>
                  </div>
                ) : null}
              </>
            )}

            {status?.connected && remote.length ? (
              <>
                <strong style={{ marginTop: 8 }}>My Modrinth projects</strong>
                {remote.map((r) => (
                  <div key={r.id} className="stack" style={{ gap: 4 }}>
                    <span>
                      {r.title}{" "}
                      <span className="muted">
                        · {r.projectType}/{r.slug}
                      </span>
                    </span>
                    <div className="row">
                      <Button
                        small
                        variant="tonal"
                        disabled={busy || !selected}
                        onClick={() => void linkRemote(r.id)}
                      >
                        Link to draft
                      </Button>
                      <Button
                        small
                        variant="outline"
                        disabled={busy}
                        onClick={() => void importRemote(r.id)}
                      >
                        Import draft
                      </Button>
                    </div>
                  </div>
                ))}
              </>
            ) : null}
          </aside>

          <section className="author-editor stack">
            {!selected ? (
              <div className="empty-mods">
                <h3>Creator drafts</h3>
                <p className="muted">
                  Connect Modrinth to list your projects, link a draft, then publish a version with a
                  local jar/zip.
                </p>
                <Button onClick={() => void createDraft()}>Create draft</Button>
              </div>
            ) : (
              <>
                <TextField
                  label="Title"
                  value={selected.title}
                  onChange={(e) =>
                    setProjects((prev) =>
                      prev.map((p) =>
                        p.id === selected.id ? { ...p, title: e.target.value } : p,
                      ),
                    )
                  }
                  onBlur={() => void savePatch({ title: selected.title })}
                />
                <TextField
                  label="Slug"
                  value={selected.slug ?? ""}
                  placeholder="url-friendly-name"
                  onChange={(e) =>
                    setProjects((prev) =>
                      prev.map((p) =>
                        p.id === selected.id ? { ...p, slug: e.target.value } : p,
                      ),
                    )
                  }
                  onBlur={() => void savePatch({ slug: selected.slug })}
                />
                <SelectField
                  label="Type"
                  value={selected.projectType}
                  onChange={(e) => {
                    const projectType = e.target.value;
                    setProjects((prev) =>
                      prev.map((p) => (p.id === selected.id ? { ...p, projectType } : p)),
                    );
                    void savePatch({ projectType });
                  }}
                >
                  {TYPES.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.label}
                    </option>
                  ))}
                </SelectField>
                <label className="field">
                  <span>Summary</span>
                  <textarea
                    rows={3}
                    value={selected.summary}
                    onChange={(e) =>
                      setProjects((prev) =>
                        prev.map((p) =>
                          p.id === selected.id ? { ...p, summary: e.target.value } : p,
                        ),
                      )
                    }
                    onBlur={() => void savePatch({ summary: selected.summary })}
                  />
                </label>
                <label className="field">
                  <span>Description</span>
                  <textarea
                    rows={8}
                    value={selected.description}
                    onChange={(e) =>
                      setProjects((prev) =>
                        prev.map((p) =>
                          p.id === selected.id ? { ...p, description: e.target.value } : p,
                        ),
                      )
                    }
                    onBlur={() => void savePatch({ description: selected.description })}
                  />
                </label>
                <p className="muted" style={{ margin: 0 }}>
                  Modrinth id: {selected.modrinthId ?? "not linked"}
                </p>
                {!selected.modrinthId ? (
                  <Button
                    disabled={busy || !status?.connected}
                    onClick={() => void createOnModrinth()}
                  >
                    Create project on Modrinth
                  </Button>
                ) : (
                  <div className="stack" style={{ gap: 8 }}>
                    <strong>Project media</strong>
                    <div className="row">
                      <Button
                        small
                        variant="outline"
                        disabled={busy || !status?.connected}
                        onClick={() => void uploadIcon()}
                      >
                        Upload icon
                      </Button>
                      <Button
                        small
                        variant="outline"
                        disabled={busy || !status?.connected}
                        onClick={() => void uploadGallery()}
                      >
                        Add gallery image
                      </Button>
                    </div>
                    <TextField
                      label="Gallery title (optional)"
                      value={galleryTitle}
                      onChange={(e) => setGalleryTitle(e.target.value)}
                    />
                    <p className="muted" style={{ margin: 0 }}>
                      Icon max 256 KiB · gallery max 5 MiB (Modrinth limits).
                    </p>
                    {gallery.length ? (
                      <ul className="checklist">
                        {gallery.map((g) => (
                          <li key={g.url} className={g.featured ? "done" : ""}>
                            <div className="row" style={{ alignItems: "center", gap: 8 }}>
                              <img
                                src={g.url}
                                alt={g.title ?? "Gallery"}
                                style={{
                                  width: 48,
                                  height: 48,
                                  objectFit: "cover",
                                  borderRadius: 8,
                                }}
                              />
                              <span style={{ flex: 1 }}>
                                {g.title || "Untitled"}
                                {g.featured ? " · featured" : ""}
                              </span>
                              {!g.featured ? (
                                <Button
                                  small
                                  variant="outline"
                                  disabled={busy}
                                  onClick={() =>
                                    void (async () => {
                                      setBusy(true);
                                      try {
                                        const res = await api.setAuthorGalleryFeatured({
                                          projectId: selected.modrinthId!,
                                          url: g.url,
                                          featured: true,
                                        });
                                        setPublishMsg(res.note);
                                        await refreshGallery(selected.modrinthId!);
                                      } catch (e) {
                                        await noteAuthError(e);
                                      } finally {
                                        setBusy(false);
                                      }
                                    })()
                                  }
                                >
                                  Set featured
                                </Button>
                              ) : null}
                              <Button
                                small
                                variant="danger"
                                disabled={busy}
                                onClick={() => {
                                  if (!confirm("Delete this gallery image on Modrinth?")) return;
                                  void (async () => {
                                    setBusy(true);
                                    try {
                                      const res = await api.deleteAuthorGalleryImage({
                                        projectId: selected.modrinthId!,
                                        url: g.url,
                                      });
                                      setPublishMsg(res.note);
                                      await refreshGallery(selected.modrinthId!);
                                    } catch (e) {
                                      await noteAuthError(e);
                                    } finally {
                                      setBusy(false);
                                    }
                                  })();
                                }}
                              >
                                Delete
                              </Button>
                            </div>
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <p className="muted" style={{ margin: 0 }}>
                        No gallery images yet.
                      </p>
                    )}
                  </div>
                )}

                <div className="stack" style={{ gap: 8 }}>
                  <strong>Publish version</strong>
                  <TextField
                    label="Version number"
                    value={versionNumber}
                    onChange={(e) => setVersionNumber(e.target.value)}
                  />
                  <TextField
                    label="Version name"
                    value={versionName}
                    placeholder="Same as number if empty"
                    onChange={(e) => setVersionName(e.target.value)}
                  />
                  <label className="field">
                    <span>Changelog</span>
                    <textarea
                      rows={3}
                      value={changelog}
                      onChange={(e) => setChangelog(e.target.value)}
                    />
                  </label>
                  <TextField
                    label="Game versions (comma-separated)"
                    value={gameVersions}
                    onChange={(e) => setGameVersions(e.target.value)}
                  />
                  <TextField
                    label="Loaders (comma-separated; use minecraft for resource packs)"
                    value={loaders}
                    onChange={(e) => setLoaders(e.target.value)}
                  />
                  <div className="row">
                    <Button variant="outline" small disabled={busy} onClick={() => void pickFile()}>
                      Choose file
                    </Button>
                    <span className="muted">{filePath ?? "No file selected"}</span>
                  </div>
                  <Button
                    disabled={busy || !status?.connected || !selected.modrinthId}
                    onClick={() => void publishVersion()}
                  >
                    Upload version to Modrinth
                  </Button>
                </div>

                <div className="stack" style={{ gap: 8 }}>
                  <strong>Publish checklist</strong>
                  <ul className="checklist">
                    {checklist.map((item) => (
                      <li key={item.id} className={item.done ? "done" : ""}>
                        {item.done ? "✓" : "○"} {item.label}
                      </li>
                    ))}
                  </ul>
                </div>

                <div className="row">
                  <Button
                    onClick={() => void savePatch({ status: "checklist" })}
                    disabled={busy}
                  >
                    Save &amp; mark checklist
                  </Button>
                  <Button variant="outline" onClick={openOnModrinth}>
                    Open on Modrinth
                  </Button>
                  <Button variant="danger" onClick={() => void removeSelected()} disabled={busy}>
                    Delete draft
                  </Button>
                </div>
              </>
            )}
          </section>
        </div>
      </div>
    </>
  );
}
