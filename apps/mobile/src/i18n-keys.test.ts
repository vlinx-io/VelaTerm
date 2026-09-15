import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import en from '../../../src/i18n/locales/en.ts';
import de from '../../../src/i18n/locales/de.ts';
import zhCN from '../../../src/i18n/locales/zh-CN.ts';

// The shared i18n entry module cannot be imported under node --test (extensionless imports), so this
// test checks the dictionaries directly: every key the start page uses exists in English and renders
// German and Chinese text rather than falling back to English.
const sources = ['main.ts', 'account-login.ts'].map(name => readFileSync(new URL(name, import.meta.url), 'utf8')).join('\n');
const usedKeys = [...new Set([...sources.matchAll(/'(mobile\.\w+)'/g)].map(match => match[1]))];
const text = (dict: Record<string, unknown>, key: string) => {
  const entry = dict[key];
  return typeof entry === 'function' ? (entry as (...args: string[]) => string)('x', 'y') : String(entry);
};

test('every mobile key used by the start page exists in the English dictionary', () => {
  assert.ok(usedKeys.length > 50, `expected the start page to use many keys, found ${usedKeys.length}`);
  for (const key of usedKeys) assert.ok(key in en, `${key} is missing in en.ts`);
});

test('the start page renders German with locale de and English with locale en', () => {
  assert.equal(text(en, 'mobile.workspaceTitle'), 'Your workspace');
  assert.equal(text(de, 'mobile.workspaceTitle'), 'Ihr Arbeitsbereich');
  assert.equal(text(en, 'mobile.phaseDisconnected'), 'Disconnected');
  assert.equal(text(de, 'mobile.phaseDisconnected'), 'Verbindung getrennt');
  assert.equal(text(en, 'mobile.buildInfo').startsWith('App vx · Built y'), true);
  assert.equal(text(de, 'mobile.deleteConnectionConfirm'), '„x“ und die gespeicherten Zugangsdaten löschen? Remote-Projekte werden nicht gelöscht.');
});

test('German and Chinese entries exist for every used key and are not English fallbacks', () => {
  for (const key of usedKeys) {
    assert.ok(key in de, `${key} is missing in de.ts`);
    assert.ok(key in zhCN, `${key} is missing in zh-CN.ts`);
    if (key === 'mobile.remote' || key === 'mobile.online' || key === 'mobile.offline') continue; // identical product terms
    assert.notEqual(text(de, key), text(en, key), `${key} is untranslated in de.ts`);
    assert.match(text(zhCN, key), /[\u4e00-\u9fff]/, `${key} has no Chinese text in zh-CN.ts`);
  }
});

test('start page sources contain no hardcoded CJK text and index.html declares English', () => {
  const srcDir = new URL('./', import.meta.url);
  const files = readdirSync(srcDir).filter(name => name.endsWith('.ts') && !name.endsWith('.test.ts'));
  assert.ok(files.includes('main.ts') && files.includes('account-login.ts'), 'source scan must cover the start page modules');
  for (const name of files) {
    const hit = readFileSync(new URL(name, srcDir), 'utf8').match(/[\u4e00-\u9fff]/);
    assert.equal(hit, null, `${name} still contains CJK text`);
  }
  const html = readFileSync(new URL('../index.html', srcDir), 'utf8');
  assert.equal(html.match(/[\u4e00-\u9fff]/), null, 'index.html still contains CJK text');
  assert.match(html, /<html lang="en">/, 'index.html must declare lang="en" as the static default');
});
