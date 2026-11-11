import { t } from "./i18n";
import { EyeOff } from "lucide-react";
export function DockButton({
  onClick,
  disabled = false,
  nativeTooltip = true,
}: {
  onClick: () => void;
  disabled?: boolean;
  nativeTooltip?: boolean;
}) {
  return (
    <button
      type="button"
      className="annotation-icon-button dock-button"
      title={nativeTooltip ? t("Nascondi barra") : undefined}
      aria-label={t("Nascondi barra")}
      disabled={disabled}
      onClick={onClick}
    >
      <EyeOff size={18} />
    </button>
  );
}
