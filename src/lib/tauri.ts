import { invoke, Channel } from "@tauri-apps/api/core";

/**
 * Progress events emitted by the Rust `synthesize_document` command
 * over a [`tauri::ipc::Channel`].
 *
 * Mirrors the Rust enum tagged with `#[serde(tag = "kind", rename_all = "camelCase")]`
 * in `src-tauri/src/commands/synthesize.rs`. Discriminated union shape
 * lets callers narrow with `switch (event.kind)`.
 */
export type ProgressEvent =
  | { kind: "chunked"; total: number }
  | { kind: "synthesizingChunk"; current: number; total: number }
  | { kind: "joining" };

/** Skeleton — real implementation lands in Phase 3. */
export async function setCredentials(_authKey: string): Promise<void> {
  throw new Error("setCredentials not implemented (Phase 3)");
}

/** Skeleton — real implementation lands in Phase 3. */
export async function testCredentials(): Promise<void> {
  throw new Error("testCredentials not implemented (Phase 3)");
}

/** Skeleton — real implementation lands in Phase 3. */
export async function deleteCredentials(): Promise<void> {
  throw new Error("deleteCredentials not implemented (Phase 3)");
}

/**
 * Skeleton — real implementation lands in Phase 3. The final body will
 * wire a `Channel<ProgressEvent>` through `invoke` and return a binary
 * payload (via `tauri::ipc::Response` once the backend tweak lands).
 */
export async function synthesizeDocument(
  _text: string,
  _voice: string,
  _onProgress: (event: ProgressEvent) => void,
): Promise<Uint8Array> {
  throw new Error("synthesizeDocument not implemented (Phase 3)");
}

/** Skeleton — real implementation lands in Phase 3. */
export async function writeWavFile(_path: string, _bytes: Uint8Array): Promise<void> {
  throw new Error("writeWavFile not implemented (Phase 3)");
}

// Re-export so consumers don't need to import from @tauri-apps/api/core directly.
// (Phase 3 will use these in real wrapper bodies.)
export { invoke, Channel };
