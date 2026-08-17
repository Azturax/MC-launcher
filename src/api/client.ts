import type {
  AppInfo,
  AuthStatus,
  GameVersion,
  Instance,
  InstancePatch,
  InstanceStatus,
  InstanceTemplate,
  JavaInstall,
  LoaderVersion,
  MemoryInfo,
  NewInstance,
  Profile,
  CatalogCategory,
  CatalogPage,
  CatalogProvider,
  CatalogVersion,
  InstallModRequest,
  InstalledMod,
  ImportMrpackRequest,
  ExportMrpackRequest,
  MrpackImportResult,
  InstallModpackRequest,
  InstallContentRequest,
  ContentInstallResult,
  InstalledContent,
  ProjectDetail,
  ProjectHit,
  SearchFilters,
  AuthorStatus,
  AuthorProject,
  NewAuthorProject,
  AuthorProjectPatch,
  PublishChecklistItem,
  RemoteModrinthProject,
  PublishVersionRequest,
  PublishVersionResult,
  CreateRemoteProjectRequest,
  UploadAuthorMediaRequest,
  UploadAuthorMediaResult,
  MediaFile,
  MediaPreview,
  LogFileEntry,
  LogFilePreview,
  RemoteGalleryImage,
  GalleryImageEditRequest,
} from "./types";

const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    return mockInvoke<T>(cmd, args);
  }
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(cmd, args);
}

