import { deflateSync } from 'node:zlib';
import { mkdir, copyFile, readFile, writeFile } from 'node:fs/promises';
const target = new URL('../plugins/remote/ios/Sources/VelaRemotePlugin/Bootstrap/', import.meta.url);
await mkdir(target, {recursive:true});
// Every `mobile.*` key of the English dictionary plus the shared keys below travels to the native plugins, so a new native key cannot be forgotten here.
const sharedNativeKeys = ['common.loading','common.retry','common.cancel'];
const nativeLocales = ['en','zh-CN','zh-TW','ja','ko','fr','de','es','pt-BR','ru','vi'];
const nativeTextKeys = dictionary => [...sharedNativeKeys, ...Object.keys(dictionary).filter(key => key.startsWith('mobile.') && typeof dictionary[key] === 'string')];
const nativeText = {};
for (const locale of nativeLocales) {
  const {default: dictionary} = await import(new URL('../../../src/i18n/locales/'+locale+'.ts', import.meta.url).href);
  nativeText[locale] = Object.fromEntries(nativeTextKeys(dictionary).map(key => [key, dictionary[key]]));
}
await writeFile(new URL('../plugins/remote/shared/native-text.json', import.meta.url), JSON.stringify(nativeText));
const source = await readFile(new URL('../plugins/remote/shared/bootstrap.py', import.meta.url));
const code = `import base64,zlib;exec(zlib.decompress(base64.b64decode('${deflateSync(source).toString('base64')}')))`;
await writeFile(new URL('../plugins/remote/shared/bootstrap-code.txt', import.meta.url), code);
for (const file of ['bootstrap.py','bootstrap-code.txt','discover.json','runtime.json','login.js','notifications.js','page-readiness.js','native-text.json']) await copyFile(new URL('../plugins/remote/shared/'+file,import.meta.url),new URL(file,target));

// 手机包版本由 package.json 统一提供，避免界面与系统安装信息不一致。
const {version}=JSON.parse(await readFile(new URL('../package.json',import.meta.url),'utf8'));
if(!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('手机 App 版本必须采用 x.y.z 格式');
for(const [path,pattern,replacement] of [
  ['../ios/App/App.xcodeproj/project.pbxproj',/MARKETING_VERSION = [^;]+;/g,`MARKETING_VERSION = ${version};`],
  ['../android/app/build.gradle',/versionName "[^"]+"/g,`versionName "${version}"`],
]) {
  const file=new URL(path,import.meta.url);const original=await readFile(file,'utf8');
  const updated=original.replace(pattern,replacement);if(updated!==original)await writeFile(file,updated);
}
