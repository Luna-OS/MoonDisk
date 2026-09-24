import { useEffect, useRef } from "react";

/**
 * Confirmation dialog for critical (delete/format) operations. Deliberately
 * plain and serious — no mascot, no playful copy: this is where MoonDisk
 * must be at its most unambiguous, not its most charming.
 *
 * Uses the native <dialog> element for built-in modal semantics (focus
 * containment, Escape-to-close, backdrop) rather than a hand-rolled focus
 * trap.
 */
export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  consequence: string;
  targetSummary: string;
  busy?: boolean;
  error?: string | null;
  onCancel: () => void;
  onConfirm: () => void;
}

export function ConfirmDialog({
  open,
  title,
  consequence,
  targetSummary,
  busy,
  error,
  onCancel,
  onConfirm,
}: ConfirmDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const confirmButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      confirmButtonRef.current?.focus();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  return (
    <dialog
      ref={ref}
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      onClose={onCancel}
      className="rounded-lg border p-0 text-(--md-color-text) bg-(--md-color-surface) border-(--md-color-error) backdrop:bg-black/60"
    >
      <form
        method="dialog"
        className="flex w-[min(90vw,32rem)] flex-col gap-4 p-6"
        onSubmit={(e) => {
          e.preventDefault();
          onConfirm();
        }}
      >
        <h2 className="text-lg font-semibold text-(--md-color-error)">{title}</h2>
        <p className="text-sm whitespace-pre-line">{targetSummary}</p>
        <p className="text-sm font-medium">{consequence}</p>

        {error && (
          <p role="alert" className="text-sm text-(--md-color-error)">
            {error}
          </p>
        )}

        <div className="mt-2 flex justify-end gap-3">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md border px-4 py-2 text-sm border-(--md-color-surface-border)"
          >
            Abbrechen
          </button>
          <button
            ref={confirmButtonRef}
            type="submit"
            disabled={busy}
            className="rounded-md px-4 py-2 text-sm font-medium text-white bg-(--md-color-error) disabled:opacity-40"
          >
            {busy ? "Wird ausgeführt …" : "Ja, wirklich ausführen"}
          </button>
        </div>
      </form>
    </dialog>
  );
}
