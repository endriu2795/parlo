import { t } from "./i18n";
export const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
export const primaryModifier = isMac ? "Cmd" : "Ctrl";
export const defaultShortcut = (key: string) => `${isMac ? "Super" : "Ctrl"}+Shift+${key}`;
export const shortcutKeyLabel = (key: string) =>
  key === "Super" || key === "Meta" || key === "Command" ? (isMac ? "Cmd" : "Win")
    : key === "Space" ? t("Spazio") : key === "Alt" && isMac ? "Option" : key;
export const formatShortcut = (value: string) => value.split("+").map(shortcutKeyLabel).join("+");
