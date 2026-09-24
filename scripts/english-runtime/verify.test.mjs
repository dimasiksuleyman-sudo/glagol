import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { readManifest, validateManifest, verifyBytes, checkLicenses, safePath } from './verify.mjs';

test('catalog pins all downloads, four voices and expected totals', () => {
  const m = readManifest(); validateManifest(m);
  assert.equal(m.artifacts.filter(a => a.kind === 'stt').reduce((n,a)=>n+a.bytes,0), 142300974);
  assert.equal(m.artifacts.filter(a => a.kind === 'tts').reduce((n,a)=>n+a.bytes,0), 110554097);
  assert.equal(m.artifacts.filter(a=>a.file.endsWith('.kokorovoice')).length,4);
});
test('reject corrupt, truncated and oversized files', () => {
  const bytes=Buffer.from('synthetic fixture'), a={file:'fixture',bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};
  verifyBytes(a,bytes);
  for (const bad of [bytes.subarray(1),Buffer.concat([bytes,Buffer.from('!')]),Buffer.alloc(bytes.length)]) assert.throws(()=>verifyBytes(a,bad),/Integrity mismatch/);
});
test('reject path escapes and Windows alternate data streams', () => {
  for(const name of ['../x','a/../../x','C:/x','/x','a:stream','a\\b','a//b','a/./b']) assert.throws(()=>safePath('.',name));
});
test('reject mutable URLs, missing hashes and duplicate paths', () => {
  for(const mutate of [m=>m.artifacts[0].url=m.artifacts[0].url.replace(m.model_revision,'main'),m=>m.runtime.sha256='',m=>m.artifacts.push(m.artifacts[0])]) {
    const m=readManifest();mutate(m);assert.throws(()=>validateManifest(m));
  }
});
test('missing license terms cannot pass the delivery gate', () => {
  assert.throws(()=>checkLicenses(readManifest()),/License terms unresolved: moonshine-g2p/);
  const result=spawnSync(process.execPath,['scripts/english-runtime/verify.mjs','licenses'],{encoding:'utf8'});
  assert.equal(result.status,1);assert.match(result.stderr,/License terms unresolved/);
});
test('file CLI exits nonzero on corrupt runtime, before any native load', () => {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'glagol-runtime-fixture-'));
  try {
    fs.writeFileSync(path.join(root,'runtime.whl'),'corrupt synthetic archive');
    const result=spawnSync(process.execPath,['scripts/english-runtime/verify.mjs','files',root],{encoding:'utf8'});
    assert.equal(result.status,1);assert.match(result.stderr,/Integrity mismatch: runtime.whl/);
  } finally {
    fs.unlinkSync(path.join(root,'runtime.whl'));fs.rmdirSync(root);
  }
});
