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
 * Perform a real OAuth handshake against Sberbank to confirm the
 * stored key works. Resolves on success; rejects with a string error
 * otherwise (no credentials, network failure, invalid AK, etc.).
 */
export async function testCredentials(): Promise<void> {
  await invoke("test_credentials");
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

export { Channel };