export const api = {
  isTauri,
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  acceptDisclaimer: () => invoke<void>("accept_disclaimer"),
  getSettings: () => invoke<Record<string, string>>("get_settings"),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  listInstances: () => invoke<Instance[]>("list_instances"),
  getInstance: (id: string) => invoke<Instance>("get_instance", { id }),
  createInstance: (newInstance: NewInstance) =>
    invoke<Instance>("create_instance", { new: newInstance }),
  updateInstance: (id: string, patch: InstancePatch) =>
    invoke<Instance>("update_instance", { id, patch }),
  upgradeInstanceVersion: (
    id: string,
    gameVersion: string,
    loader?: string | null,
    loaderVersion?: string | null,
  ) =>
    invoke<Instance>("upgrade_instance_version", {
      id,
      gameVersion,
      loader: loader ?? null,
      loaderVersion: loaderVersion ?? null,
    }),
  deleteInstance: (id: string, removeFiles: boolean) =>
    invoke<void>("delete_instance", { id, removeFiles }),
  cloneInstance: (id: string) => invoke<Instance>("clone_instance", { id }),
  listTemplates: () => invoke<InstanceTemplate[]>("list_templates"),
  applyTemplate: (templateId: string, name?: string) =>
    invoke<Instance>("apply_template", { templateId, name }),
  instanceStatus: (id: string) => invoke<InstanceStatus>("instance_status", { id }),
  listGameVersions: () => invoke<GameVersion[]>("list_game_versions"),
  listLoaderVersions: (loader: string, gameVersion: string) =>
    invoke<LoaderVersion[]>("list_loader_versions", { loader, gameVersion }),
  installInstance: (id: string) => invoke<string>("install_instance", { id }),
  launchInstance: (id: string, profileId?: string) =>
    invoke<number>("launch_instance", { id, profileId }),
  stopInstance: (id: string) => invoke<void>("stop_instance", { id }),
  listRunningInstances: () => invoke<string[]>("list_running_instances"),
  getLogTail: (id: string, lines?: number) => invoke<string[]>("get_log_tail", { id, lines }),
  openInstanceFolder: (id: string) => invoke<void>("open_instance_folder", { id }),
  openCrashReports: (id: string) => invoke<void>("open_crash_reports", { id }),
  listProfiles: () => invoke<Profile[]>("list_profiles"),
  authStatus: () => invoke<AuthStatus>("auth_status"),
  startMicrosoftLogin: () => invoke<Profile>("start_microsoft_login"),
  createOfflineProfile: (displayName: string) =>
    invoke<Profile>("create_offline_profile", { displayName }),
  deleteProfile: (id: string) => invoke<void>("delete_profile", { id }),
  setActiveProfile: (id: string) => invoke<void>("set_active_profile", { id }),
  getActiveProfile: () => invoke<Profile | null>("get_active_profile"),
  discoverJava: () => invoke<JavaInstall[]>("discover_java"),
  getSystemMemory: () => invoke<MemoryInfo>("get_system_memory"),
  listCatalogProviders: () => invoke<CatalogProvider[]>("list_catalog_providers"),
  listCatalogCategories: () => invoke<CatalogCategory[]>("list_catalog_categories"),
  searchCatalog: (filters: SearchFilters) =>
    invoke<CatalogPage<ProjectHit>>("search_catalog", { filters }),
  getCatalogProject: (id: string) => invoke<ProjectDetail>("get_catalog_project", { id }),
  listCatalogVersions: (
    id: string,
    loaders?: string[],
    gameVersions?: string[],
    channel?: string,
  ) =>
    invoke<CatalogVersion[]>("list_catalog_versions", {
      id,
      loaders,
      gameVersions,
      channel,
    }),
  installMod: (request: InstallModRequest) => invoke("install_mod", { request }),
  listInstanceMods: (instanceId: string) =>
    invoke<InstalledMod[]>("list_instance_mods", { instanceId }),
  openModsFolder: (instanceId: string) =>
    invoke<void>("open_mods_folder", { instanceId }),
  setModPin: (instanceId: string, projectId: string, pinned: boolean) =>
    invoke<void>("set_mod_pin", { instanceId, projectId, pinned }),
  setModEnabled: (instanceId: string, projectId: string, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { instanceId, projectId, enabled }),
  removeMod: (instanceId: string, projectId: string) =>
    invoke<void>("remove_mod", { instanceId, projectId }),
  checkModUpdates: (instanceId: string) =>
    invoke<InstalledMod[]>("check_mod_updates", { instanceId }),
  reorderMods: (instanceId: string, projectIds: string[]) =>
    invoke<InstalledMod[]>("reorder_mods", { instanceId, projectIds }),
  importMrpack: (request: ImportMrpackRequest) =>
    invoke<MrpackImportResult>("import_mrpack", { request }),
  exportMrpack: (request: ExportMrpackRequest) =>
    invoke<string>("export_mrpack", { request }),
  pickMrpackFile: () => invoke<string | null>("pick_mrpack_file"),
  pickMrpackSave: (defaultName?: string) =>
    invoke<string | null>("pick_mrpack_save", { defaultName }),
  installModpack: (request: InstallModpackRequest) =>
    invoke<MrpackImportResult>("install_modpack", { request }),
  installContent: (request: InstallContentRequest) =>
    invoke<ContentInstallResult>("install_content", { request }),
  listInstanceContent: (instanceId: string) =>
    invoke<InstalledContent[]>("list_instance_content", { instanceId }),
  checkContentUpdates: (instanceId: string) =>
    invoke<InstalledContent[]>("check_content_updates", { instanceId }),
  applyContentUpdate: (instanceId: string, contentId: string) =>
    invoke<ContentInstallResult>("apply_content_update", { instanceId, contentId }),
  removeContent: (instanceId: string, contentId: string) =>
    invoke<void>("remove_content", { instanceId, contentId }),
  openContentFolder: (instanceId: string, projectType?: string) =>
    invoke<void>("open_content_folder", { instanceId, projectType }),
  listInstanceMedia: (instanceId: string) =>
    invoke<MediaFile[]>("list_instance_media", { instanceId }),
  readMediaPreview: (instanceId: string, name: string) =>
    invoke<MediaPreview>("read_media_preview", { instanceId, name }),
  readMediaThumb: (instanceId: string, name: string) =>
    invoke<MediaPreview>("read_media_thumb", { instanceId, name }),
  deleteMediaFile: (instanceId: string, name: string) =>
    invoke<void>("delete_media_file", { instanceId, name }),
  openMediaFolder: (instanceId: string) =>
    invoke<void>("open_media_folder", { instanceId }),
  pickMediaFiles: () => invoke<string[]>("pick_media_files"),
  importMediaFiles: (instanceId: string, paths: string[]) =>
    invoke<MediaFile[]>("import_media_files", { instanceId, paths }),
  listLogFiles: (instanceId: string, folder: string) =>
    invoke<LogFileEntry[]>("list_log_files", { instanceId, folder }),
  readLogFile: (instanceId: string, folder: string, name: string) =>
    invoke<LogFilePreview>("read_log_file", { instanceId, folder, name }),
  deleteLogFile: (instanceId: string, folder: string, name: string) =>
    invoke<void>("delete_log_file", { instanceId, folder, name }),
  openLogFolder: (instanceId: string, folder: string) =>
    invoke<void>("open_log_folder", { instanceId, folder }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  authorStatus: () => invoke<AuthorStatus>("author_status"),
  startModrinthLogin: () => invoke<AuthorStatus>("start_modrinth_login"),
  connectModrinthPat: (pat: string) => invoke<AuthorStatus>("connect_modrinth_pat", { pat }),
  disconnectModrinth: () => invoke<AuthorStatus>("disconnect_modrinth"),
  listMyModrinthProjects: () =>
    invoke<RemoteModrinthProject[]>("list_my_modrinth_projects"),
  linkAuthorDraft: (draftId: string, remoteId: string) =>
    invoke<AuthorProject>("link_author_draft", { draftId, remoteId }),
  importModrinthProject: (remoteId: string) =>
    invoke<AuthorProject>("import_modrinth_project", { remoteId }),
  createModrinthProject: (request: CreateRemoteProjectRequest) =>
    invoke<AuthorProject>("create_modrinth_project", { request }),
  pickPublishFile: () => invoke<string | null>("pick_publish_file"),
  pickAuthorImage: () => invoke<string | null>("pick_author_image"),
  uploadAuthorIcon: (request: UploadAuthorMediaRequest) =>
    invoke<UploadAuthorMediaResult>("upload_author_icon", { request }),
  uploadAuthorGallery: (request: UploadAuthorMediaRequest) =>
    invoke<UploadAuthorMediaResult>("upload_author_gallery", { request }),
  listAuthorGallery: (projectId: string) =>
    invoke<RemoteGalleryImage[]>("list_author_gallery", { projectId }),
  setAuthorGalleryFeatured: (request: GalleryImageEditRequest) =>
    invoke<UploadAuthorMediaResult>("set_author_gallery_featured", { request }),
  deleteAuthorGalleryImage: (request: GalleryImageEditRequest) =>
    invoke<UploadAuthorMediaResult>("delete_author_gallery_image", { request }),
  publishAuthorVersion: (request: PublishVersionRequest) =>
    invoke<PublishVersionResult>("publish_author_version", { request }),
  listAuthorProjects: () => invoke<AuthorProject[]>("list_author_projects"),
  getAuthorProject: (id: string) => invoke<AuthorProject>("get_author_project", { id }),
  createAuthorProject: (newProject: NewAuthorProject) =>
    invoke<AuthorProject>("create_author_project", { new: newProject }),
  updateAuthorProject: (id: string, patch: AuthorProjectPatch) =>
    invoke<AuthorProject>("update_author_project", { id, patch }),
  deleteAuthorProject: (id: string) => invoke<void>("delete_author_project", { id }),
  authorPublishChecklist: (id: string) =>
    invoke<PublishChecklistItem[]>("author_publish_checklist", { id }),
};

let mockStore = loadMock();

function loadMock() {
  const raw = localStorage.getItem("aureum-mock");
  if (raw) {
    try {
      return JSON.parse(raw) as MockState;
    } catch {
      /* fall through */
    }
  }
  return {
    disclaimer: false,
    settings: { theme: "system", contrast: "normal" } as Record<string, string>,
    instances: [] as Instance[],
    profiles: [] as Profile[],
    activeProfile: "",
  };
}

interface MockState {
  disclaimer: boolean;
  settings: Record<string, string>;
  instances: Instance[];
  profiles: Profile[];
  activeProfile: string;
  authorProjects?: AuthorProject[];
  mediaByInstance?: Record<string, MediaFile[]>;
}

function saveMock() {
  localStorage.setItem("aureum-mock", JSON.stringify(mockStore));
}

function iso() {
  return new Date().toISOString();
}

async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    case "get_app_info":
      return {
        name: "Aureum",
        version: "0.1.0-dev",
        disclaimer:
          "NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT",
        disclaimerAccepted: mockStore.disclaimer,
        dataDir: "(browser preview)",
        instancesRoot: "(browser preview)",
      } as T;
    case "accept_disclaimer":
      mockStore.disclaimer = true;
      saveMock();
      return undefined as T;
    case "get_settings":
      return mockStore.settings as T;
    case "set_setting":
      mockStore.settings[String(args?.key)] = String(args?.value ?? "");
      saveMock();
      return undefined as T;
    case "list_instances":
      return mockStore.instances as T;
    case "create_instance": {
      const n = args?.new as NewInstance;
      const inst: Instance = {
        id: crypto.randomUUID(),
        name: n.name,
        loader: n.loader,
        gameVersion: n.gameVersion,
        loaderVersion: n.loaderVersion,
        gameDir: `instances/${n.name}`,
        javaPath: n.javaPath,
        memoryMb: n.memoryMb ?? 2048,
        jvmArgs: n.jvmArgs,
        keepOpen: n.keepOpen ?? true,
        createdAt: iso(),
        updatedAt: iso(),
      };
      mockStore.instances.unshift(inst);
      saveMock();
      return inst as T;
    }
    case "update_instance": {
      const id = String(args?.id);
      const patch = (args?.patch ?? {}) as InstancePatch;
      const inst = mockStore.instances.find((i) => i.id === id);
      if (!inst) throw new Error("Instance not found");
      Object.assign(inst, patch, { updatedAt: iso() });
      saveMock();
      return inst as T;
    }
    case "upgrade_instance_version": {
      const id = String(args?.id);
      const inst = mockStore.instances.find((i) => i.id === id);
      if (!inst) throw new Error("Instance not found");
      inst.gameVersion = String(args?.gameVersion ?? inst.gameVersion);
      if (args?.loader) inst.loader = String(args.loader);
      inst.loaderVersion =
        inst.loader === "vanilla" ? null : ((args?.loaderVersion as string | null) ?? inst.loaderVersion);
      inst.updatedAt = iso();
      saveMock();
      return inst as T;
    }
    case "delete_instance":
      mockStore.instances = mockStore.instances.filter((i) => i.id !== args?.id);
      saveMock();
      return undefined as T;
    case "clone_instance": {
      const src = mockStore.instances.find((i) => i.id === args?.id);
      if (!src) throw new Error("Instance not found");
      const copy = {
        ...src,
        id: crypto.randomUUID(),
        name: `${src.name} (copy)`,
        createdAt: iso(),
        updatedAt: iso(),
      };
      mockStore.instances.unshift(copy);
      saveMock();
      return copy as T;
    }
    case "list_templates":
      return [
        {
          id: "vanilla-latest",
          name: "Vanilla Latest",
          loader: "vanilla",
          gameVersion: "latest-release",
          description: "Official release, isolated game directory.",
        },
        {
          id: "fabric-1.21.1",
          name: "Fabric 1.21.1",
          loader: "fabric",
          gameVersion: "1.21.1",
          description: "Fabric loader on 1.21.1.",
        },
      ] as T;
    case "apply_template": {
      const name = (args?.name as string | undefined) ?? "New instance";
      return api.createInstance({
        name,
        loader: "vanilla",
        gameVersion: "1.21.1",
        keepOpen: true,
      }) as T;
    }
    case "instance_status":
      return { installed: false } as T;
    case "list_game_versions":
      return [
        { id: "1.21.8", type: "release", url: "", latest: true, latestSnapshot: false },
        { id: "1.21.11-rc1", type: "snapshot", url: "", latest: false, latestSnapshot: false },
        { id: "1.21.11-pre1", type: "snapshot", url: "", latest: false, latestSnapshot: false },
        { id: "25w31a", type: "snapshot", url: "", latest: false, latestSnapshot: true },
        { id: "1.21.1", type: "release", url: "", latest: false, latestSnapshot: false },
        { id: "1.20.1", type: "release", url: "", latest: false, latestSnapshot: false },
        { id: "b1.7.3", type: "old_beta", url: "", latest: false, latestSnapshot: false },
      ] as T;
    case "list_loader_versions":
      return [{ loader: String(args?.loader), version: "latest", stable: true }] as T;
    case "install_instance":
    case "launch_instance":
      throw new Error("Install and launch require the Aureum desktop shell (npm run tauri dev).");
    case "list_running_instances":
      return [] as T;
    case "get_log_tail":
      return [] as T;
    case "list_profiles":
      return mockStore.profiles as T;
    case "auth_status":
      return {
        dryRun: true,
        hasClientId: false,
        tenant: "consumers",
        redirectUri: "http://127.0.0.1:17890/auth/callback",
      } as T;
    case "start_microsoft_login": {
      const p: Profile = {
        id: crypto.randomUUID(),
        kind: "microsoft-dry-run",
        displayName: "Dev Player",
        uuid: crypto.randomUUID(),
        hasSecret: false,
        createdAt: iso(),
        updatedAt: iso(),
      };
      mockStore.profiles.unshift(p);
      mockStore.activeProfile = p.id;
      saveMock();
      return p as T;
    }
    case "create_offline_profile": {
      const p: Profile = {
        id: crypto.randomUUID(),
        kind: "offline",
        displayName: String(args?.displayName ?? "Player"),
        uuid: crypto.randomUUID(),
        hasSecret: false,
        createdAt: iso(),
        updatedAt: iso(),
      };
      mockStore.profiles.unshift(p);
      saveMock();
      return p as T;
    }
    case "delete_profile":
      mockStore.profiles = mockStore.profiles.filter((p) => p.id !== args?.id);
      saveMock();
      return undefined as T;
    case "set_active_profile":
      mockStore.activeProfile = String(args?.id);
      saveMock();
      return undefined as T;
    case "get_active_profile":
      return (mockStore.profiles.find((p) => p.id === mockStore.activeProfile) ?? null) as T;
    case "discover_java":
      return [] as T;
    case "get_system_memory":
      return { totalMb: 16384, recommendedMb: 4096 } as T;
    case "list_catalog_providers":
      return [
        { id: "modrinth", label: "Modrinth", enabled: true },
        {
          id: "curseforge",
          label: "CurseForge",
          enabled: false,
          reason: "Hidden until a licensed API key is configured.",
        },
      ] as T;
    case "list_catalog_categories":
      return [
        { name: "optimization", projectType: "mod", header: "categories" },
        { name: "utility", projectType: "mod", header: "categories" },
        { name: "realistic", projectType: "shader", header: "categories" },
        { name: "16x", projectType: "resourcepack", header: "resolutions" },
        { name: "adventure", projectType: "datapack", header: "categories" },
        { name: "technology", projectType: "modpack", header: "categories" },
      ] as T;
    case "search_catalog":
      return {
        hits: [
          {
            id: "sodium",
            slug: "sodium",
            title: "Sodium",
            description: "Modern rendering (browser mock).",
            source: "modrinth",
            loaders: ["fabric"],
            gameVersions: ["1.21.1"],
            downloads: 0,
            projectType: "mod",
            categories: ["optimization"],
          },
        ],
        offset: 0,
        total: 1,
      } as T;
    case "get_catalog_project":
      return {
        id: "sodium",
        slug: "sodium",
        title: "Sodium",
        description: "Modern rendering (browser mock).",
        body: "## Sodium\n\nA mock project body with a [link](https://modrinth.com).",
        source: "modrinth",
        iconUrl: null,
        loaders: ["fabric"],
        gameVersions: ["1.21.1"],
        license: "MIT",
        projectUrl: "https://modrinth.com/mod/sodium",
        projectType: "mod",
        categories: ["optimization"],
        gallery: [],
        members: [{ userId: "1", name: "MockAuthor", role: "Owner", avatarUrl: null }],
        downloads: 0,
        followers: 0,
        published: null,
        updated: null,
        sourceUrl: "https://github.com/example/sodium",
        issuesUrl: null,
        wikiUrl: null,
        discordUrl: null,
        donationUrls: [],
      } as T;
    case "list_catalog_versions":
      return [
        {
          id: "mock-ver",
          projectId: "sodium",
          name: "mock",
          versionNumber: "0.0.0",
          channel: "release",
          loaders: ["fabric"],
          gameVersions: ["1.21.1"],
          featured: true,
        },
      ] as T;
    case "list_instance_mods":
    case "check_mod_updates":
    case "reorder_mods":
      return [] as T;
    case "install_mod":
      throw new Error("Mod install requires the Aureum desktop shell (npm run tauri dev).");
    case "import_mrpack":
    case "export_mrpack":
    case "install_modpack":
    case "install_content":
      throw new Error("Pack/content install requires the Aureum desktop shell (npm run tauri dev).");
    case "list_instance_content":
      return [] as T;
    case "remove_content":
    case "open_content_folder":
      return undefined as T;
    case "pick_mrpack_file":
    case "pick_mrpack_save":
      return null as T;
    case "set_mod_pin":
    case "set_mod_enabled":
    case "remove_mod":
      return undefined as T;
    case "author_status":
      return {
        oauthConfigured: false,
        secretConfigured: false,
        connected: false,
        expired: false,
        dryRun: true,
        username: null,
        userId: null,
        expiresAt: null,
        redirectUri: "http://127.0.0.1:17891/modrinth/callback",
        scopes: "USER_READ+PROJECT_READ+PROJECT_CREATE+VERSION_CREATE",
        note: "Browser mock — use tauri for real Modrinth OAuth.",
      } as T;
    case "start_modrinth_login":
    case "connect_modrinth_pat":
    case "disconnect_modrinth":
      return {
        oauthConfigured: true,
        secretConfigured: true,
        connected: cmd !== "disconnect_modrinth",
        expired: false,
        dryRun: true,
        username: cmd === "disconnect_modrinth" ? null : "DryRunCreator",
        userId: cmd === "disconnect_modrinth" ? null : "dry-run",
        expiresAt: null,
        redirectUri: "http://127.0.0.1:17891/modrinth/callback",
        scopes: "USER_READ+PROJECT_READ+PROJECT_CREATE+VERSION_CREATE",
        note: "Browser mock connect.",
      } as T;
    case "list_my_modrinth_projects":
      return [
        {
          id: "dryrunproj",
          slug: "aureum-dry-run-mod",
          title: "Aureum Dry-Run Mod",
          description: "Simulated project",
          projectType: "mod",
          iconUrl: null,
        },
      ] as T;
    case "link_author_draft":
    case "import_modrinth_project": {
      const p: AuthorProject = {
        id: crypto.randomUUID(),
        title: "Linked project",
        slug: "linked",
        summary: "Linked from Modrinth",
        description: "",
        projectType: "mod",
        status: "checklist",
        modrinthId: String(args?.remoteId ?? "dryrunproj"),
        createdAt: iso(),
        updatedAt: iso(),
      };
      mockStore.authorProjects = [p, ...(mockStore.authorProjects ?? [])];
      saveMock();
      return p as T;
    }
    case "pick_publish_file":
      return null as T;
    case "pick_author_image":
      return null as T;
    case "upload_author_icon":
    case "upload_author_gallery":
      return {
        projectId: String((args?.request as { projectId?: string })?.projectId ?? ""),
        kind: cmd === "upload_author_icon" ? "icon" : "gallery",
        note: "Browser mock upload.",
      } as T;
    case "publish_author_version":
      return {
        versionId: "mock-ver",
        projectId: "dryrunproj",
        versionNumber: "0.0.1",
        projectUrl: "https://modrinth.com/mod/aureum-dry-run-mod",
      } as T;
    case "check_content_updates":
      return [] as T;
    case "apply_content_update":
      return {
        path: "/mock/pack.zip",
        filename: "pack.zip",
        projectType: "datapack",
      } as T;
    case "list_instance_media": {
      const id = String(args?.instanceId ?? "");
      return (mockStore.mediaByInstance?.[id] ?? []) as T;
    }
    case "read_media_preview": {
      const name = String(args?.name ?? "shot.png");
      return {
        name,
        mime: "image/png",
        dataUrl:
          "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
      } as T;
    }
    case "read_media_thumb": {
      const name = String(args?.name ?? "shot.png");
      return {
        name,
        mime: "image/jpeg",
        dataUrl:
          "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAn/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/8QAFQEBAQAAAAAAAAAAAAAAAAAAAAX/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIQAxAAAAGfAP/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAQUCf//EABQRAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQMBAT8Bf//EABQRAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQIBAT8Bf//Z",
      } as T;
    }
    case "list_log_files":
      return [
        {
          name: "latest.log",
          folder: "logs",
          size: 120,
          modifiedAt: iso(),
          previewable: true,
        },
      ] as T;
    case "read_log_file":
      return {
        name: String(args?.name ?? "latest.log"),
        folder: "logs",
        text: "[mock] log line\n",
        truncated: false,
        size: 20,
      } as T;
    case "delete_log_file":
    case "open_log_folder":
      return undefined as T;
    case "list_author_gallery":
      return [
        {
          url: "https://cdn.modrinth.com/data/dry/images/dry-gallery.png",
          title: "Mock",
          featured: true,
          ordering: 0,
        },
      ] as T;
    case "set_author_gallery_featured":
    case "delete_author_gallery_image":
      return {
        projectId: String((args?.request as { projectId?: string })?.projectId ?? ""),
        kind: "gallery",
        note: "Browser mock gallery edit.",
      } as T;
    case "delete_media_file": {
      const id = String(args?.instanceId ?? "");
      const name = String(args?.name ?? "");
      const list = mockStore.mediaByInstance?.[id] ?? [];
      mockStore.mediaByInstance = {
        ...(mockStore.mediaByInstance ?? {}),
        [id]: list.filter((m) => m.name !== name),
      };
      saveMock();
      return undefined as T;
    }
    case "open_media_folder":
    case "pick_media_files":
      return (cmd === "pick_media_files" ? [] : undefined) as T;
    case "import_media_files": {
      const id = String(args?.instanceId ?? "");
      const paths = (args?.paths as string[]) ?? [];
      const added: MediaFile[] = paths.map((p, i) => ({
        name: p.split(/[/\\]/).pop() || `import-${i}.png`,
        size: 1024,
        modifiedAt: iso(),
        mime: "image/png",
      }));
      mockStore.mediaByInstance = {
        ...(mockStore.mediaByInstance ?? {}),
        [id]: [...added, ...(mockStore.mediaByInstance?.[id] ?? [])],
      };
      saveMock();
      return added as T;
    }
    case "create_modrinth_project": {
      const draftId = String((args?.request as { draftId?: string })?.draftId ?? "");
      const list = mockStore.authorProjects ?? [];
      const p = list.find((x) => x.id === draftId);
      if (!p) throw new Error("Author project not found");
      p.modrinthId = "newremote";
      p.status = "checklist";
      p.updatedAt = iso();
      saveMock();
      return p as T;
    }
    case "list_author_projects":
      return (mockStore.authorProjects ?? []) as T;
    case "get_author_project": {
      const p = (mockStore.authorProjects ?? []).find((x) => x.id === args?.id);
      if (!p) throw new Error("Author project not found");
      return p as T;
    }
    case "create_author_project": {
      const n = args?.new as NewAuthorProject;
      const now = iso();
      const p: AuthorProject = {
        id: crypto.randomUUID(),
        title: n.title,
        slug: n.slug ?? null,
        summary: n.summary ?? "",
        description: n.description ?? "",
        projectType: n.projectType ?? "mod",
        status: "draft",
        createdAt: now,
        updatedAt: now,
      };
      mockStore.authorProjects = [p, ...(mockStore.authorProjects ?? [])];
      saveMock();
      return p as T;
    }
    case "update_author_project": {
      const id = String(args?.id);
      const patch = (args?.patch ?? {}) as AuthorProjectPatch;
      const list = mockStore.authorProjects ?? [];
      const p = list.find((x) => x.id === id);
      if (!p) throw new Error("Author project not found");
      Object.assign(p, patch, { updatedAt: iso() });
      saveMock();
      return p as T;
    }
    case "delete_author_project":
      mockStore.authorProjects = (mockStore.authorProjects ?? []).filter((p) => p.id !== args?.id);
      saveMock();
      return undefined as T;
    case "author_publish_checklist": {
      const p = (mockStore.authorProjects ?? []).find((x) => x.id === args?.id);
      if (!p) throw new Error("Author project not found");
      return [
        { id: "title", label: "Project title set", done: !!p.title.trim() },
        { id: "summary", label: "Short summary", done: p.summary.trim().length >= 16 },
        { id: "body", label: "Long description drafted", done: p.description.trim().length >= 40 },
        { id: "slug", label: "URL slug chosen", done: !!p.slug },
        { id: "oauth", label: "Connect Modrinth account (coming soon)", done: false },
      ] as T;
    }
    case "open_external":
    case "open_instance_folder":
    case "open_mods_folder":
    case "open_crash_reports":
    case "stop_instance":
      return undefined as T;
    default:
      throw new Error(`Unknown command ${cmd}`);
  }
}
