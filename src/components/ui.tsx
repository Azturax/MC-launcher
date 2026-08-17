import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  PointerEvent,
  ReactNode,
  SelectHTMLAttributes,
} from "react";

type BtnVariant = "filled" | "tonal" | "outline" | "text" | "danger";
type TooltipSide = "top" | "bottom" | "left" | "right";

export function Button({
  variant = "filled",
  small,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: BtnVariant; small?: boolean }) {
  return (
    <button
      className={`btn btn-${variant} ${small ? "btn-sm" : ""} ${className}`}
      {...props}
    />
  );
}

/** Icon-only action with M3 tooltip + matching aria-label. */
export function IconButton({
  label,
  variant = "filled",
  small,
  side = "top",
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  variant?: BtnVariant;
  small?: boolean;
  side?: TooltipSide;
}) {
  return (
    <button
      type="button"
      {...props}
      className={`btn btn-${variant} btn-icon with-tooltip ${small ? "btn-sm" : ""} ${className}`}
      data-tooltip={label}
      data-tooltip-side={side}
      aria-label={label}
    >
      {children}
    </button>
  );
}

export function TextField({
  label,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { label: string }) {
  return (
    <label className="field">
      <span>{label}</span>
      <input {...props} />
    </label>
  );
}

export function SelectField({
  label,
  children,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement> & { label: string; children: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      <select {...props}>{children}</select>
    </label>
  );
}

export function Switch({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <label className="switch">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

export function Dialog({
  title,
  children,
  onClose,
}: {
  title: string;
  children: ReactNode;
  onClose?: () => void;
}) {
  function closeIfBackdrop(e: PointerEvent<HTMLDivElement>) {
    // Only the dimmed overlay — never the panel or its children.
    // Using pointerdown + target check avoids native <select> option
    // clicks "falling through" and dismissing the dialog.
    if (!onClose) return;
    if (e.target === e.currentTarget) onClose();
  }

  return (
    <div
      className="dialog-backdrop"
      onPointerDown={closeIfBackdrop}
      role="presentation"
    >
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="dialog-header">
          <h2>{title}</h2>
          {onClose ? (
            <button
              type="button"
              className="btn btn-text btn-icon btn-sm"
              aria-label="Close"
              onClick={onClose}
            >
              ✕
            </button>
          ) : null}
        </div>
        {children}
      </div>
    </div>
  );
}

export function Callout({
  children,
  tone = "danger",
}: {
  children: ReactNode;
  tone?: "danger" | "info" | "warn";
}) {
  const extra = tone === "info" ? "info" : tone === "warn" ? "warn" : "";
  return <div className={`callout ${extra}`}>{children}</div>;
}
