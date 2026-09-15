// Guards the native localization of the remote plugin against the real artifacts: plugin sources, the locale dictionaries and both generated native-text.json copies.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import en from '../../../src/i18n/locales/en.ts';

const plugin = fileURLToPath(new URL('../plugins/remote/', import.meta.url));
const dictionary: Record<string, unknown> = en;
const locales = ['en', 'zh-CN', 'zh-TW', 'ja', 'ko', 'fr', 'de', 'es', 'pt-BR', 'ru', 'vi'];
const nativeCopies = ['shared/native-text.json', 'ios/Sources/VelaRemotePlugin/Bootstrap/native-text.json'];
// The same locale sources sync-resources.mjs reads: the generated JSON must equal them, or the shipped text is stale.
const dictionaries: Record<string, Record<string, unknown>> = {};
for (const locale of locales) dictionaries[locale] = (await import(`../../../src/i18n/locales/${locale}.ts`)).default;

function sources(directory: string, files: string[] = []): string[] {
  for (const entry of readdirSync(directory)) {
    if (entry === '.build' || entry === 'node_modules' || entry === 'build' || entry === 'Bootstrap') continue;
    const path = directory + entry;
    if (statSync(path).isDirectory()) sources(path + '/', files);
    else if (/\.(swift|kt|js)$/.test(entry)) files.push(path);
  }
  return files;
}
const files = sources(plugin);
// Only string entries travel through native-text.json; function entries (parameterized web copy) are the web app's alone, exactly as sync-resources.mjs filters them.
const mobileKeys = Object.keys(dictionary).filter(key => key.startsWith('mobile.') && typeof dictionary[key] === 'string');
const placeholders = (text: string) => [...text.matchAll(/\{[a-z]+\}/g)].map(match => match[0]).sort();
const keyLiteral = /["']((?:mobile|common)\.[A-Za-z0-9.]+)["']/g;

// Returns the balanced argument text of every `.get(` call that names a dictionary key (Swift `MobileText.get(...)`, Kotlin `texts.get(...)`, JS `text(...)`).
function nativeCalls(source: string): string[] {
  const calls: string[] = [];
  for (const match of source.matchAll(/\b(?:get|text)\(/g)) {
    let depth = 1, index = match.index + match[0].length, quote: string | null = null;
    const start = index;
    while (index < source.length && depth > 0) {
      const char = source[index];
      if (quote) { if (char === '\\') index++; else if (char === quote) quote = null; }
      else if (char === '"' || char === "'") quote = char;
      else if (char === '(' || char === '[') depth++;
      else if (char === ')' || char === ']') depth--;
      index++;
    }
    const args = source.slice(start, index - 1);
    if (keyLiteral.test(args)) calls.push(args);
    keyLiteral.lastIndex = 0;
  }
  return calls;
}

test('plugin sources contain no CJK text; every string comes from the dictionary', () => {
  assert.ok(files.some(file => file.endsWith('VelaRemotePlugin.swift')) && files.some(file => file.endsWith('VelaRemotePlugin.kt')) && files.some(file => file.endsWith('download.js')));
  for (const file of files) {
    // Han, Hiragana/Katakana, Hangul plus CJK and fullwidth punctuation, so a stray fullwidth comma or ideographic full stop is caught as well.
    const hits = readFileSync(file, 'utf8').split('\n').flatMap((line, index) => /[　-〿぀-ヿ㐀-䶿一-鿿가-힯＀-￯]/.test(line) ? [`${file}:${index + 1}`] : []);
    assert.deepEqual(hits, [], `CJK text left in ${file}`);
  }
});

test('every mobile.* or common.* key referenced from Swift, Kotlin or JS exists in en.ts and in native-text.json', () => {
  const referenced = new Set<string>();
  for (const file of files) for (const match of readFileSync(file, 'utf8').matchAll(keyLiteral)) referenced.add(match[1]);
  assert.ok(referenced.has('mobile.native.trustAccept') && referenced.has('mobile.native.downloadRetry') && referenced.has('common.cancel'));
  const missing = [...referenced].filter(key => typeof dictionary[key] !== 'string');
  assert.deepEqual(missing, []);
  const nativeText = JSON.parse(readFileSync(plugin + nativeCopies[0], 'utf8')) as Record<string, Record<string, unknown>>;
  for (const locale of locales) assert.deepEqual([...referenced].filter(key => typeof nativeText[locale][key] !== 'string'), [], `${locale}: referenced key missing from native-text.json`);
});

test('both native-text.json copies carry every mobile.* key for all 11 locales with the current dictionary text', () => {
  for (const copy of nativeCopies) {
    const nativeText = JSON.parse(readFileSync(plugin + copy, 'utf8')) as Record<string, Record<string, unknown>>;
    assert.deepEqual(Object.keys(nativeText).sort(), [...locales].sort(), copy);
    for (const locale of locales) {
      const missing = [...mobileKeys, 'common.cancel', 'common.retry', 'common.loading'].filter(key => typeof nativeText[locale][key] !== 'string');
      assert.deepEqual(missing, [], `${copy} ${locale}`);
      // Stale generation: a text changed in the locale file but native-text.json was not regenerated with sync-resources.mjs.
      const stale = Object.keys(nativeText[locale]).filter(key => nativeText[locale][key] !== dictionaries[locale][key]);
      assert.deepEqual(stale, [], `${copy} ${locale}: regenerate with node scripts/sync-resources.mjs`);
    }
  }
  assert.equal(readFileSync(plugin + nativeCopies[0], 'utf8'), readFileSync(plugin + nativeCopies[1], 'utf8'));
});

test('placeholders of mobile.native.* keys match the English source in every locale', () => {
  const nativeText = JSON.parse(readFileSync(plugin + nativeCopies[0], 'utf8')) as Record<string, Record<string, string>>;
  for (const key of mobileKeys.filter(key => key.startsWith('mobile.native.'))) {
    const expected = placeholders(dictionary[key] as string);
    for (const locale of locales) assert.deepEqual(placeholders(nativeText[locale][key]), expected, `${locale} ${key}`);
  }
});

test('every native call site supplies exactly the placeholders its key declares', () => {
  let parameterized = 0;
  for (const file of files) {
    for (const args of nativeCalls(readFileSync(file, 'utf8'))) {
      // Swift `["host": value]` or Kotlin `mapOf("host" to value)`; a key without a values argument must not declare placeholders.
      const supplied = [...args.matchAll(/"([a-z]+)"\s*(?::|\bto\b)/g)].map(match => `{${match[1]}}`).sort();
      if (supplied.length) parameterized++;
      for (const match of args.matchAll(keyLiteral)) assert.deepEqual(placeholders(dictionary[match[1]] as string), supplied, `${file}: ${args}`);
    }
  }
  assert.ok(parameterized >= 9, `expected the Swift and Kotlin parameterized call sites, found ${parameterized}`);
});
