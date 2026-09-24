/**
 * Frontend mirror of src-tauri/src/security/confirmation.rs, used only to
 * drive the confirm button's enabled state. The backend re-checks this
 * itself and is the actual authority — see docs/architecture.md §7 ("Das
 * Backend vertraut dem Frontend nie").
 */
export const PHRASE_DE = "LÖSCHEN";
export const PHRASE_EN = "DELETE";

export function expectedPhrase(language: string): string {
  return language === "en" ? PHRASE_EN : PHRASE_DE;
}

export function phraseMatches(input: string, language: string): boolean {
  return input.normalize("NFC").trim() === expectedPhrase(language).normalize("NFC").trim();
}
