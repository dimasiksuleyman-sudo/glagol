// Development probe only. Nothing in this directory is bundled with Glagol.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

export const manifestPath = fileURLToPath(new URL('./artifacts.json', import.meta.url));
export function readManifest() { return JSON.parse(fs.readFileSync(manifestPath, 'utf8')); }
export function safePath(root, name) {
  if (!/^[a-zA-Z0-9_.\-/]+$/.test(name) || name.split('/').some(p => !p || p === '..' || p === '.') || path.isAbsolute(name)) throw Error(`Unsafe artifact path: ${name}`);
  return path.join(root, name);
}
export function validateManifest(m) {
  if (m.schema_version !== 1 || m.runtime_version !== '0.1.5' || m.abi_version !== 30000 || m.target !== 'windows-x86_64') throw Error('Unexpected runtime/ABI/target');
  if (!/^[a-f0-9]{40}$/.test(m.model_revision)) throw Error('Unpinned model revision');
  const files = [m.runtime, ...m.runtime.members, ...m.artifacts];
  const paths = new Set();
  for (const a of files) {
    safePath('.', a.file);
    if (paths.has(a.file)) throw Error(`Duplicate path: ${a.file}`);
    paths.add(a.file);
    if (!Number.isSafeInteger(a.bytes) || a.bytes <= 0 || !/^[a-f0-9]{64}$/.test(a.sha256)) throw Error(`Missing size/hash: ${a.file}`);
    if (a.url && new URL(a.url).protocol !== 'https:') throw Error(`Insecure URL: ${a.file}`);
  }
  for (const a of m.artifacts) {
    if (!['stt', 'tts'].includes(a.kind) || !m.licenses[a.license]) throw Error(`Missing kind/license: ${a.file}`);
    if (!a.url.startsWith(`https://huggingface.co/moonshine-ai/moonshine-voice-assets/resolve/${m.model_revision}/`)) throw Error(`Unpinned URL: ${a.file}`);
  }
}
export function verifyBytes(a, bytes) {
  if (bytes.length !== a.bytes || createHash('sha256').update(bytes).digest('hex') !== a.sha256) throw Error(`Integrity mismatch: ${a.file}`);
}
export function checkLicenses(m) {
  const unknown = [...new Set(m.artifacts.filter(a => !m.licenses[a.license].spdx).map(a => a.license))];
  if (unknown.length) throw Error(`License terms unresolved: ${unknown.join(', ')}`);
}
export function verify(root, m) {
  validateManifest(m);
  for (const a of [m.runtime, ...m.runtime.members]) verifyBytes(a, fs.readFileSync(safePath(root, a.file)));
  for (const a of m.artifacts) verifyBytes(a, fs.readFileSync(safePath(path.join(root, 'assets'), a.file)));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const mode = process.argv[2], m = readManifest();
    validateManifest(m);
    if (mode === 'licenses') { checkLicenses(m); console.log('LICENSES PASS'); }
    else if (mode === 'files' && process.argv[3]) { verify(path.resolve(process.argv[3]), m); console.log('ARTIFACT INTEGRITY PASS'); }
    else throw Error('Usage: node scripts/english-runtime/verify.mjs licenses | files <probe-root>');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
