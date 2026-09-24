// Isolated process checks. The supplied EXE runs only its internal worker mode.
import { spawn, fork, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function start(exe, root) {
  const child = spawn(exe, ['--moonshine-worker', root, String(process.pid)], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  child.stdin.on('error', () => {});
  child.stderr.resume(); // Never log model input/output.
  const ready = new Promise((resolve, reject) => {
    let buffer = '';
    const timeout = setTimeout(() => { child.kill(); reject(new Error('Worker startup timeout')); }, 90000);
    child.stdout.on('data', chunk => {
      buffer += chunk;
      if (buffer.length > 512000) { child.kill(); reject(new Error('Oversized response')); clearTimeout(timeout); }
      if (buffer.includes('\n')) {
        try { assert.equal(JSON.parse(buffer.split('\n')[0]).ok, true); clearTimeout(timeout); resolve(); }
        catch (error) { clearTimeout(timeout); reject(error); }
      }
    });
    child.once('exit', code => { clearTimeout(timeout); reject(new Error(`Worker exited before ready: ${code}`)); });
  });
  return { child, ready };
}

if (process.argv[2] === '--parent') {
  const { child, ready } = start(process.argv[3], process.argv[4]);
  await ready;
  process.send({ ready: true, pid: child.pid });
} else {
  const exe = path.resolve(process.argv[2]);
  const root = path.resolve(process.argv[3]);
  const { child, ready } = start(exe, root);
  await ready;
  const memory = execFileSync('powershell.exe', ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).PeakWorkingSet64`], { encoding: 'utf8', windowsHide: true }).trim();
  console.log(`Moonshine loaded worker peak working set: ${Math.round(Number(memory) / 1048576)} MiB`);
  const exit = once(child, 'exit');
  child.stdin.end('x'.repeat(512001));
  const [code] = await exit;
  assert.notEqual(code, 0, 'oversized unterminated request must fail');

  const parent = fork(fileURLToPath(import.meta.url), ['--parent', exe, root], { windowsHide: true, stdio: ['ignore', 'ignore', 'ignore', 'ipc'] });
  const timer = setTimeout(() => parent.kill(), 90000);
  const [message] = await once(parent, 'message');
  clearTimeout(timer);
  assert.equal(message.ready, true);
  const parentExit = once(parent, 'exit');
  parent.kill(); await parentExit;
  let exited = false;
  for (let attempt = 0; attempt < 50; attempt++) {
    try { process.kill(message.pid, 0); } catch { exited = true; break; }
    await sleep(100);
  }
  if (!exited) { process.kill(message.pid); throw new Error('Owned worker survived parent exit'); }
  console.log('MOONSHINE PROCESS SAFETY PASS: oversized protocol rejected; worker exits after parent termination');
}
