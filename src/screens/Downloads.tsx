import { useMemo } from "react";
import { api } from "../api/client";
import { Button } from "../components/ui";
import { useAppStore } from "../store/app";

function pct(progress: number) {
  return Math.round(Math.min(1, Math.max(0, progress)) * 100);
}

export function Downloads() {
  const {
    instances,
    installProgress,
    downloadHistory,
    running,
    setSelectedId,
    setRoute,
    setRunning,
  } = useAppStore();

  const nameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const i of instances) map.set(i.id, i.name);
    return map;
  }, [instances]);

  const active = useMemo(() => {
    return Object.entries(installProgress)
      .filter(([, p]) => p.progress < 1)
      .map(([instanceId, p]) => ({ instanceId, ...p }))
      .sort((a, b) => a.instanceId.localeCompare(b.instanceId));
  }, [installProgress]);

  const recent = useMemo(() => {
    const activeIds = new Set(active.map((a) => a.instanceId));
    return downloadHistory.filter((h) => !activeIds.has(h.instanceId) || h.progress >= 1);
  }, [downloadHistory, active]);

  const runningIds = useMemo(
    () => Object.entries(running).filter(([, on]) => on).map(([id]) => id),
    [running],
  );

  const empty = active.length === 0 && recent.length === 0 && runningIds.length === 0;

  function openInstance(id: string) {
    setSelectedId(id);
    setRoute("home");
  }

  return (
    <>
      <div className="topbar">
        <h1>Downloads</h1>
      </div>
      <div className="content stack">
        {empty ? (
          <div className="empty">
            <p>Nothing downloading right now.</p>
            <p className="muted" style={{ margin: 0 }}>
              Instance installs and updates will show up here while they run.
            </p>
          </div>
        ) : null}

        {active.length > 0 ? (
          <section className="settings-section stack">
            <h3 style={{ margin: 0 }}>In progress</h3>
            <ul className="download-list">
              {active.map((item) => (
                <li key={item.instanceId} className="download-row">
                  <div className="download-row-head">
                    <button
                      type="button"
                      className="linkish"
                      onClick={() => openInstance(item.instanceId)}
                    >
                      {nameById.get(item.instanceId) ?? item.instanceId}
                    </button>
                    <span className="muted">{pct(item.progress)}%</span>
                  </div>
                  <span className="muted">{item.message}</span>
                  <div className="progress" aria-hidden>
                    <span style={{ width: `${pct(item.progress)}%` }} />
                  </div>
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        {runningIds.length > 0 ? (
          <section className="settings-section stack">
            <h3 style={{ margin: 0 }}>Running</h3>
            <p className="muted" style={{ margin: 0 }}>
              These instances have a live game process. Launch output stays on the
              Home inspector.
            </p>
            <ul className="download-list">
              {runningIds.map((id) => (
                <li key={id} className="download-row">
                  <div className="download-row-head">
                    <button type="button" className="linkish" onClick={() => openInstance(id)}>
                      {nameById.get(id) ?? id}
                    </button>
                    <span className="pill">Running</span>
                    <Button
                      small
                      variant="danger"
                      onClick={() =>
                        void api
                          .stopInstance(id)
                          .then(() => setRunning(id, false))
                          .catch(() => setRunning(id, false))
                      }
                    >
                      Stop
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        {recent.length > 0 ? (
          <section className="settings-section stack">
            <h3 style={{ margin: 0 }}>Recent</h3>
            <ul className="download-list">
              {recent.map((item) => (
                <li key={`${item.instanceId}-${item.updatedAt}`} className="download-row">
                  <div className="download-row-head">
                    <button
                      type="button"
                      className="linkish"
                      onClick={() => openInstance(item.instanceId)}
                    >
                      {nameById.get(item.instanceId) ?? item.instanceId}
                    </button>
                    <span className="muted">
                      {item.progress >= 1 ? "Done" : `${pct(item.progress)}%`}
                    </span>
                  </div>
                  <span className="muted">{item.message}</span>
                  {item.progress < 1 ? (
                    <div className="progress" aria-hidden>
                      <span style={{ width: `${pct(item.progress)}%` }} />
                    </div>
                  ) : null}
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </div>
    </>
  );
}
