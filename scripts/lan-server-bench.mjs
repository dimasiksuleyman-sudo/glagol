#!/usr/bin/env node
// Load test for Glagol's LAN recognition server (GL-010, step L6).
//
// Usage (from any computer in the same network, Node 18+ only, no packages):
//   node scripts/lan-server-bench.mjs "<connection code>" [clients=10] [rounds=3] [wav=path]
//
// Each round, <clients> requests are sent at the same moment, like several
// office computers finishing a phrase together. Without a WAV file a 6-second
// synthetic signal is used: GigaAM's cost depends on the audio length, not on
// what is said, so the timings are representative; recognized text is empty.
// Nothing is saved; only timings are printed.

import { readFileSync } from "node:fs";

function parseCode(code) {
  const match = /^glagol:\/\/([^/?#]+)\/\?(?:.*&)?key=([0-9a-fA-F]{64})(?:&.*)?$/.exec(code.trim());
  if (!match) throw new Error("not a Glagol connection code (glagol://host:port/?key=...)");
  return { base: `http://${match[1]}/v1`, key: match[2] };
}

function syntheticWav(seconds = 6, rate = 16000) {
  const samples = seconds * rate;
  const buffer = Buffer.alloc(44 + samples * 2);
  buffer.write("RIFF", 0); buffer.writeUInt32LE(36 + samples * 2, 4); buffer.write("WAVE", 8);
  buffer.write("fmt ", 12); buffer.writeUInt32LE(16, 16); buffer.writeUInt16LE(1, 20); buffer.writeUInt16LE(1, 22);
  buffer.writeUInt32LE(rate, 24); buffer.writeUInt32LE(rate * 2, 28); buffer.writeUInt16LE(2, 32); buffer.writeUInt16LE(16, 34);
  buffer.write("data", 36); buffer.writeUInt32LE(samples * 2, 40);
  let seed = 12345;
  for (let i = 0; i < samples; i++) {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    const noise = (seed / 0x7fffffff - 0.5) * 3000;
    const tone = Math.sin((2 * Math.PI * 220 * i) / rate) * 2000 * (0.5 + 0.5 * Math.sin(i / 4000));
    buffer.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(noise + tone))), 44 + i * 2);
  }
  return buffer;
}

function wavSeconds(wav) {
  const rate = wav.readUInt32LE(24), channels = wav.readUInt16LE(22), bits = wav.readUInt16LE(34);
  const dataSize = wav.readUInt32LE(40);
  return dataSize / (rate * channels * (bits / 8));
}

async function transcribe(base, key, wav) {
  const form = new FormData();
  form.append("file", new Blob([wav], { type: "audio/wav" }), "bench.wav");
  form.append("model", "glagol");
  form.append("language", "ru");
  form.append("response_format", "json");
  const started = performance.now();
  const response = await fetch(`${base}/audio/transcriptions`, { method: "POST", headers: { Authorization: `Bearer ${key}` }, body: form });
  await response.text();
  return { ms: performance.now() - started, status: response.status };
}

function percentile(sorted, p) {
  if (sorted.length === 0) return NaN;
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
}

async function main() {
  const [code, clientsArg = "10", roundsArg = "3", wavPath] = process.argv.slice(2);
  if (!code) {
    console.error('Usage: node scripts/lan-server-bench.mjs "<connection code>" [clients=10] [rounds=3] [wav=path]');
    process.exit(2);
  }
  const { base, key } = parseCode(code);
  const clients = Math.max(1, Number(clientsArg));
  const rounds = Math.max(1, Number(roundsArg));
  const wav = wavPath ? readFileSync(wavPath) : syntheticWav();
  const seconds = wavSeconds(wav);

  const models = await fetch(`${base}/models`, { headers: { Authorization: `Bearer ${key}` } });
  if (!models.ok) throw new Error(`GET /models answered ${models.status}: ${await models.text()}`);
  const model = (await models.json()).data?.[0]?.id ?? "?";
  console.log(`Server ${base} · model ${model} · ${clients} clients × ${rounds} rounds · ${seconds.toFixed(1)} s of audio each`);

  // One warm-up request so model loading is not counted.
  const warm = await transcribe(base, key, wav);
  console.log(`warm-up: ${(warm.ms / 1000).toFixed(2)} s (status ${warm.status})`);

  const all = [];
  let busy = 0, failed = 0, audio = 0;
  const benchStarted = performance.now();
  for (let round = 1; round <= rounds; round++) {
    const results = await Promise.all(Array.from({ length: clients }, () => transcribe(base, key, wav).catch(() => ({ ms: NaN, status: 0 }))));
    const ok = results.filter((r) => r.status === 200);
    busy += results.filter((r) => r.status === 429).length;
    failed += results.filter((r) => r.status !== 200 && r.status !== 429).length;
    audio += ok.length * seconds;
    ok.forEach((r) => all.push(r.ms));
    const times = ok.map((r) => r.ms).sort((a, b) => a - b);
    console.log(`round ${round}: ok ${ok.length}/${clients}, fastest ${(times[0] / 1000).toFixed(2)} s, slowest ${(times[times.length - 1] / 1000).toFixed(2)} s`);
  }
  const wall = (performance.now() - benchStarted) / 1000;
  const sorted = all.sort((a, b) => a - b);
  console.log("---");
  console.log(`waiting time per phrase: median ${(percentile(sorted, 50) / 1000).toFixed(2)} s, 95% ${(percentile(sorted, 95) / 1000).toFixed(2)} s`);
  console.log(`throughput: ${(audio / wall).toFixed(1)} s of speech per second (${(audio / 60).toFixed(1)} min of speech in ${wall.toFixed(1)} s)`);
  console.log(`busy (429): ${busy}, errors: ${failed}`);
}

main().catch((e) => { console.error(String(e?.message ?? e)); process.exit(1); });
