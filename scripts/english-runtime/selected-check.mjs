// Current E1 candidate: Moonshine STT + Silero EN. Kokoro is historical only.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { readManifest, verifyBytes, safePath } from './verify.mjs';
const root=path.resolve(process.argv[2]||'.scratch/english-first');
const moonshine=readManifest();
for (const notice of JSON.parse(fs.readFileSync(new URL('notices.json', import.meta.url)))) {
  verifyBytes(notice, fs.readFileSync(new URL(`../../docs/third-party/${notice.file}`, import.meta.url)));
}
for(const a of [moonshine.runtime,...moonshine.runtime.members])verifyBytes(a,fs.readFileSync(safePath(root,a.file)));
for(const a of moonshine.artifacts.filter(a=>a.kind==='stt')) {
  if(moonshine.licenses[a.license]?.spdx!=='MIT')throw Error('STT license not pinned');
  verifyBytes(a,fs.readFileSync(safePath(path.join(root,'assets'),a.file)));
}
const en=JSON.parse(fs.readFileSync(new URL('../silero/english-model.json',import.meta.url),'utf8'));
verifyBytes(en,fs.readFileSync(safePath(root,en.file)));
const license=fs.readFileSync(new URL('../../docs/third-party/Silero-LICENSE.txt',import.meta.url));
if(createHash('sha256').update(license).digest('hex')!==en.license.sha256||en.license.spdx!=='CC-BY-NC-SA-4.0')throw Error('Silero license mismatch');
const runtime=JSON.parse(fs.readFileSync(new URL('../silero/runtime-manifest.json',import.meta.url),'utf8'));
if(runtime.id!==en.runtime)throw Error('Shared runtime mismatch');
console.log('SELECTED EN ARTIFACTS PASS: Moonshine STT + Silero EN; direct Silero origin availability is a separate check');
