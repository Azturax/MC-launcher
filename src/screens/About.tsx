import { api } from "../api/client";
import { Button, Callout } from "../components/ui";
import type { AppInfo } from "../api/types";

export function About({ info }: { info: AppInfo | null }) {
  return (
    <>
      <div className="topbar">
        <h1>About Aureum</h1>
      </div>
      <div className="content stack">
        <p>
          Aureum is a third-party instance manager. Philosophy: speed for casual
          players, power for modders. Version {info?.version ?? "0.1.0-dev"}{" "}
          (development build — see project RELEASE.md before treating this as a
          public release).
        </p>
        <Callout>{info?.disclaimer}</Callout>
        <p className="muted">
          Minecraft is a trademark of Mojang AB / Microsoft. Aureum does not
          bundle game jars or assets. Files are downloaded from official
          version manifests and loader metadata on your behalf. Optional
          support ranks sell only themes and badges — never launch, login, or
          the catalog.
        </p>
        <div className="row">
          <Button
            variant="outline"
            onClick={() => void api.openExternal("https://www.minecraft.net/en-us/eula")}
          >
            Minecraft EULA
          </Button>
          <Button
            variant="outline"
            onClick={() =>
              void api.openExternal("https://www.minecraft.net/en-us/usage-guidelines")
            }
          >
            Usage guidelines
          </Button>
        </div>
        {info ? (
          <p className="muted">
            Data directory: {info.dataDir}
            <br />
            Instances: {info.instancesRoot}
          </p>
        ) : null}
      </div>
    </>
  );
}
