import { useEffect, useState } from "react";
import { api } from "../api/client";
import type { Instance, JavaInstall, MemoryInfo } from "../api/types";
import { Button, Callout, Dialog, SelectField, Switch, TextField } from "./ui";

export function InstanceSettings({
  instance,
  onClose,
  onSaved,
}: {
  instance: Instance;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = useState(instance.name);
  const [memoryMb, setMemoryMb] = useState(instance.memoryMb);
  const [jvmArgs, setJvmArgs] = useState(instance.jvmArgs ?? "");
  const [javaPath, setJavaPath] = useState(instance.javaPath ?? "");
  const [keepOpen, setKeepOpen] = useState(instance.keepOpen);
  const [javas, setJavas] = useState<JavaInstall[]>([]);
  const [memory, setMemory] = useState<MemoryInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setName(instance.name);
    setMemoryMb(instance.memoryMb);
    setJvmArgs(instance.jvmArgs ?? "");
    setJavaPath(instance.javaPath ?? "");
    setKeepOpen(instance.keepOpen);
  }, [instance]);

  useEffect(() => {
    void api.discoverJava().then(setJavas).catch(() => undefined);
    void api.getSystemMemory().then(setMemory).catch(() => undefined);
  }, []);

  const maxMb = Math.max(memory?.totalMb ? memory.totalMb - 1024 : 16384, 1024);

  async function save() {
    const trimmed = name.trim();
    if (!trimmed) {
      setError("Name is required.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.updateInstance(instance.id, {
        name: trimmed,
        memoryMb,
        jvmArgs: jvmArgs.trim() ? jvmArgs.trim() : null,
        javaPath: javaPath.trim() ? javaPath.trim() : null,
        keepOpen,
      });
      onSaved();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title="Instance settings" onClose={onClose}>
      <div className="instance-settings stack">
        <p className="muted" style={{ margin: 0 }}>
          {instance.loader} · {instance.gameVersion}
          {memory
            ? ` · ${memory.totalMb} MB system memory`
            : ""}
        </p>
        <TextField
          label="Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <label className="field">
          <span>Memory ({memoryMb} MB)</span>
          <input
            type="range"
            min={512}
            max={maxMb}
            step={256}
            value={Math.min(memoryMb, maxMb)}
            onChange={(e) => setMemoryMb(Number(e.target.value))}
          />
        </label>
        <TextField
          label="JVM arguments"
          value={jvmArgs}
          placeholder="Optional extras for this instance"
          onChange={(e) => setJvmArgs(e.target.value)}
        />
        <SelectField
          label="Java runtime"
          value={javaPath}
          onChange={(e) => setJavaPath(e.target.value)}
        >
          <option value="">Use global / PATH</option>
          {javas.map((j) => (
            <option key={j.path} value={j.path}>
              {j.version} — {j.path}
            </option>
          ))}
          {javaPath && !javas.some((j) => j.path === javaPath) ? (
            <option value={javaPath}>Custom — {javaPath}</option>
          ) : null}
        </SelectField>
        <TextField
          label="Java path override"
          value={javaPath}
          placeholder="Leave empty for global default"
          onChange={(e) => setJavaPath(e.target.value)}
        />
        <div className="row">
          <Button
            variant="outline"
            small
            onClick={() => void api.discoverJava().then(setJavas)}
          >
            Rediscover Java
          </Button>
        </div>
        <Switch label="Keep launcher open" checked={keepOpen} onChange={setKeepOpen} />
        {error ? <Callout>{error}</Callout> : null}
        <div className="row">
          <Button onClick={() => void save()} disabled={busy || !name.trim()}>
            Save
          </Button>
          <Button variant="text" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
