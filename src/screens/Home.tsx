import { FileArchive, Plus, Square } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import type { InstanceStatus } from "../api/types";
import { CreateInstance } from "../components/CreateInstance";
import { InstanceWorkspace } from "../components/InstanceWorkspace";
import { Button, Callout } from "../components/ui";
import { useAppStore } from "../store/app";

export function Home() {
  const {
    instances,
    selectedId,
    search,
    setSelectedId,
    setSearch,
    setRoute,
    setWorkspaceTab,
    installProgress,
    setInstances,
    activeProfile,
    running,
    setRunning,
  } = useAppStore();
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<InstanceStatus | null>(null);
  const [packMsg, setPackMsg] = useState<string | null>(null);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return instances;
    return instances.filter(
      (i) =>
        i.name.toLowerCase().includes(q) ||
        i.loader.toLowerCase().includes(q) ||
        i.gameVersion.toLowerCase().includes(q),
    );
  }, [instances, search]);

  const selected = instances.find((i) => i.id === selectedId) ?? null;

  useEffect(() => {
    if (!selectedId) {
      setStatus(null);
      return;
    }
    void api.instanceStatus(selectedId).then(setStatus).catch(() => setStatus(null));
  }, [selectedId, installProgress]);

  async function refresh() {
    setInstances(await api.listInstances());
  }

  async function play(id: string) {
    setBusy(id);
    setError(null);
    try {
      await api.launchInstance(id);
      setRunning(id, true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }

  async function stop(id: string) {
    setBusy(id);
    setError(null);
    try {
      await api.stopInstance(id);
      setRunning(id, false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      try {
        const ids = await api.listRunningInstances();
        setRunning(id, ids.includes(id));
      } catch {
        setRunning(id, false);
      }
    } finally {
      setBusy(null);
    }
  }

  async function install(id: string) {
    setBusy(id);
    setError(null);
    try {
      await api.installInstance(id);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }

  async function importMrpack() {
    setPackMsg(null);
    setError(null);
    try {
      const path = await api.pickMrpackFile();
      if (!path) return;
      const result = await api.importMrpack({ path });
      setPackMsg(
        `Imported ${result.filesInstalled} file${result.filesInstalled === 1 ? "" : "s"} as ${result.instance.name}.`,
      );
      await refresh();
      setSelectedId(result.instance.id);
      setWorkspaceTab("mods");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <>
      <div className="topbar">
        <h1>Instances</h1>
        <input
          className="search"
          placeholder="Search instances"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <Button onClick={() => setCreating(true)}>
          <Plus size={16} /> New
        </Button>
        <Button variant="outline" onClick={() => void importMrpack()}>
          <FileArchive size={16} /> Import .mrpack
        </Button>
        <button className="account-chip" onClick={() => setRoute("accounts")}>
          <span className="avatar">
            {(activeProfile?.displayName ?? "A").slice(0, 1).toUpperCase()}
          </span>
          <span>{activeProfile?.displayName ?? "Sign in"}</span>
        </button>
      </div>
      <div className="content">
        {error ? <Callout>{error}</Callout> : null}
        {packMsg ? <Callout tone="info">{packMsg}</Callout> : null}
        {filtered.length === 0 ? (
          <div className="empty">
            <p>No instances yet. Create one or import a Modrinth .mrpack.</p>
            <div className="row">
              <Button onClick={() => setCreating(true)}>Create instance</Button>
              <Button variant="outline" onClick={() => void importMrpack()}>
                Import .mrpack
              </Button>
            </div>
          </div>
        ) : (
          <div className="card-grid">
            {filtered.map((inst) => {
              const isRunning = !!running[inst.id];
              return (
                <button
                  key={inst.id}
                  className={`instance-card ${selectedId === inst.id ? "selected" : ""}`}
                  onClick={() => setSelectedId(inst.id)}
                >
                  <div className="row">
                    <span className="pill">{inst.loader}</span>
                    <span className="muted">{inst.gameVersion}</span>
                    {isRunning ? <span className="pill pill-match">Running</span> : null}
                  </div>
                  <h3>{inst.name}</h3>
                  <span className="muted">
                    {inst.lastPlayed
                      ? `Last played ${new Date(inst.lastPlayed).toLocaleString()}`
                      : "Never launched"}
                  </span>
                  {isRunning ? (
                    <Button
                      small
                      variant="danger"
                      disabled={busy === inst.id}
                      onClick={(e) => {
                        e.stopPropagation();
                        void stop(inst.id);
                      }}
                    >
                      <Square size={14} aria-hidden /> Stop
                    </Button>
                  ) : (
                    <Button
                      small
                      disabled={busy === inst.id}
                      onClick={(e) => {
                        e.stopPropagation();
                        void play(inst.id);
                      }}
                    >
                      Play
                    </Button>
                  )}
                </button>
              );
            })}
          </div>
        )}
      </div>
      {selected ? (
        <InstanceWorkspace
          instance={selected}
          status={status}
          busy={busy === selected.id}
          onPlay={() => void play(selected.id)}
          onStop={() => void stop(selected.id)}
          onInstall={() => void install(selected.id)}
          onRefresh={() => void refresh()}
          onDeleted={() => {
            setSelectedId(null);
            void refresh();
          }}
        />
      ) : null}
      {creating ? (
        <CreateInstance
          onClose={() => setCreating(false)}
          onCreated={() => void refresh()}
        />
      ) : null}
    </>
  );
}
