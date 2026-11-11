import { t } from "./i18n";
import { useId, type ReactNode } from "react";
import { useDialog } from "./useDialog";

export default function ConfirmDialog({ title, children, confirmLabel, busy = false, danger = false, onConfirm, onCancel }: {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  busy?: boolean;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const titleId = useId();
  const dialog = useDialog<HTMLElement>(onCancel, ".secondary-button");
  return (
    <div className="modal-backdrop" onClick={onCancel}>
      <section className="confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby={titleId}
        ref={dialog.ref} onKeyDown={dialog.onKeyDown} onClick={event => event.stopPropagation()}>
        <h2 id={titleId}>{title}</h2>
        <div className="confirm-dialog-body">{children}</div>
        <footer>
          <button type="button" className="secondary-button" onClick={onCancel}>{t("Annulla")}</button>
          <button type="button" className={danger ? "danger-button" : "primary-button"} disabled={busy} onClick={onConfirm}>{confirmLabel}</button>
        </footer>
      </section>
    </div>
  );
}
