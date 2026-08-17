import { useState } from "react";
import { api } from "../api/client";
import { Button, Callout, TextField } from "../components/ui";
import { useAppStore } from "../store/app";
import {
  KEYS_BUY_URL,
  RANKS,
  THEME_PRESETS,
  canUsePreset,
  presetById,
  rankFromKey,
  type SupportRank,
} from "../theme/presets";

export function Shop() {
  const { supportRank, themePresetId, setSupportRank, setThemePresetId } = useAppStore();
  const [key, setKey] = useState("");
  const [note, setNote] = useState<string | null>(null);

  async function persistRank(rank: SupportRank) {
    setSupportRank(rank);
    await api.setSetting("support_rank", rank);
    const current = presetById(themePresetId);
    if (!canUsePreset(current, rank)) {
      setThemePresetId("aureum");
      await api.setSetting("theme_preset", "aureum");
    }
  }

  async function applyPreset(id: string) {
    const preset = presetById(id);
    if (!canUsePreset(preset, supportRank)) {
      setNote(`${preset.name} needs the ${preset.rank} rank. Launch and mods stay free.`);
      return;
    }
    setNote(null);
    setThemePresetId(preset.id);
    await api.setSetting("theme_preset", preset.id);
  }

  async function redeem() {
    const next = rankFromKey(key);
    if (!next) {
      setNote("Unknown key. Buy keys at azturax.github.io — local preview keys are AUREUM-SUPPORT and AUREUM-PATRON.");
      return;
    }
    await persistRank(next);
    setKey("");
    setNote(`Rank set to ${next}. Cosmetics only — no launch, login, or catalog perk.`);
  }

  return (
    <>
      <div className="topbar">
        <h1>Support shop</h1>
        <span className={`rank-badge rank-${supportRank}`}>{supportRank}</span>
      </div>
      <div className="content stack">
        <Callout tone="info">
          Only customization is sold. Play, Microsoft sign-in, installs, and
          Modrinth stay free. No ads. Keys are cosmetics only — not a Minecraft
          purchase, and not associated with Mojang or Microsoft.
        </Callout>

        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Buy keys</h3>
          <p className="muted" style={{ margin: 0 }}>
            Supporter (€2.50) and Patron (€7.50) redeem keys are sold on the
            studio site. After checkout you receive a key to paste below.
          </p>
          <div className="row">
            <Button variant="filled" onClick={() => void api.openExternal(KEYS_BUY_URL)}>
              Buy keys at azturax.github.io
            </Button>
          </div>
        </section>

        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Ranks</h3>
          <div className="rank-grid">
            {RANKS.map((r) => (
              <article
                key={r.id}
                className={`theme-card ${supportRank === r.id ? "selected" : ""}`}
              >
                <div className="row">
                  <strong>{r.label}</strong>
                  <span className="pill">{r.price}</span>
                </div>
                <p className="muted" style={{ margin: 0 }}>
                  {r.blurb}
                </p>
                {r.id === "free" ? (
                  <Button
                    variant="outline"
                    small
                    disabled={supportRank === "free"}
                    onClick={() => void persistRank("free")}
                  >
                    Use free
                  </Button>
                ) : (
                  <span className="muted">Redeem a key to unlock.</span>
                )}
              </article>
            ))}
          </div>
          <TextField
            label="Support key"
            placeholder="AUREUM-SUPPORT"
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
          <div className="row">
            <Button variant="tonal" onClick={() => void redeem()} disabled={!key.trim()}>
              Redeem
            </Button>
          </div>
          {note ? <p className="muted">{note}</p> : null}
        </section>

        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Theme shop</h3>
          <p className="muted">
            Accents seed the Material 3 palette. Locked swatches need a rank —
            they never gate the game.
          </p>
          <div className="theme-grid">
            {THEME_PRESETS.map((preset) => {
              const locked = !canUsePreset(preset, supportRank);
              const active = themePresetId === preset.id;
              return (
                <button
                  key={preset.id}
                  type="button"
                  className={`theme-card ${active ? "selected" : ""} ${locked ? "locked" : ""}`}
                  onClick={() => void applyPreset(preset.id)}
                >
                  <div className="swatch-row" aria-hidden>
                    <span style={{ background: preset.primary }} />
                    <span style={{ background: preset.secondary }} />
                  </div>
                  <div className="row">
                    <strong>{preset.name}</strong>
                    <span className="pill">{preset.rank}</span>
                  </div>
                  <span className="muted">{preset.blurb}</span>
                  <span className="muted">
                    {active ? "In use" : locked ? "Locked" : "Apply"}
                  </span>
                </button>
              );
            })}
          </div>
        </section>
      </div>
    </>
  );
}
