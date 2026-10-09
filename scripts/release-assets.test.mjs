import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { buildManifest, prepareReleaseAssets } from "./release-assets.mjs";

function fixture(version = "1.2.3", { signature = "c2lnbmF0dXJl\n", installer = true } = {}) {
  const base = mkdtempSync(join(tmpdir(), "glagol-release-"));
  mkdirSync(join(base, "src-tauri"));
  writeFileSync(join(base, "package.json"), JSON.stringify({ version }));
  writeFileSync(join(base, "src-tauri/tauri.conf.json"), JSON.stringify({ version }));
  writeFileSync(join(base, "src-tauri/Cargo.toml"), `[package]\nname = "glagol"\nversion = "${version}"\n`);
  writeFileSync(join(base, "src-tauri/Cargo.lock"), `[[package]]\nname = "glagol"\nversion = "${version}"\n`);
  const bundle = join(base, "nsis");
  mkdirSync(bundle);
  if (installer) writeFileSync(join(bundle, `Glagol_${version}_x64-setup.exe`), "installer-bytes");
  if (signature !== null) writeFileSync(join(bundle, `Glagol_${version}_x64-setup.exe.sig`), signature);
  return { base, bundle };
}

test("manifest points both Windows targets at the versioned release asset", () => {
  const manifest = buildManifest({ version: "0.6.0", signature: " sig\n", notes: "n", pubDate: "2026-10-09T00:00:00.000Z" });
  assert.equal(manifest.version, "0.6.0");
  for (const key of ["windows-x86_64-nsis", "windows-x86_64"]) {
    assert.equal(manifest.platforms[key].signature, "sig");
    assert.equal(
      manifest.platforms[key].url,
      "https://github.com/dimasiksuleyman-sudo/glagol/releases/download/v0.6.0/Glagol_0.6.0_x64-setup.exe",
    );
  }
});

test("writes latest.json and SHA256SUMS.txt for the current version", () => {
  const { base, bundle } = fixture();
  const result = prepareReleaseAssets({ base, dir: "nsis", notes: "Fixes", now: new Date("2026-10-09T00:00:00Z") });
  const manifest = JSON.parse(readFileSync(join(bundle, "latest.json"), "utf8"));
  assert.equal(manifest.version, "1.2.3");
  assert.equal(manifest.notes, "Fixes");
  assert.equal(manifest.pub_date, "2026-10-09T00:00:00.000Z");
  const digest = createHash("sha256").update("installer-bytes").digest("hex");
  assert.equal(result.digest, digest);
  assert.equal(readFileSync(join(bundle, "SHA256SUMS.txt"), "utf8"), `${digest}  Glagol_1.2.3_x64-setup.exe\n`);
});

test("refuses an unsigned build, an empty signature and a missing installer", () => {
  assert.throws(() => prepareReleaseAssets({ base: fixture("1.0.0", { signature: null }).base, dir: "nsis" }), /missing .*\.sig/);
  assert.throws(() => prepareReleaseAssets({ base: fixture("1.0.0", { signature: "  \n" }).base, dir: "nsis" }), /empty updater signature/);
  assert.throws(() => prepareReleaseAssets({ base: fixture("1.0.0", { installer: false }).base, dir: "nsis" }), /missing .*setup\.exe/);
});
