// Isolated parent-death/protocol check with pinned model/runtime. No user profile.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawn, fork, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
const script = fileURLToPath(new URL('../../src-tauri/src/tts/silero/worker.py', import.meta.url));
function start(root, directory) {
  const child = spawn(path.join(root, 'runtime/python.exe'), ['-I', '-B', '-u', script, path.join(root, 'v3_en.pt'), directory, String(process.pid), 'en'], { windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'] });
  child.stdin.on('error', () => {});
  const ready = new Promise((resolve, reject) => {
    let buffer = '';
    const timer = setTimeout(() => { child.kill(); reject(Error('Silero startup timeout')); }, 90000);
    child.stdout.on('data', chunk => {
      buffer += chunk;
      if (buffer.includes('\n')) {
        try { assert.equal(JSON.parse(buffer.split('\n')[0]).ready, true); clearTimeout(timer); resolve(); }
        catch (e) { clearTimeout(timer); reject(e); }
      }
    });
    child.once('exit', code => { clearTimeout(timer); reject(Error(`Silero exited: ${code}`)); });
  });
  return { child, ready };
}
if (process.argv[2] === '--parent') {
  const { child, ready } = start(process.argv[3], process.argv[4]);
  await ready;
  process.send({ pid: child.pid });
} else {
  const root = path.resolve(process.argv[2]);
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'glagol-en-process-'));
  try {
    const { child, ready } = start(root, directory);
    await ready;
    const memory = execFileSync('powershell.exe', ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).PeakWorkingSet64`], { encoding: 'utf8', windowsHide: true });
    console.log(`Silero EN loaded worker peak working set: ${Math.round(Number(memory) / 1048576)} MiB`);
    const exit = once(child, 'exit');
    child.stdin.end('x'.repeat(4097));
    assert.notEqual((await exit)[0], 0, 'oversized protocol must fail');
    const parent = fork(fileURLToPath(import.meta.url), ['--parent', root, directory], { windowsHide: true, stdio: ['ignore', 'ignore', 'ignore', 'ipc'] });
    const timer = setTimeout(() => parent.kill(), 90000);
    const [message] = await once(parent, 'message');
    clearTimeout(timer);
    const parentExit = once(parent, 'exit'); parent.kill(); await parentExit;
    let exited = false;
    for (let attempt = 0; attempt < 50; attempt++) {
      try { process.kill(message.pid, 0); } catch { exited = true; break; }
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    if (!exited) { process.kill(message.pid); throw Error('Silero survived parent exit'); }
    console.log('SILERO EN PROCESS SAFETY PASS: oversized protocol; parent termination');
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
}
