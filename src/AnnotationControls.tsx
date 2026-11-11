import { t } from "./i18n";
import { useRef, useState } from "react";
import type { CSSProperties, PointerEvent } from "react";
import { Check, ChevronDown, Plus } from "lucide-react";
const swatches = [
  "#ff565f",
  "#ffad45",
  "#ffd95b",
  "#6ee7aa",
  "#57b8ff",
  "#aa8bff",
  "#f58cc7",
  "#ffffff",
  "#18212d",
];
function hsv(hex: string): [number, number, number] {
  const [r, g, b] = [1, 3, 5].map(
      (i) => parseInt(hex.slice(i, i + 2), 16) / 255,
    ),
    max = Math.max(r, g, b),
    min = Math.min(r, g, b),
    d = max - min;
  const hue = !d
    ? 0
    : max === r
      ? ((g - b) / d + 6) % 6
      : max === g
        ? (b - r) / d + 2
        : (r - g) / d + 4;
  return [hue * 60, max ? d / max : 0, max];
}
function hex(h: number, s: number, v: number) {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round((v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(5)}${f(3)}${f(1)}`;
}
export function ColorControls({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const [custom, setCustom] = useState(false),
    [draft, setDraft] = useState(value),
    [tone, setTone] = useState(() => hsv(value));
  const pad = useRef<HTMLDivElement>(null);
  const select = (color: string) => {
    onChange(color);
    setDraft(color);
    setTone(hsv(color));
  };
  const update = (next: [number, number, number]) => {
    setTone(next);
    const color = hex(...next);
    setDraft(color);
    onChange(color);
  };
  const point = (event: PointerEvent<HTMLDivElement>) => {
    const rect = pad.current!.getBoundingClientRect();
    update([
      tone[0],
      Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)),
      1 - Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height)),
    ]);
  };
  return (
    <div className="annotation-colors">
      <div className="annotation-popover-label">
        {t("Colore") + " "}<span>{value.toUpperCase()}</span>
      </div>
      <div className="annotation-swatches">
        {swatches.map((color) => (
          <button
            key={color}
            type="button"
            className={value.toLowerCase() === color ? "selected" : ""}
            style={{ "--swatch": color } as CSSProperties}
            aria-label={t("Colore {0}", color)}
            aria-pressed={value.toLowerCase() === color}
            onClick={() => select(color)}
          >
            {value.toLowerCase() === color && <Check size={13} />}
          </button>
        ))}
        <button
          type="button"
          className="custom-swatch"
          title={t("Colore personalizzato")}
          aria-label={t("Colore personalizzato")}
          aria-expanded={custom}
          onClick={() => {
            setCustom(!custom);
            setTone(hsv(value));
            setDraft(value);
          }}
        >
          <Plus size={14} />
        </button>
      </div>
      {custom && (
        <div className="annotation-custom-color">
          <div
            ref={pad}
            className="color-saturation"
            role="slider"
            tabIndex={0}
            aria-label={t("Saturazione e luminosità")}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(tone[1] * 100)}
            aria-valuetext={t("Saturazione {0}%, luminosità {1}%", Math.round(tone[1] * 100), Math.round(tone[2] * 100))}
            style={{ backgroundColor: `hsl(${tone[0]} 100% 50%)` }}
            onPointerDown={(e) => {
              e.currentTarget.setPointerCapture(e.pointerId);
              point(e);
            }}
            onPointerMove={(e) => {
              if (e.currentTarget.hasPointerCapture(e.pointerId)) point(e);
            }}
            onKeyDown={(e) => {
              if (!e.key.startsWith("Arrow")) return;
              e.preventDefault();
              e.stopPropagation();
              update([
                tone[0],
                Math.max(
                  0,
                  Math.min(
                    1,
                    tone[1] +
                      (e.key === "ArrowRight"
                        ? 0.02
                        : e.key === "ArrowLeft"
                          ? -0.02
                          : 0),
                  ),
                ),
                Math.max(
                  0,
                  Math.min(
                    1,
                    tone[2] +
                      (e.key === "ArrowUp"
                        ? 0.02
                        : e.key === "ArrowDown"
                          ? -0.02
                          : 0),
                  ),
                ),
              ]);
            }}
          >
            <i
              style={{
                left: `${tone[1] * 100}%`,
                top: `${(1 - tone[2]) * 100}%`,
                background: value,
              }}
            />
          </div>
          <input
            className="color-hue"
            type="range"
            min="0"
            max="359"
            value={Math.round(tone[0])}
            aria-label={t("Tonalità colore")}
            onChange={(e) => update([Number(e.target.value), tone[1], tone[2]])}
          />
          <label className="annotation-hex">
            <span>HEX</span>
            <input
              value={draft}
              maxLength={7}
              aria-label={t("Colore esadecimale")}
              onChange={(e) => {
                setDraft(e.target.value);
                if (/^#[0-9a-f]{6}$/i.test(e.target.value))
                  select(e.target.value);
              }}
              onBlur={() => setDraft(value)}
              spellCheck={false}
            />
            <i style={{ background: value }} />
          </label>
        </div>
      )}
    </div>
  );
}
export function SizeControls({
  value,
  onChange,
  text = false,
}: {
  value: number;
  onChange: (value: number) => void;
  text?: boolean;
}) {
  const presets = text ? [16, 24, 36, 48] : [2, 5, 10, 16];
  const min = text ? 12 : 1,
    max = text ? 72 : 30;
  return (
    <div className="annotation-size-controls">
      <div className="annotation-popover-label">
        {text ? t("Dimensione testo") : t("Spessore tratto")}
        <span>{value} px</span>
      </div>
      <div className="annotation-sizes">
        {presets.map((size) => (
          <button
            type="button"
            key={size}
            className={value === size ? "selected" : ""}
            onClick={() => onChange(size)}
            aria-label={`${size} pixel`}
            aria-pressed={value === size}
          >
            {text ? (
              <span style={{ fontSize: Math.min(size, 25) }}>Aa</span>
            ) : (
              <i style={{ width: size, height: size }} />
            )}
            <small>{size}</small>
          </button>
        ))}
      </div>
      <input
        className="annotation-range"
        type="range"
        aria-label={text ? t("Dimensione testo") : t("Spessore tratto")}
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={
          {
            "--range-progress": `${((value - min) / (max - min)) * 100}%`,
          } as CSSProperties
        }
      />
    </div>
  );
}
export function PropertyButton({
  label,
  children,
  open,
  onClick,
}: {
  label: string;
  children: React.ReactNode;
  open: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="annotation-property"
      aria-label={label}
      aria-expanded={open}
      onClick={onClick}
    >
      {children}
      <ChevronDown size={12} />
    </button>
  );
}
