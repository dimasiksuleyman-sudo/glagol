import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const version = JSON.parse(read("package.json")).version;
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
console.info(`Glagol ${version}: package, Tauri and Cargo versions match.`);
