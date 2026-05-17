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
 * wav_join) and return the resulting WAV bytes.
 *
 * Progress flows through a Tauri `Channel<ProgressEvent>`; the caller
 * receives `Chunked`, then one `SynthesizingChunk` per chunk, and
 * finally `Joining` before resolution.
 *
 * The Rust side returns `tauri::ipc::Response`, which Tauri delivers
 * as an `ArrayBuffer` on the JS side — avoiding the multi-MB JSON
 * array-of-numbers round-trip a plain `Vec<u8>` would incur.
 */
export async function synthesizeDocument(
  text: string,
  voice: string,
  onProgress: (event: ProgressEvent) => void,
): Promise<Uint8Array> {
  const channel = new Channel<ProgressEvent>();
  channel.onmessage = onProgress;
  const buffer = await invoke<ArrayBuffer>("synthesize_document", {
    text,
    voice,
    onProgress: channel,
  });
  return new Uint8Array(buffer);
}

/**
 * Write WAV bytes to a path the user just chose via `dialog.save()`.
 *
 * Bytes are serialised as a JSON array of numbers because the Tauri 2
 * raw-body API for commands would require encoding the path in an HTTP
 * header — uglier than the wire-cost overhead is worth at MVP scale.
 * A 21 MB WAV takes about a second to serialise on top of the disk
 * write; revisit in Sprint 4 if real-world docs exceed that.
 */
export async function writeWavFile(path: string, bytes: Uint8Array): Promise<void> {
  await invoke("write_wav_file", {
    path,
    bytes: Array.from(bytes),
  });
}

export { Channel };
