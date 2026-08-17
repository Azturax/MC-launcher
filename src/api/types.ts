export type Loader = "vanilla" | "fabric" | "forge" | "neoforge" | "quilt";

export interface Instance {
  id: string;
  name: string;
  loader: Loader | string;
  gameVersion: string;
  loaderVersion?: string | null;
  gameDir: string;
  javaPath?: string | null;
  memoryMb: number;
  jvmArgs?: string | null;
  keepOpen: boolean;
  lastPlayed?: string | null;
  icon?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface NewInstance {
  name: string;
  loader: string;
  gameVersion: string;
  loaderVersion?: string | null;
  javaPath?: string | null;
  memoryMb?: number;
  jvmArgs?: string | null;
  keepOpen?: boolean;
}

export interface InstancePatch {
  name?: string;
  javaPath?: string | null;
  memoryMb?: number;
  jvmArgs?: string | null;
  keepOpen?: boolean;
  icon?: string | null;
  gameVersion?: string;
  loader?: string;
  loaderVersion?: string | null;
}

export interface InstanceTemplate {
  id: string;
  name: string;
  loader: string;
  gameVersion: string;
  loaderVersion?: string | null;
  description: string;
}

export interface InstanceStatus {
  installed: boolean;
  clientJar?: string | null;
}

export interface GameVersion {
  id: string;
  type: string;
  url: string;
  sha1?: string | null;
  latest: boolean;
  latestSnapshot?: boolean;
}

export type VersionChannel = "release" | "snapshot" | "prerelease" | "legacy" | "all";

export interface LoaderVersion {
  loader: string;
  version: string;
  stable: boolean;
}

export interface Profile {
  id: string;
  kind: string;
  displayName: string;
  uuid?: string | null;
  skinUrl?: string | null;
  expiresAt?: string | null;
  hasSecret: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface AuthStatus {
  dryRun: boolean;
  hasClientId: boolean;
  tenant: string;
  redirectUri: string;
}

export interface AppInfo {
  name: string;
  version: string;
  disclaimer: string;
  disclaimerAccepted: boolean;
  dataDir: string;
  instancesRoot: string;
}

export interface JavaInstall {
  path: string;
  version: string;
  vendor?: string | null;
  source: string;
}

export interface MemoryInfo {
  totalMb: number;
  recommendedMb: number;
}

export interface InstallProgress {
  instanceId: string;
  phase: string;
  message: string;
  progress: number;
}

export interface LaunchLog {
  instanceId: string;
  stream: string;
  line: string;
}

export type Route =
  | "home"
  | "mods"
  | "shop"
  | "accounts"
  | "downloads"
  | "settings"
  | "about"
  | "author";

export interface DownloadHistoryEntry {
  instanceId: string;
  message: string;
  progress: number;
  updatedAt: number;
}

export type ThemeMode = "light" | "dark" | "system";
export type ModChannel = "stable" | "beta" | "all";

export type CatalogSource = "modrinth" | "curseforge";
export type CatalogSort = "relevance" | "downloads" | "updated" | "newest";
export type ProjectType = "mod" | "shader" | "resourcepack" | "datapack" | "modpack";

export interface CatalogProvider {
  id: string;
  label: string;
  enabled: boolean;
  reason?: string | null;
}

export interface CatalogCategory {
  name: string;
  projectType: string;
  header?: string | null;
}

export interface SearchFilters {
  query?: string;
  source?: string;
  loaders?: string[];
  gameVersions?: string[];
  projectTypes?: string[];
  categories?: string[];
  index?: CatalogSort | string;
  offset?: number;
  limit?: number;
  channel?: ModChannel | string;
}

export interface ProjectHit {
  id: string;
  slug: string;
  title: string;
  description: string;
  source: string;
  loaders: string[];
  gameVersions: string[];
  iconUrl?: string | null;
  downloads: number;
  projectType: string;
  categories?: string[];
}

export interface CatalogPage<T> {
  hits: T[];
  offset: number;
  total: number;
}

export interface GalleryImage {
  url: string;
  title?: string | null;
  description?: string | null;
  featured: boolean;
}

export interface DonationLink {
  id: string;
  platform: string;
  url: string;
}

export interface ProjectMember {
  userId: string;
  name: string;
  role: string;
  avatarUrl?: string | null;
}

export interface ProjectDetail {
  id: string;
  slug: string;
  title: string;
  description: string;
  body?: string | null;
  source: string;
  iconUrl?: string | null;
  loaders: string[];
  gameVersions: string[];
  license?: string | null;
  projectUrl: string;
  projectType: string;
  categories: string[];
  gallery: GalleryImage[];
  members: ProjectMember[];
  downloads: number;
  followers: number;
  published?: string | null;
  updated?: string | null;
  sourceUrl?: string | null;
  issuesUrl?: string | null;
  wikiUrl?: string | null;
  discordUrl?: string | null;
  donationUrls: DonationLink[];
}

export interface CatalogVersion {
  id: string;
  projectId: string;
  name: string;
  versionNumber: string;
  channel: string;
  loaders: string[];
  gameVersions: string[];
  featured: boolean;
  datePublished?: string | null;
}

export interface InstalledMod {
  id: string;
  instanceId: string;
  projectId: string;
  versionId: string;
  filename: string;
  source: string;
  sha512?: string | null;
  sha1?: string | null;
  pinned: boolean;
  enabled: boolean;
  channel?: string | null;
  updateVersionId?: string | null;
  displayName?: string | null;
  versionNumber?: string | null;
  sortOrder?: number;
  /** Instance-scoped: ok | update | incompatible | pinned | local */
  compatStatus?: string | null;
}

export interface InstallModRequest {
  instanceId: string;
  projectId: string;
  versionId?: string | null;
  pin?: boolean;
  channel?: string;
}

export interface ImportMrpackRequest {
  path: string;
  instanceId?: string | null;
  name?: string | null;
}

export interface ExportMrpackRequest {
  instanceId: string;
  path: string;
  name?: string | null;
  versionId?: string | null;
  summary?: string | null;
}

export interface MrpackImportResult {
  instance: Instance;
  filesInstalled: number;
  created: boolean;
}

export interface InstallModpackRequest {
  projectId: string;
  versionId?: string | null;
  name?: string | null;
  channel?: string;
}

export interface InstallContentRequest {
  instanceId: string;
  projectId: string;
  versionId?: string | null;
  projectType: string;
  channel?: string;
}

export interface ContentInstallResult {
  path: string;
  filename: string;
  projectType: string;
}

export interface InstalledContent {
  id: string;
  instanceId: string;
  projectType: string;
  filename: string;
  path: string;
  source: string;
  projectId?: string | null;
  versionId?: string | null;
  sha1?: string | null;
  sha512?: string | null;
  kind: string;
  updateVersionId?: string | null;
  compatStatus?: string | null;
}

export interface AuthorStatus {
  oauthConfigured: boolean;
  secretConfigured: boolean;
  connected: boolean;
  expired: boolean;
  dryRun: boolean;
  username?: string | null;
  userId?: string | null;
  expiresAt?: string | null;
  redirectUri: string;
  scopes: string;
  note: string;
}

export interface AuthorProject {
  id: string;
  title: string;
  slug?: string | null;
  summary: string;
  description: string;
  projectType: string;
  status: string;
  modrinthId?: string | null;
  localPath?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface NewAuthorProject {
  title: string;
  slug?: string | null;
  summary?: string | null;
  description?: string | null;
  projectType?: string | null;
}

export interface AuthorProjectPatch {
  title?: string;
  slug?: string | null;
  summary?: string;
  description?: string;
  projectType?: string;
  status?: string;
  modrinthId?: string | null;
  localPath?: string | null;
}

export interface PublishChecklistItem {
  id: string;
  label: string;
  done: boolean;
}

export interface RemoteModrinthProject {
  id: string;
  slug: string;
  title: string;
  description: string;
  projectType: string;
  iconUrl?: string | null;
}

export interface CreateRemoteProjectRequest {
  draftId: string;
  categories?: string[];
  clientSide?: string;
  serverSide?: string;
  licenseId?: string;
}

export interface PublishVersionRequest {
  projectId: string;
  draftId?: string | null;
  name: string;
  versionNumber: string;
  changelog?: string | null;
  gameVersions: string[];
  loaders: string[];
  versionType?: string | null;
  featured?: boolean;
  filePath: string;
}

export interface PublishVersionResult {
  versionId: string;
  projectId: string;
  versionNumber: string;
  projectUrl: string;
}

export interface UploadAuthorMediaRequest {
  projectId: string;
  filePath: string;
  featured?: boolean;
  title?: string;
  description?: string;
}

export interface UploadAuthorMediaResult {
  projectId: string;
  kind: string;
  note: string;
}

export interface MediaFile {
  name: string;
  size: number;
  modifiedAt?: string | null;
  mime: string;
}

export interface MediaPreview {
  name: string;
  mime: string;
  dataUrl: string;
}

export type LogFolder = "logs" | "crashReports";

export interface LogFileEntry {
  name: string;
  folder: LogFolder;
  size: number;
  modifiedAt?: string | null;
  previewable: boolean;
}

export interface LogFilePreview {
  name: string;
  folder: LogFolder;
  text: string;
  truncated: boolean;
  size: number;
}

export interface RemoteGalleryImage {
  url: string;
  title?: string | null;
  description?: string | null;
  featured: boolean;
  ordering?: number | null;
}

export interface GalleryImageEditRequest {
  projectId: string;
  url: string;
  featured?: boolean;
}

export type WorkspaceTab = "mods" | "settings" | "logs" | "packs" | "screenshots";

/** Resource packs & shaders: never hard-filter by MC version / loader. */
export function ignoresInstanceVersion(type: string): boolean {
  return type === "resourcepack" || type === "shader";
}