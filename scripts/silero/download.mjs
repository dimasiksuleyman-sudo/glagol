// Development bootstrap: download only pinned runtime artifacts, never a model.
// Usage: node scripts/silero/download.mjs .scratch/silero/downloads
import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdir, readFile, rename, rm, stat } from "node:fs/promises";
import { resolve, basename } from "node:path";
import { Readable, Transform } from "node:stream";
import { pipeline } from "node:stream/promises";

const manifest = JSON.parse(await readFile(new URL("./runtime-manifest.json", import.meta.url), "utf8"));
const directory = resolve(process.argv[2] ?? ".scratch/silero/downloads");
await mkdir(directory, { recursive: true });

async function verified(path, artifact) {
  try {
    if ((await stat(path)).size !== artifact.bytes) return false;
    const hash = createHash("sha256");
    for await (const chunk of createReadStream(path)) hash.update(chunk);
    return hash.digest("hex") === artifact.sha256;
  } catch { return false; }
}

for (const artifact of manifest.artifacts) {
  if (basename(artifact.file) !== artifact.file || !artifact.url.startsWith("https://")) throw new Error("Invalid manifest");
  const target = resolve(directory, artifact.file);
  if (await verified(target, artifact)) {
    console.log(`${artifact.name}: verified`);
    continue;
  }
  const partial = target + ".part";
  try {
    const response = await fetch(artifact.url, { signal: AbortSignal.timeout(300_000) });
    if (!response.ok || !response.body) throw new Error(`HTTP ${response.status}`);
    let received = 0;
    const limiter = new Transform({ transform(chunk, _, done) {
      received += chunk.length;
      done(received > artifact.bytes ? new Error("Artifact exceeds pinned size") : null, chunk);
    } });
    await pipeline(Readable.fromWeb(response.body), limiter, createWriteStream(partial));
    if (!await verified(partial, artifact)) throw new Error("Size or SHA-256 mismatch");
    await rename(partial, target);
    console.log(`${artifact.name}: downloaded and verified`);
  } catch (error) {
    await rm(partial, { force: true });
    throw new Error(`${artifact.name}: ${error.message}`);
  }
}
console.log(`Runtime download: ${manifest.artifacts.reduce((sum, a) => sum + a.bytes, 0)} bytes. Model excluded.`);
