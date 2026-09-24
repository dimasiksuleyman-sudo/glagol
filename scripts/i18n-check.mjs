import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';

// Load only repository-owned pure TypeScript modules; no bundler or test framework.
export function loadTs(file, cache = new Map()) {
  file = path.resolve(file);
  if (cache.has(file)) return cache.get(file);
  const result = {};
  cache.set(file, result);
  const code = ts.transpileModule(fs.readFileSync(file, 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const require = (specifier) => {
    let target = specifier.startsWith('@/') ? path.resolve('src', specifier.slice(2)) : path.resolve(path.dirname(file), specifier);
    if (!fs.existsSync(`${target}.ts`)) target = path.join(target, 'index');
    return loadTs(`${target}.ts`, cache);
  };
  new Function('exports', 'require', code)(result, require);
  return result;
}

export function validate(en, ru) {
  const errors = [];
  const parameters = text => [...new Set([...text.matchAll(/\{([^{}]+)\}/g)].map(match => match[1]))].sort().join(',');
  for (const key of new Set([...Object.keys(en), ...Object.keys(ru)])) {
    if (typeof en[key] !== 'string' || !en[key].trim()) errors.push(`Missing English text: ${key}`);
    if (typeof ru[key] !== 'string' || !ru[key].trim()) errors.push(`Missing Russian text: ${key}`);
    if (typeof en[key] === 'string' && typeof ru[key] === 'string' && parameters(en[key]) !== parameters(ru[key])) errors.push(`Parameter mismatch: ${key}`);
  }
  if (errors.length) throw new Error(errors.join('\n'));
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    const fixture = process.argv[2];
    const { en, ru } = fixture ? JSON.parse(fs.readFileSync(fixture, 'utf8')) : { en: loadTs('src/i18n/en.ts').en, ru: loadTs('src/i18n/ru.ts').ru };
    validate(en, ru);
    console.log(`I18N PASS: ${Object.keys(en).length} keys with matching parameters`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
