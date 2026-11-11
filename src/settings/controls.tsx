import { t } from "../i18n";
import { cloneElement, useId, type ReactElement, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import type { EngineSetup, Health, Settings } from "../types";

/** State and helpers every settings section receives from the modal. */
export type SectionProps = {
  draft: Settings;
  update: (patch: Partial<Settings>) => void;
  busy: boolean;
  health: Health | null;
  setError: (error: string) => void;
};

export type EngineControls = {
  setup: EngineSetup | null;
  /** True while a model download request is being sent or running. */
  working: boolean;
  download: (model: string, preview?: boolean, previewMode?: string) => Promise<void>;
};

/** A titled block of related settings, rendered as one card. */
export function Group({ title, description, children }: { title?: string; description?: ReactNode; children: ReactNode }) {
  return (
    <section className="settings-group">
      {(title || description) && <header>
        {title && <h4>{title}</h4>}
        {description && <p>{description}</p>}
      </header>}
      <div className="settings-card">{children}</div>
    </section>
  );
}

/** Label and optional hint on the left, the control on the right. */
export function Field({ label, hint, children, stacked = false }: { label: ReactNode; hint?: ReactNode; children: ReactElement<{ id?: string; "aria-describedby"?: string }>; stacked?: boolean }) {
  const id = useId(), hintId = `${id}-hint`;
  return (
    <div className={`setting-row ${stacked ? "stacked" : ""}`}>
      <div className="setting-copy">
        <label htmlFor={id}>{label}</label>
        {hint && <small id={hintId}>{hint}</small>}
      </div>
      <div className="setting-control">{cloneElement(children, { id, "aria-describedby": hint ? hintId : undefined })}</div>
    </div>
  );
}

export function Switch({ label, hint, checked, disabled, onChange }: { label: ReactNode; hint?: ReactNode; checked: boolean; disabled?: boolean; onChange: (checked: boolean) => void }) {
  const id = useId(), hintId = `${id}-hint`;
  return (
    <div className="setting-row switch-row">
      <div className="setting-copy">
        <label htmlFor={id}>{label}</label>
        {hint && <small id={hintId}>{hint}</small>}
      </div>
      <input id={id} type="checkbox" role="switch" className="switch" checked={checked} disabled={disabled}
        aria-describedby={hint ? hintId : undefined} onChange={event => onChange(event.target.checked)} />
    </div>
  );
}

export type OptionCard<T extends string> = { value: T; title: string; description?: ReactNode; badge?: string; disabled?: boolean };

/** Mutually exclusive choices presented as selectable cards (a radio group). */
export function OptionCards<T extends string>({ label, value, options, onChange, disabled, compact = false }: {
  label: string; value: T | undefined; options: OptionCard<T>[]; onChange: (value: T) => void; disabled?: boolean; compact?: boolean;
}) {
  const name = useId();
  return (
    <div className={`option-cards ${compact ? "compact" : ""}`} role="radiogroup" aria-label={label}>
      {options.map(option => (
        <label key={option.value} className={`option-card ${value === option.value ? "selected" : ""}`}>
          <input type="radio" name={name} value={option.value} checked={value === option.value}
            disabled={disabled || option.disabled} onChange={() => onChange(option.value)} />
          <span>
            <strong>{option.title}{option.badge && <em className="option-badge">{option.badge}</em>}</strong>
            {option.description && <small>{option.description}</small>}
          </span>
        </label>
      ))}
    </div>
  );
}

/** Technical options stay available but collapsed, so the basics read first. */
export function Advanced({ children, label = t("Opzioni avanzate") }: { children: ReactNode; label?: string }) {
  return (
    <details className="settings-advanced">
      <summary><ChevronRight size={14} />{label}</summary>
      <div className="settings-advanced-body">{children}</div>
    </details>
  );
}

export function Hint({ children, tone }: { children: ReactNode; tone?: "warning" | "error" }) {
  return <p className={`settings-hint ${tone || ""}`} role={tone === "error" ? "alert" : undefined}>{children}</p>;
}

export function Actions({ children }: { children: ReactNode }) {
  return <div className="settings-actions">{children}</div>;
}

export function Progress({ label, value, max, text }: { label: string; value: number; max: number; text: string }) {
  return (
    <div className="settings-progress" role="status">
      <progress aria-label={label} value={value} max={max || 1} />
      <span>{text}</span>
    </div>
  );
}
