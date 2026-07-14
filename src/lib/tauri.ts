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
 * Rename a document. The title is trimmed at the IPC boundary; an
 * empty / whitespace-only value rejects with «Заголовок не может быть
 * пустым», an unknown document id rejects with «Документ не найден».
 * Both error strings are Russian and suitable for direct toast display.
 */
export async function updateDocumentTitle(documentId: string, title: string): Promise<void> {
  await invoke("update_document_title", { documentId, title });
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

/**
 * Metadata stored inside a Glagol backup `.zip` as `manifest.json`.
 * Mirrors `backup::BackupManifest` on the Rust side — serde uses field
 * names as-is (snake_case) so the shape lines up 1:1 over the IPC
 * boundary.
 *
 * `backup_version` is the on-disk format version; this build of Glagol
 * is `CURRENT_BACKUP_VERSION = 1` (see `src-tauri/src/backup/mod.rs`).
 * Restore refuses archives whose version exceeds the build's supported
 * version.
 */
export interface BackupManifest {
  backup_version: number;
  app_version: string;
  /** ISO 8601 UTC timestamp at backup creation time. */
  created_at: string;
  document_count: number;
  audio_file_count: number;
  total_size_bytes: number;
}

/**
 * Payload of the `backup-progress` and `backup-restore-progress` Tauri
 * events. `current` and `total` are file counts — manifest + db + each
 * audio file for create; db + each audio file for restore.
 */
export interface BackupProgressEvent {
  current: number;
  total: number;
}

/**
 * Channel name for "Создаю резервную копию" progress events. Frontend
 * `listen()`s for the duration of the create-backup modal. Keep this
 * string in lock-step with `BACKUP_PROGRESS_EVENT` in
 * `src-tauri/src/commands/backup.rs`.
 */
export const BACKUP_PROGRESS_EVENT = "backup-progress";

/**
 * Channel name for "Восстанавливаю" progress events. Keep in lock-step
 * with `BACKUP_RESTORE_PROGRESS_EVENT` in Rust.
 */
export const BACKUP_RESTORE_PROGRESS_EVENT = "backup-restore-progress";

/**
 * Create a full backup of the library — `glagol.db` + every cached
 * audio file + a `manifest.json` — into a freshly named zip in
 * `targetFolder`. The filename is generated by the backend
 * (`glagol-backup-YYYY-MM-DD-HHMMSS.zip`); the resolved absolute path
 * of the produced archive is returned so the success toast can show
 * just the filename.
 *
 * Progress events fire on the `BACKUP_PROGRESS_EVENT` channel — one
 * per file processed plus an initial `(0, total)` frame.
 */
export async function createBackup(targetFolder: string): Promise<string> {
  return await invoke<string>("create_backup", { targetFolder });
}

/**
 * Non-destructive pre-check on a candidate backup file. Reads the
 * manifest, validates the schema version + count consistency + that
 * no entry tries to escape the target directory, and returns the
 * parsed manifest so the confirm dialog can show how many documents
 * the archive contains.
 *
 * Rejects with a Russian-language string suitable for direct toast
 * display when the file is not a recognisable Glagol backup.
 */
export async function validateBackup(sourcePath: string): Promise<BackupManifest> {
  return await invoke<BackupManifest>("validate_backup", { sourcePath });
}

/**
 * Destructive restore. Sequence on the backend: re-validate → write a
 * `glagol-pre-restore-…zip` snapshot of the current library in the
 * same folder as `sourcePath` (Sprint 5c D3 Safety Net 2) → wipe
 * existing data → extract archive on top. If the pre-restore snapshot
 * fails the call aborts before any deletion, leaving the user's data
 * untouched.
 *
 * Progress events fire on the `BACKUP_RESTORE_PROGRESS_EVENT`
 * channel. Caller is expected to follow a successful resolution with
 * a `relaunchApp()` call so the next process picks up the freshly
 * extracted `glagol.db` on a clean setup hook.
 */
export async function restoreBackup(sourcePath: string): Promise<void> {
  await invoke("restore_backup", { sourcePath });
}

/**
 * Restart the application. Used by the restore flow to guarantee the
 * next Tauri setup hook opens the freshly extracted `glagol.db` from
 * scratch — the previous process's SQLite connection is replaced
 * along with everything else.
 *
 * Note: control does not return on success — the process is replaced
 * before this Promise can resolve. Implemented with an `invoke`
 * regardless so the type checker treats the call site as a regular
 * Tauri command (no special "may never resolve" semantics needed).
 */
export async function relaunchApp(): Promise<void> {
  await invoke("relaunch_app");
}

/**
 * Snapshot of the current month's SaluteSpeech consumption. Mirrors
 * `commands::usage::UsageInfo` on the Rust side — serde keeps the
 * field names as-is (snake_case) so the shape lines up 1:1 over the
 * IPC boundary, matching the `DocumentRecord` convention.
 *
 * `month` is the `YYYY-MM` key (local calendar month); the Settings UI
 * formats it for display via a local Russian month-name helper that
 * mirrors `commands::usage::russian_month_genitive`.
 *
 * `percent_used` is computed backend-side, capped at 100 so over-quota
 * paid-tier users still see a pinned bar (never a 125 % overflow).
 */
export interface UsageInfo {
  month: string;
  chars_used: number;
  chars_limit: number;
  percent_used: number;
}

/**
 * Read the current calendar month's SaluteSpeech usage. Called on
 * mount of the Settings page's "Использование SaluteSpeech" section,
 * and again whenever the backend emits `SYNTHESIS_COMPLETED_EVENT`.
 *
 * Rejects with a Russian-language string suitable for direct toast or
 * inline-error display when the DB read fails.
 */
export async function getCurrentMonthUsage(): Promise<UsageInfo> {
  return await invoke<UsageInfo>("get_current_month_usage");
}

/**
 * Payload of the `synthesis-completed` event. The backend emits this
 * after a successful `synthesize_document` call so the Settings usage
 * counter (and, optionally, the Library list) can refresh without
 * polling. Field names are camelCase because the Rust struct uses
 * `#[serde(rename_all = "camelCase")]`.
 */
export interface SynthesisCompletedEvent {
  documentId: string;
  charsAdded: number;
}

/**
 * Channel name for the post-synthesis broadcast. Kept in lock-step
 * with `SYNTHESIS_COMPLETED_EVENT` in `src-tauri/src/commands/synthesize.rs`.
 */
export const SYNTHESIS_COMPLETED_EVENT = "synthesis-completed";

/**
 * Non-secret Dictation (STT) provider configuration. Mirrors
 * `commands::dictation::SttSettings` on the Rust side — serde keeps the
 * field names as-is (snake_case) so the shape lines up 1:1 over the IPC
 * boundary, matching the `DocumentRecord` / `UsageInfo` convention.
 *
 * The API key is deliberately NOT part of this shape: it lives in the OS
 * keyring and never crosses IPC on a read. Use {@link hasSttKey} to learn
 * whether a key is stored, {@link setSttKey} to store one.
 */
export interface SttSettings {
  base_url: string;
  model: string;
  /** Empty string when no proxy is configured. */
  proxy: string;
  /** `"ru" | "en" | "auto"`. */
  language: string;
}

/**
 * Read the persisted STT settings, with backend defaults substituted for any
 * key that has never been saved (first run → AITunnel preset).
 */
export async function getSttSettings(): Promise<SttSettings> {
  return await invoke<SttSettings>("get_stt_settings");
}

/**
 * Validate and persist the STT settings. Rejects with a Russian-language
 * string (suitable for a toast) when `baseUrl`/`proxy`/`language` are
 * malformed — external `http://` endpoints, a proxy without a port, an
 * unknown language, etc. Nothing is written unless every field validates.
 */
export async function saveSttSettings(settings: SttSettings): Promise<void> {
  await invoke("save_stt_settings", {
    baseUrl: settings.base_url,
    model: settings.model,
    proxy: settings.proxy,
    language: settings.language,
  });
}

/**
 * Store the STT provider API key in the OS keyring. Resets the backend-side
 * "key validated" flag so the next {@link testSttKey} re-checks. Rejects for
 * an empty/whitespace-only key.
 */
export async function setSttKey(key: string): Promise<void> {
  await invoke("set_stt_key", { key });
}

/**
 * Remove the stored STT API key. Idempotent on the backend — resolves cleanly
 * even if there was nothing to delete (a keyless local server is valid).
 */
export async function deleteSttKey(): Promise<void> {
  await invoke("delete_stt_key");
}

/**
 * Whether an STT API key is currently stored in the keyring. Used to render
 * the key field's «сохранён» / «не задан» hint on mount.
 */
export async function hasSttKey(): Promise<boolean> {
  return await invoke<boolean>("has_stt_key");
}

/**
 * Validate the whole STT chain — key + endpoint + proxy — against the live
 * provider (persist settings first with {@link saveSttSettings}).
 *
 * - `force = false` (mount-time probe): returns immediately if the backend
 *   already validated the key this process lifetime.
 * - `force = true` (the Test button): bypass the cache and hit the provider.
 *   Runs the cheap `GET /models` probe, falling back to a tiny silent
 *   transcription for providers that do not expose `/models`.
 *
 * Rejects with a Russian-language string on any failure (bad key, no balance,
 * unreachable endpoint/proxy, …).
 */
export async function testSttKey(force = false): Promise<void> {
  await invoke("test_stt_key", { force });
}

export { Channel };
