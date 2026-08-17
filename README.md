# Aureum

A local-first desktop instance manager for Minecraft: Java Edition.
**Speed for casual players, power for modders.**

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

**Status: `0.1.0-dev` — internal / contributor builds.** Prefer continuing development over a public release until the checklist in [RELEASE.md](./RELEASE.md) is met (signed updater stub, auth client IDs, honest release notes).

Aureum never bundles Minecraft jars or assets. It downloads official version
manifests and loader metadata on behalf of a signed-in owner. No ads, no
Bedrock, no cracked accounts, no private mod repo.

## Stack

- Tauri 2 + React 19 + TypeScript + Vite
- Rust backend (`src-tauri`) with module boundaries for later crate extraction
- SQLite via sqlx
- Material 3 token pipeline from orange `#FFA726` and gold `#FFD54F`

## Prerequisites

- Windows is the primary dev OS (macOS/Linux are structured, not smoke-tested here)
- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) 1.84+ (stable)
- WebView2 (ships with current Windows 10/11)
- A Java 17+ JDK on PATH to actually launch the game

Tauri’s Windows notes: <https://v2.tauri.app/start/prerequisites/>

## Downloads

**No public stable channel yet.** Build from source (below). When ready, binary builds will use [GitHub Releases](https://github.com/Azturax/MC-launcher/releases) via `.github/workflows/release.yml` (see [RELEASE.md](./RELEASE.md)).

| Channel | Where to find it | Notes |
| --- | --- | --- |
| **Dev** | This repo (`0.1.0-dev`) | `npm run tauri dev` / local `tauri build` |
| **Alpha** (later) | Pre-release tags `v0.x.x-alpha.N` | Unsigned OK; expect SmartScreen/Gatekeeper warnings |
| **Stable** (later) | Full Releases | Prefer after updater + signing criteria |

Until the first assets are attached, build from source. Do not treat early alphas as production-ready.

**Unsigned alphas are OK for early testing** once criteria in RELEASE.md are met. Code signing and Apple notarization are optional later — see commented secret placeholders in the release workflow (`TAURI_SIGNING_*`, `APPLE_*`).

### Publish a multi-OS release (later)

```bash
git tag v0.1.0-alpha.1
git push origin v0.1.0-alpha.1
```

Tags containing `alpha`, `beta`, or `-pre` are marked as GitHub pre-releases automatically. Use a clean semver tag (e.g. `v1.0.0`) only when RELEASE.md v1.0 criteria are green.

## Run

```bash
npm install
copy .env.example .env
npm run tauri dev
```

Frontend-only preview (in-memory mock, no install/launch):

```bash
npm run dev
```

Local release / alpha build (same Tauri pipeline; channel is decided by how you tag the GitHub Release):

```bash
npm run tauri build
```

## Microsoft sign-in

Copy `.env.example` to `.env`. Leave `AUREUM_MS_CLIENT_ID` empty for **dry-run**
mode: the profile switcher works, no refresh token is stored, and the renderer
never sees secrets.

To enable the real PKCE + Xbox + Minecraft Services chain:

1. Register an Azure public client (no secret).
2. Add redirect `http://127.0.0.1:17890/auth/callback`.
3. Request Xbox Live access (Microsoft review).
4. Set `AUREUM_MS_CLIENT_ID` in `.env`.

Offline named profiles are LAN/dev only. They cannot obtain a session token.

## Signed updater

`src-tauri/tauri.conf.json` has an updater stub (`plugins.updater`) with a
placeholder pubkey and endpoint. The Rust plugin is **not** registered until
you generate real minisign keys:

```bash
npm run tauri signer generate -- -w ~/.tauri/aureum.key
```

Put the public key in `plugins.updater.pubkey`, add `tauri-plugin-updater`,
and set `bundle.createUpdaterArtifacts` to `true`. Never commit the private key.

## Module map

| Module | Future crate | Owns |
| --- | --- | --- |
| `instances`, `launch`, `java`, `install` | `aureum-core` | Instances, JVM, official downloads |
| `auth` | `aureum-auth` | Microsoft/Xbox/Minecraft tokens + keychain |
| `catalog` | `aureum-catalog` | Modrinth adapter (official API + ETag cache) |
| `resolve` | `aureum-resolve` | Transitive deps, pins, lockfile |
| `mrpack` | (with resolve) | Modrinth `.mrpack` import / export |

## Mods (Modrinth)

The Mods page searches [Modrinth](https://docs.modrinth.com/api/) only. Install
targets the selected instance, walks required dependencies, writes
`aureum.lock.json`, and verifies SHA-1/SHA-512. CurseForge is not scraped.
Attribution stays on the Modrinth chip.

### Instance workspace

Select an instance on Home to open the right-hand workspace tabs: **Mods**
(pin / enable / update / remove / load order), **Packs**, **Screenshots**,
**Logs** (live tail + `logs/` / `crash-reports/` browser), and **Settings**
(JVM / memory / version upgrade). Load order is persisted for Aureum UI; Fabric
and Quilt still own class loading.

### `.mrpack` import / export

Use **Import .mrpack** on Home, or the instance **Packs** tab. From the Mods
catalog, choose **Modpacks** and **Install as instance** to download the
Modrinth `.mrpack` and run the same import pipeline. Export writes a
Modrinth-format pack from installed mods (CDN links when known; otherwise jars
are embedded under `overrides/`). Spec:
https://docs.modrinth.com/modpacks/format/

Resource packs, shaders, and datapacks install into `resourcepacks/`,
`shaderpacks/`, and `datapacks/` when **Add Content** is enabled.

### Forge smoke

```bash
# PowerShell
$env:AUREUM_FORGE_SMOKE="1"
cd src-tauri
cargo test forge_1211_install_smoke -- --ignored --nocapture
```

Downloads the 1.21.1 client jar + Forge installer (no assets, no game launch).
Ignored by default so routine `cargo test` stays offline-friendly.

GitHub Actions: `.github/workflows/forge-smoke.yml` runs the same check on
`workflow_dispatch` and a weekly schedule (not on every PR).

## License / policy

Respect the [Minecraft EULA](https://www.minecraft.net/en-us/eula) and
[Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines).
Do not redistribute game files.
