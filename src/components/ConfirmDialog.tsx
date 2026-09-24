import { useEffect, useId, useRef, useState } from "react";
import { expectedPhrase, phraseMatches } from "@/lib/confirmation";

/**
 * Confirmation dialog for critical (delete/format) operations. Deliberately
 * plain and serious — no mascot, no playful copy — see docs/branding.md
 * §7 and docs/safety-model.md §8: this is where MoonDisk must be at its
 * most unambiguous, not its most charming.
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
  language: string;
  busy?: boolean;
  error?: string | null;
  onCancel: () => void;
  onConfirm: (phrase: string) => void;
}

export function ConfirmDialog({
  open,
  title,
  consequence,
  targetSummary,
  language,
  busy,
  error,
  onCancel,
  onConfirm,
}: ConfirmDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const [phrase, setPhrase] = useState("");
  const inputId = useId();

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      setPhrase("");
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  const matches = phraseMatches(phrase, language);

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
          if (matches) onConfirm(phrase);
        }}
      >
        <h2 className="text-lg font-semibold text-(--md-color-error)">{title}</h2>
        <p className="text-sm whitespace-pre-line">{targetSummary}</p>
        <p className="text-sm font-medium">{consequence}</p>

        <label htmlFor={inputId} className="text-sm">
          Gib <strong>{expectedPhrase(language)}</strong> ein, um fortzufahren:
        </label>
        <input
          id={inputId}
          // This is a native <dialog> opened via showModal(), and the
          // confirmation phrase field is meant to be ready for input
          // immediately (docs/safety-model.md §8, point 6).
          // eslint-disable-next-line jsx-a11y/no-autofocus
          autoFocus
          autoComplete="off"
          spellCheck={false}
          value={phrase}
          onChange={(e) => setPhrase(e.target.value)}
          className="rounded-md border px-3 py-2 bg-(--md-color-bg) border-(--md-color-surface-border) text-(--md-color-text)"
          aria-describedby={error ? `${inputId}-error` : undefined}
        />
        {error && (
          <p id={`${inputId}-error`} role="alert" className="text-sm text-(--md-color-error)">
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
            type="submit"
            disabled={!matches || busy}
            className="rounded-md px-4 py-2 text-sm font-medium text-white bg-(--md-color-error) disabled:opacity-40"
          >
            {busy ? "Wird ausgeführt …" : "Ausführen"}
          </button>
        </div>
      </form>
    </dialog>
  );
}
