# Aureum release status

**Recommendation: continue development; publish later.**  
This document is the honesty contract for `0.1.0-dev` (internal / contributor builds). Do **not** cut a public “stable” GitHub Release until the checklist below is green.

## Current channel

| Field | Value |
| --- | --- |
| Marketing name | Aureum |
| Package / Tauri / Cargo version | `0.1.0-dev` |
| Audience | Contributors and self-builders |
| Signed updater | **Stub only** (`REPLACE_WITH_MINISIGN_PUBLIC_KEY`, plugin not wired, `createUpdaterArtifacts: false`) |
| Code signing / notarization | Optional secrets in CI — **not** required for unsigned alphas later |

CI (`.github/workflows/release.yml`) can produce multi-OS artifacts when a `v*` tag is pushed. That does **not** mean the product is ready for a public beta announcement.

## What works today (from source)

- Instance create / install (vanilla + Fabric / Quilt / Forge / NeoForge paths)
- Launch + **Stop** (per-instance Java process tree)
- Microsoft auth **when** `AUREUM_MS_CLIENT_ID` is set; otherwise dry-run profiles
- Offline named profiles (LAN/dev only — no real session)
- Modrinth catalog search / install / deps / lockfile (anonymous browse)
- `.mrpack` import / export
- Instance workspace: Mods, Packs, Screenshots, Logs (+ crash-reports browser), Settings
- Author drafts + Modrinth OAuth/PAT publish flows **when** client id/secret or PAT configured
- First-run Mojang/Microsoft disclaimer

## What’s stubbed or incomplete

| Area | Status |
| --- | --- |
| In-app updater | Config stub; no live auto-update |
| MS login without Azure app | Dry-run only — cannot play online |
| Modrinth Author without app/PAT | Drafts local; no real create/upload |
| Shop / support ranks | Themes/badges UX — not a payment backend |
| Signed Windows / notarized macOS | Not configured |
| Public CDN `releases.aureum.dev` | Placeholder endpoint |
| Forge full smoke | Ignored network test + optional workflow |

## How to run (dev)

```bash
npm install
copy .env.example .env
npm run tauri dev
```

See [README.md](./README.md) for Microsoft / Modrinth env vars and Forge smoke.

## Publish criteria

### Before first public **pre-release** (`v0.1.0-alpha.N`)

- [ ] README + this file still accurate
- [ ] `cargo check`, `tsc -b`, `cargo test` green on Windows
- [ ] Tag-driven `release.yml` run once; artifacts downloadable
- [ ] Release notes list known stubs (auth dry-run, unsigned, no updater)
- [ ] No secrets in the repo (`.env` gitignored; no private minisign key)

### Before **v0.1.0** (limited beta)

- [ ] Real `AUREUM_MS_CLIENT_ID` documented for maintainers (Xbox Live review as needed)
- [ ] Play / Stop / install / Modrinth mods verified on a clean machine
- [ ] SmartScreen/Gatekeeper warnings acknowledged in release notes
- [ ] Updater still optional (unsigned beta OK)

### Before **v1.0.0** (stable)

- [ ] Minisign keys generated; `tauri-plugin-updater` registered; pubkey real; `createUpdaterArtifacts: true`
- [ ] Update endpoint hosted and tested
- [ ] Windows code signing (and Apple notarization if shipping macOS)
- [ ] MS + Modrinth auth paths documented for end users (or first-run guided setup)
- [ ] No dry-run as the default “looks like signed in” experience for public builds
- [ ] Crash/report UX and major launch failures have clear recovery copy

## How to publish later (when criteria met)

Do **not** push tags or create releases without an explicit go-ahead.

```bash
# 1. Bump package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json to the release version
# 2. Update RELEASE.md / changelog notes
# 3. Commit (only when asked)
# 4. Tag and push — CI builds + attaches artifacts
git tag v0.1.0-alpha.1
git push origin v0.1.0-alpha.1
```

Or open a draft release manually after a successful `workflow_dispatch` build.

## Next development priorities

1. Real MS client onboarding path (or clearer dry-run → “cannot join online servers”)
2. Wire signed updater (or remove updater config from shipped UI/docs until ready)
3. First-run / empty-state polish for install + play failure modes
4. Optional: gzip log preview, gallery reorder
5. Cut `v0.1.0-alpha.1` only after one successful unsigned CI artifact run
