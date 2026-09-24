// Explicit developer-only download; never runs during build, startup or tests.
import fs from 'node:fs';
import path from 'node:path';
import { readManifest, safePath, validateManifest, verifyBytes } from './verify.mjs';

const root=path.resolve(process.argv[2] || '.scratch/english-first');
const m=readManifest();validateManifest(m);
// Kokoro/G2P remain in the historical manifest, never in the default download.
const artifacts = process.argv.includes('--historical-kokoro') ? m.artifacts : m.artifacts.filter(a => a.kind === 'stt');
for (const a of [m.runtime,...artifacts]) {
  const out=safePath(a===m.runtime?root:path.join(root,'assets'),a.file);
  if (fs.existsSync(out)) { verifyBytes(a,fs.readFileSync(out));continue; }
  fs.mkdirSync(path.dirname(out),{recursive:true});
  const response=await fetch(a.url);
  if(!response.ok)throw Error(`HTTP ${response.status}: ${a.file}`);
  // Bound memory before accepting the body. SDK archives/models are small here.
  const chunks=[];let size=0;
  for await (const chunk of response.body) {
    size+=chunk.length;if(size>a.bytes)throw Error(`Oversized download: ${a.file}`);
    chunks.push(chunk);
  }
  const bytes=Buffer.concat(chunks);verifyBytes(a,bytes);
  fs.writeFileSync(out+'.part',bytes,{flag:'wx'});
  fs.renameSync(out+'.part',out);
  console.log(`${a.file}: ${a.bytes} bytes verified`);
}
console.log('PROBE DOWNLOADS VERIFIED (license gate is separate)');
