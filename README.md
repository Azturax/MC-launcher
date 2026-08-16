# Aureum

A local-first desktop instance manager for Minecraft: Java Edition.
**Speed for casual players, power for modders.**

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

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

Binary builds are published on [GitHub Releases](https://github.com/Azturax/MC-launcher/releases):

| Channel | Where to find it | Notes |
| --- | --- | --- |
| **Alpha** | Releases marked **Pre-release**, tags like `v0.x.x-alpha.N` | Early builds; expect bugs and breaking changes |
| **Stable** | Full (non–pre-release) Releases | Prefer these for day-to-day use |

Until the first assets are attached, build from source (below). Alpha installers will appear on Pre-release entries when published — do not treat alpha as production-ready.

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

## Mods (Modrinth)

The Mods page searches [Modrinth](https://docs.modrinth.com/api/) only. Install
targets the selected instance, walks required dependencies, writes
`aureum.lock.json`, and verifies SHA-1/SHA-512. CurseForge is not scraped.
Attribution stays on the Modrinth chip.

## License / policy

Respect the [Minecraft EULA](https://www.minecraft.net/en-us/eula) and
[Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines).
Do not redistribute game files.
