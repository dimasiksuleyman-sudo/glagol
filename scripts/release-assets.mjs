// Prepare the GitHub Release assets that accompany a signed NSIS build:
// `latest.json` for the in-app updater and `SHA256SUMS.txt`.
//
//   node scripts/release-assets.mjs [--notes "text"] [--dir <bundle/nsis>]
//
// Requires `Glagol_<version>_x64-setup.exe` and its `.sig` produced by
// `pnpm tauri build --config src-tauri/tauri.release.conf.json` with the
// signing key in TAURI_SIGNING_PRIVATE_KEY. Reads no secrets itself.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkVersions } from "./check-version.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPOSITORY = "https://github.com/dimasiksuleyman-sudo/glagol";

export const installerName = (version) => `Glagol_${version}_x64-setup.exe`;

/** Build the updater manifest; both target keys point at the same NSIS installer. */
export function buildManifest({ version, signature, notes, pubDate }) {
  const sig = signature.trim();
  if (!sig) throw new Error("empty updater signature");
  const platform = {
    signature: sig,
    url: `${REPOSITORY}/releases/download/v${version}/${installerName(version)}`,
  };
  return {
    version,
    notes,
    pub_date: pubDate,
    platforms: { "windows-x86_64-nsis": platform, "windows-x86_64": platform },
  };
}

export function prepareReleaseAssets({ base = root, dir, notes = "", now = new Date() } = {}) {
  const version = checkVersions(base);
  const bundle = resolve(base, dir ?? "src-tauri/target/release/bundle/nsis");
  const installer = resolve(bundle, installerName(version));
  const signaturePath = `${installer}.sig`;
  for (const path of [installer, signaturePath]) {
    if (!existsSync(path)) throw new Error(`missing ${path}; run the signed release build first`);
  }
  const manifest = buildManifest({
    version,
    signature: readFileSync(signaturePath, "utf8"),
    notes: notes || `Glagol ${version}`,
    pubDate: now.toISOString(),
  });
  const digest = createHash("sha256").update(readFileSync(installer)).digest("hex");
  writeFileSync(resolve(bundle, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  writeFileSync(resolve(bundle, "SHA256SUMS.txt"), `${digest}  ${installerName(version)}\n`);
  return { version, bundle, digest };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const option = (name) => {
    const index = args.indexOf(name);
    return index >= 0 ? args[index + 1] : undefined;
  };
  try {
    const { version, bundle, digest } = prepareReleaseAssets({ notes: option("--notes"), dir: option("--dir") });
    console.info(`Glagol ${version}: latest.json and SHA256SUMS.txt written to ${bundle}`);
    console.info(`${digest}  ${installerName(version)}`);
  } catch (error) {
    console.error(`release-assets: ${error.message}`);
    process.exitCode = 1;
  }
}
