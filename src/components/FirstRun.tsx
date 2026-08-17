import { Button, Callout, Dialog } from "./ui";

export function FirstRun({
  disclaimer,
  onAccept,
}: {
  disclaimer: string;
  onAccept: () => void;
}) {
  return (
    <Dialog title="Welcome to Aureum">
      <p className="muted">
        A local-first instance manager. Speed for casual players, power for
        modders. Minecraft is a game you already own — Aureum never bundles it.
      </p>
      <Callout>{disclaimer}</Callout>
      <p className="muted">
        No ads. No Bedrock. No cracked accounts. Telemetry stays off.
      </p>
      <div className="row">
        <Button onClick={onAccept}>I understand — continue</Button>
      </div>
    </Dialog>
  );
}
