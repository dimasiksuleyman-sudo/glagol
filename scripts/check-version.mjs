import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function checkVersions(base = root) {
  const read = (path) => readFileSync(resolve(base, path), "utf8");
  const version = JSON.parse(read("package.json")).version;
  if (typeof version !== "string" || !/^\d+\.\d+\.\d+(?:-[\w.-]+)?(?:\+[\w.-]+)?$/.test(version)) {
    throw new Error("package.json: valid application version required");
  }
  const versions = {
    "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version,
    "src-tauri/Cargo.toml": read("src-tauri/Cargo.toml")
      .match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1],
    "src-tauri/Cargo.lock": read("src-tauri/Cargo.lock")
      .match(/^name = "glagol"\r?\nversion = "([^"]+)"/m)?.[1],
  };
  for (const [path, actual] of Object.entries(versions)) {
    if (actual !== version) {
      throw new Error(`${path}: version ${actual} differs from package.json (${version})`);
    }
  }
  return version;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.info(`Glagol ${checkVersions()}: package, Tauri and Cargo versions match.`);
}
