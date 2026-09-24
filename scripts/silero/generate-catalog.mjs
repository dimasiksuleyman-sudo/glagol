// Run after assembling and validating the pinned runtime.
import { readFileSync, writeFileSync, copyFileSync } from "node:fs";
const manifest = JSON.parse(readFileSync(new URL("./runtime-manifest.json", import.meta.url)));
const model = { file: "v5_5_ru.pt", url: "https://models.silero.ai/models/tts/ru/v5_5_ru.pt", bytes: 145420684, sha256: "50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437" };
const english = JSON.parse(readFileSync(new URL("./english-model.json", import.meta.url)));
const rust = a => `Artifact { file: ${JSON.stringify(a.file)}, url: ${JSON.stringify(a.url)}, bytes: ${a.bytes}, sha256: ${JSON.stringify(a.sha256)} }`;
writeFileSync("src-tauri/src/tts/silero/catalog.rs", "// Generated from scripts/silero/runtime-manifest.json; update only after a real smoke test.\nuse crate::stt::local::catalog::Artifact;\npub const ARTIFACTS: &[Artifact] = &[\n" + manifest.artifacts.map(a => rust(a) + ",").join("\n") + "\n];\npub const MODEL: Artifact = " + rust(model) + ";\n");
copyFileSync(process.argv[2] ?? ".scratch/silero/runtime-271/runtime.json", "src-tauri/src/tts/silero/runtime-files.json");
writeFileSync("src-tauri/src/tts/silero/catalog.rs", "\npub const EN_MODEL: Artifact = " + rust(english) + ";\n", {flag:"a"});
