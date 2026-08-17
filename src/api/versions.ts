import type { GameVersion, VersionChannel } from "./types";

export function versionChannelOf(v: GameVersion): Exclude<VersionChannel, "all"> {
  const id = v.id.toLowerCase();
  if (/(?:-pre\d+|-rc\d+)$/i.test(id) || id.includes("-pre") || id.includes("-rc")) {
    return "prerelease";
  }
  if (v.type === "old_beta" || v.type === "old_alpha") {
    return "legacy";
  }
  if (v.type === "snapshot") {
    return "snapshot";
  }
  return "release";
}

export function filterGameVersions(
  list: GameVersion[],
  channel: VersionChannel,
  limit = 150,
): GameVersion[] {
  const filtered =
    channel === "all"
      ? list
      : list.filter((v) => {
          const kind = versionChannelOf(v);
          if (kind === channel) return true;
          // Mojang's latest.snapshot is often a pre or RC — keep it on Snapshots too.
          return channel === "snapshot" && Boolean(v.latestSnapshot);
        });
  return filtered.slice(0, limit);
}

export function versionLabel(v: GameVersion): string {
  const tags: string[] = [];
  if (v.latest) tags.push("latest release");
  if (v.latestSnapshot) tags.push("latest snapshot");
  const channel = versionChannelOf(v);
  if (channel === "prerelease" && !v.latestSnapshot) tags.push("pre/rc");
  if (channel === "snapshot" && !v.latestSnapshot) tags.push("snapshot");
  if (channel === "legacy") tags.push(v.type);
  return tags.length ? `${v.id} (${tags.join(", ")})` : v.id;
}
