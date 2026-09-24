import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { loadTs, validate } from './i18n-check.mjs';

test('both dictionaries match and substitution preserves user text', () => {
  validate(loadTs('src/i18n/en.ts').en, loadTs('src/i18n/ru.ts').ru);
  const { translate } = loadTs('src/i18n/index.ts');
  assert.equal(translate('en', 'downloadRequired', { size: '57.2' }), 'Download required: 57.2 MB');
  assert.equal(translate('ru', 'downloadRequired', { size: '57,2' }), 'Нужно скачать: 57,2 МБ');
  assert.equal(translate('en', 'downloadRequired', { size: '{user text}' }), 'Download required: {user text} MB');
});

test('plural and date helpers follow live interface changes', () => {
  const cache = new Map();
  const { setInterfaceLanguage, currentLocale } = loadTs('src/i18n/index.ts', cache);
  const { pluralizeDocuments } = loadTs('src/lib/pluralize.ts', cache);
  const { formatRelativeTime } = loadTs('src/lib/format.ts', cache);
  setInterfaceLanguage('ru');
  assert.deepEqual([0, 1, 2, 5, 11, 21, 22].map(pluralizeDocuments), ['документов', 'документ', 'документа', 'документов', 'документов', 'документ', 'документа']);
  assert.equal(formatRelativeTime(Date.now()), 'только что');
  setInterfaceLanguage('en');
  assert.deepEqual([0, 1, 2, 21].map(pluralizeDocuments), ['documents', 'document', 'documents', 'documents']);
  assert.equal(formatRelativeTime(Date.now()), 'just now');
  assert.equal(currentLocale(), 'en-US');
  const date = Date.UTC(2001, 1, 3);
  assert.equal(formatRelativeTime(date), new Intl.DateTimeFormat('en-US').format(date));
  setInterfaceLanguage('ru');
  assert.equal(formatRelativeTime(date), new Intl.DateTimeFormat('ru-RU').format(date));
});

test('negative CLI gates exit nonzero for missing, extra and mismatched keys', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'glagol-i18n-'));
  try {
    for (const ru of [{}, { greeting: 'Привет {other}' }, { greeting: 'Привет {name}', extra: 'Лишнее' }]) {
      const file = path.join(dir, 'broken.json');
      fs.writeFileSync(file, JSON.stringify({ en: { greeting: 'Hello {name}' }, ru }));
      const result = spawnSync(process.execPath, ['scripts/i18n-check.mjs', file], { encoding: 'utf8' });
      assert.equal(result.status, 1, result.stdout + result.stderr);
      assert.match(result.stderr, /Missing|mismatch/);
    }
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});
