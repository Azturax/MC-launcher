import { useState } from "react";
import { api } from "../api/client";
import { Button, Callout, TextField } from "../components/ui";
import { useAppStore } from "../store/app";
import type { AuthStatus } from "../api/types";

export function Accounts({ auth }: { auth: AuthStatus | null }) {
  const { profiles, activeProfile, setProfiles, setActiveProfile } = useAppStore();
  const [offlineName, setOfflineName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    const [list, active] = await Promise.all([api.listProfiles(), api.getActiveProfile()]);
    setProfiles(list);
    setActiveProfile(active);
  }

  async function signIn() {
    setBusy(true);
    setError(null);
    try {
      await api.startMicrosoftLogin();
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function addOffline() {
    setBusy(true);
    setError(null);
    try {
      await api.createOfflineProfile(offlineName);
      setOfflineName("");
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="topbar">
        <h1>Accounts</h1>
      </div>
      <div className="content stack">
        {auth?.dryRun ? (
          <Callout tone="info">
            Microsoft sign-in is in dry-run mode
            {auth.hasClientId ? "" : " (no AUREUM_MS_CLIENT_ID)"}. The profile
            switcher works; refresh tokens are never created, and the renderer
            never sees secrets.
          </Callout>
        ) : (
          <Callout tone="info">
            Sign-in opens the system browser (PKCE). Tokens stay in the OS
            keychain. Redirect: {auth?.redirectUri}
          </Callout>
        )}
        <div className="row">
          <Button onClick={() => void signIn()} disabled={busy}>
            Sign in with Microsoft
          </Button>
          <Button variant="text" onClick={() => void api.openExternal("https://www.minecraft.net/")}>
            minecraft.net
          </Button>
          <Button
            variant="text"
            onClick={() => void api.openExternal("https://www.minecraft.net/msaprofile")}
          >
            Account profile
          </Button>
          <Button
            variant="text"
            onClick={() => void api.openExternal("https://help.minecraft.net/")}
          >
            Help
          </Button>
        </div>
        {error ? <p className="muted">{error}</p> : null}
        <div className="stack">
          {profiles.length === 0 ? (
            <p className="muted">No profiles yet.</p>
          ) : (
            profiles.map((p) => (
              <div
                key={p.id}
                className={`profile-card ${activeProfile?.id === p.id ? "active" : ""}`}
              >
                <div className="avatar" aria-hidden>
                  {p.displayName.slice(0, 1).toUpperCase()}
                </div>
                <div className="stack" style={{ gap: 2, flex: 1 }}>
                  <strong>{p.displayName}</strong>
                  <span className="muted">
                    {p.kind}
                    {p.hasSecret ? " · keychain" : ""}
                    {p.kind === "offline" ? " · LAN/dev only, no session token" : ""}
                  </span>
                </div>
                <Button
                  variant="tonal"
                  small
                  onClick={() => void api.setActiveProfile(p.id).then(refresh)}
                >
                  Use
                </Button>
                <Button variant="text" small onClick={() => void api.deleteProfile(p.id).then(refresh)}>
                  Remove
                </Button>
              </div>
            ))
          )}
        </div>
        <div className="settings-section stack">
          <h3 style={{ margin: 0 }}>Offline named profile</h3>
          <p className="muted">
            Local display name for LAN and singleplayer. Cannot obtain a
            session token and cannot join online-mode servers.
          </p>
          <TextField
            label="Name"
            maxLength={16}
            value={offlineName}
            onChange={(e) => setOfflineName(e.target.value)}
          />
          <div>
            <Button variant="outline" disabled={busy || !offlineName.trim()} onClick={() => void addOffline()}>
              Add offline profile
            </Button>
          </div>
        </div>
      </div>
    </>
  );
}
