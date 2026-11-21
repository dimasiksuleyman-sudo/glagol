import { emitTo } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

interface InsertRequest {
  id: string;
  text: string;
  expires_at_ms: number;
}

export function insertIntoFocusedField(text: string): boolean {
  const field = document.activeElement;
  const textarea = field instanceof HTMLTextAreaElement;
  if (!document.hasFocus() || (!textarea && !(field instanceof HTMLInputElement))) {
    return false;
  }
  if (field.disabled || field.readOnly || (!textarea && field.type === "password")) {
    return false;
  }

  const { selectionStart, selectionEnd, value, maxLength } = field;
  if (selectionStart === null || selectionEnd === null) return false;
  const insertion = text.replace(/\r\n?|\n/g, textarea ? "\n" : " ");
  const next = value.slice(0, selectionStart) + insertion + value.slice(selectionEnd);
  if (maxLength >= 0 && next.length > maxLength) return false;

  // React tracks assignments to the element's own setter. Use the native
  // setter so the bubbling input event publishes the change to React state.
  const prototype = textarea ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  const setValue = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
  if (!setValue) return false;
  setValue.call(field, next);
  const caret = selectionStart + insertion.length;
  field.setSelectionRange(caret, caret);
  field.dispatchEvent(new Event("input", { bubbles: true }));
  return true;
}

/** The main webview owns this listener for its whole lifetime. */
export async function listenForDictationInsert() {
  await getCurrentWebviewWindow().listen<InsertRequest>("dictation-insert-in-app", ({ payload }) => {
    const inserted = Date.now() < payload.expires_at_ms && insertIntoFocusedField(payload.text);
    void emitTo({ kind: "App" }, "dictation-insert-result", { id: payload.id, inserted })
      .catch(() => undefined);
  });
}
