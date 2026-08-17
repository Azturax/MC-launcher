import { useEffect, useState } from "react";
import { api } from "../api/client";
import { Button, SelectField, Switch, TextField } from "../components/ui";
import { useAppStore } from "../store/app";
import { presetById } from "../theme/presets";
import type { Contrast } from "../theme/tokens";
import type { JavaInstall, MemoryInfo, ThemeMode } from "../api/types";

export function Settings() {
  const {
    themeMode,
    contrast,
    themePresetId,
    supportRank,
    addContent,
    setThemeMode,
    setContrast,
    setAddContent,
    setRoute,
  } = useAppStore();
  const [javas, setJavas] = useState<JavaInstall[]>([]);
  const [memory, setMemory] = useState<MemoryInfo | null>(null);
  const [javaPath, setJavaPath] = useState("");
  const [memoryMb, setMemoryMb] = useState(2048);
  const [jvmArgs, setJvmArgs] = useState("");
  const [proxy, setProxy] = useState("");
  const [instancesRoot, setInstancesRoot] = useState("");

  useEffect(() => {
    void api.discoverJava().then(setJavas).catch(() => undefined);
    void api.getSystemMemory().then((m) => {
      setMemory(m);
      setMemoryMb(m.recommendedMb);
    });
    void api.getSettings().then((s) => {
      setJavaPath(s.java_path ?? "");
      setJvmArgs(s.jvm_args ?? "");
      setProxy(s.proxy_url ?? "");
      setInstancesRoot(s.instances_root ?? "");
      if (s.memory_mb) setMemoryMb(Number(s.memory_mb));
    });
  }, []);

  async function persist(key: string, value: string) {
    await api.setSetting(key, value);
  }

  return (
    <>
      <div className="topbar">
        <h1>Settings</h1>
      </div>
      <div className="content stack">
        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Appearance</h3>
          <SelectField
            label="Scheme"
            value={themeMode}
            onChange={(e) => {
              const mode = e.target.value as ThemeMode;
              setThemeMode(mode);
              void persist("theme", mode);
            }}
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </SelectField>
          <Switch
            label="High contrast"
            checked={contrast === "high"}
            onChange={(on) => {
              const next: Contrast = on ? "high" : "normal";
              setContrast(next);
              void persist("contrast", next);
            }}
          />
          <p className="muted">
            Accent: {presetById(themePresetId).name} · rank {supportRank}. Accents
            and badges are the only paid surface.
          </p>
          <div>
            <Button variant="tonal" small onClick={() => setRoute("shop")}>
              Open theme shop
            </Button>
          </div>
        </section>

        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Java</h3>
          <p className="muted">
            {memory
              ? `${memory.totalMb} MB system memory. Recommended ${memory.recommendedMb} MB.`
              : "Detecting memory…"}
          </p>
          <SelectField
            label="Discovered runtimes"
            value={javaPath}
            onChange={(e) => {
              setJavaPath(e.target.value);
              void persist("java_path", e.target.value);
            }}
          >
            <option value="">Use PATH</option>
            {javas.map((j) => (
              <option key={j.path} value={j.path}>
                {j.version} — {j.path}
              </option>
            ))}
          </SelectField>
          <TextField
            label="Custom java path"
            value={javaPath}
            onChange={(e) => setJavaPath(e.target.value)}
            onBlur={() => void persist("java_path", javaPath)}
          />
          <label className="field">
            <span>Memory ({memoryMb} MB)</span>
            <input
              type="range"
              min={512}
              max={Math.max(memory?.totalMb ? memory.totalMb - 1024 : 8192, 1024)}
              step={256}
              value={memoryMb}
              onChange={(e) => setMemoryMb(Number(e.target.value))}
              onMouseUp={() => void persist("memory_mb", String(memoryMb))}
            />
          </label>
          <TextField
            label="Extra JVM arguments"
            value={jvmArgs}
            onChange={(e) => setJvmArgs(e.target.value)}
            onBlur={() => void persist("jvm_args", jvmArgs)}
          />
          <Button variant="outline" small onClick={() => void api.discoverJava().then(setJavas)}>
            Rediscover Java
          </Button>
        </section>

        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Network</h3>
          <TextField
            label="HTTP(S) proxy"
            placeholder="http://127.0.0.1:7890"
            value={proxy}
            onChange={(e) => setProxy(e.target.value)}
            onBlur={() => void persist("proxy_url", proxy)}
          />
          <TextField
            label="Instances root"
            value={instancesRoot}
            onChange={(e) => setInstancesRoot(e.target.value)}
            onBlur={() => void persist("instances_root", instancesRoot)}
          />
        </section>

        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Content</h3>
          <Switch
            label="Add Content"
            checked={addContent}
            onChange={(on) => {
              setAddContent(on);
              void persist("add_content", on ? "1" : "0");
            }}
          />
          <p className="muted">
            Required later for resource packs, shaders, and modpacks on an
            instance. Mods can be added without this. Pack install is not
            available yet.
          </p>
        </section>

        <section className="settings-section stack">
          <h3 style={{ margin: 0 }}>Privacy</h3>
          <Switch label="Telemetry (always off in MVP)" checked={false} onChange={() => undefined} />
          <p className="muted">No analytics, no ads, no behavioral tracking.</p>
        </section>
      </div>
    </>
  );
}
