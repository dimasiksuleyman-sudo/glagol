import { invoke, Channel } from "@tauri-apps/api/core";

/**
 * Progress events emitted by the Rust `synthesize_document` command
 * over a [`tauri::ipc::Channel`].
 *
 * Mirrors the Rust enum tagged with
 * `#[serde(tag = "kind", rename_all = "camelCase")]` in
 * `src-tauri/src/commands/synthesize.rs`. The discriminated union
 * shape lets callers narrow with `switch (event.kind)`.
 */
export type ProgressEvent =
  | { kind: "chunked"; total: number }
  | { kind: "synthesizingChunk"; current: number; total: number }
  | { kind: "joining" };

/**
 * Persist the SaluteSpeech Authorization Key in the OS keyring.
 * Resets the backend-side cached `SaluteAuth` so the next operation
 * picks up the new key.
 */
export async function setCredentials(authKey: string): Promise<void> {
  await invoke("set_credentials", { authKey });
}

/**
 * Check that credentials are usable.
 *
 * - `force = false` (the default — mount-time probe path): if the
 *   backend has already authenticated this process lifetime, returns
 *   immediately without contacting Sberbank. This protects the
 *   `CredentialsContext` mount-time probe from transient network
 *   errors (Ctrl+R refresh of the dev WebView, page navigation)
 *   falsely mapping a valid key to `"invalid"`.
 * - `force = true` (the Settings → Test button path): bypass the
 *   cache and perform a real OAuth handshake. Used when the user
 *   explicitly asks to revalidate.
 *
 * Rejects with a string error otherwise (no credentials, network
 * failure, invalid AK, etc.).
 */
export async function testCredentials(force = false): Promise<void> {
  await invoke("test_credentials", { force });
}

/**
 * Remove the stored Authorization Key. Idempotent on the backend —
 * resolves cleanly even if there was nothing to delete.
 */
export async function deleteCredentials(): Promise<void> {
  await invoke("delete_credentials");
}

/**
 * Run the full synthesis pipeline (chunker → loop synthesize →
 * wav_join), persist the result into the local library, and resolve
 * with the freshly minted `document_id` (UUID v4).
 *
 * Progress flows through a Tauri `Channel<ProgressEvent>`; the caller
 * receives `Chunked`, then one `SynthesizingChunk` per chunk, and
 * finally `Joining` before resolution.
 *
 * Audio bytes do not cross the IPC boundary — they're written directly
 * to `%LOCALAPPDATA%\<bundle>\audio_cache\{document_id}.wav` by the
 * backend, inside the same transaction that inserts the library row.
 * Use {@link exportAudio} when the user wants a copy at a path they
 * pick themselves.
 */
export async function synthesizeDocument(
  text: string,
  voice: string,
  onProgress: (event: ProgressEvent) => void,
): Promise<string> {
  const channel = new Channel<ProgressEvent>();
  channel.onmessage = onProgress;
  return await invoke<string>("synthesize_document", {
    text,
    voice,
    onProgress: channel,
  });
}

/**
 * Resolve the absolute filesystem path of a document's cached audio,
 * ready to be handed to the Tauri asset protocol for playback.
 *
 * Rejects with a string error if the document does not exist or its
 * `audio_path` column is `NULL` (Sprint 4 error-row case).
 */
export async function getAudioPath(documentId: string): Promise<string> {
  return await invoke<string>("get_audio_path", { documentId });
}

/**
 * Copy a document's cached audio to `destPath` (typically chosen by
 * the user via `dialog.save()`). Backend uses `fs::copy`, so the
 * original cached file stays in place; this is a pure export, not a
 * move.
 */
export async function exportAudio(documentId: string, destPath: string): Promise<void> {
  await invoke("export_audio", { documentId, destPath });
}

/**
 * A persisted library document. Mirrors `db::repository::DocumentRecord`
 * on the Rust side; serde uses field names as-is (snake_case) so the
 * shape lines up 1:1 over the IPC boundary.
 *
 * Nullable columns are typed `T | null` (not `T | undefined`) because
 * that's what `Option::None` serialises to via serde_json.
 */
export interface DocumentRecord {
  id: string;
  title: string;
  source_type: string;
  char_count: number;
  voice: string;
  status: string;
  error_message: string | null;
  /** Unix epoch milliseconds. */
  created_at: number;
  /** File name relative to `audio_cache_root`. `null` for status='error' rows (Sprint 4). */
  audio_path: string | null;
  audio_duration_ms: number | null;
}

/**
 * List every persisted document, most recently created first.
 */
export async function listDocuments(): Promise<DocumentRecord[]> {
  return await invoke<DocumentRecord[]>("list_documents");
}

/**
 * Delete a document from the library: removes the DB row and
 * best-effort removes the cached audio file from disk. Resolves
 * even if the file was already missing (Sprint 5 orphan cleanup
 * will handle stragglers).
 */
export async function deleteDocument(documentId: string): Promise<void> {
  await invoke("delete_document", { documentId });
}

/**
 * Outcome of a successful file parse. Mirrors `parser::ParsedDocument`
 * on the Rust side; serde uses field names as-is (snake_case) so the
 * shape lines up 1:1 over the IPC boundary.
 *
 * `is_scanned_pdf` is the only non-obvious field: it is `true` only
 * when the PDF parser extracted no usable text (typical of image-only
 * scanned documents). The frontend uses it to show the OCR disclaimer
 * dialog instead of loading an empty textarea.
 */
export interface ParsedDocument {
  text: string;
  is_scanned_pdf: boolean;
  /** `"txt" | "md" | "docx" | "pdf"` */
  source_format: string;
}

/**
 * Read a file from disk and parse it according to its extension
 * (TXT/MD/DOCX/PDF). Unknown extensions fall through to a try-all
 * dispatcher (the "Все файлы" escape hatch in the file picker).
 *
 * Two server-side limits are enforced before the parsed content is
 * returned:
 *
 * - **10 MB** file size — rejected pre-parse via `fs::metadata`
 * - **500 000** characters of extracted text — rejected post-parse
 *   via `chars().count()` (Cyrillic counted per letter, not per byte)
 *
 * Failures (file too big, content too long, parse error, missing
 * file) reject with a Russian-language string suitable for direct
 * toast display.
 */
export async function readAndParseFile(path: string): Promise<ParsedDocument> {
  return await invoke<ParsedDocument>("read_and_parse_file", { path });
}

export { Channel };
