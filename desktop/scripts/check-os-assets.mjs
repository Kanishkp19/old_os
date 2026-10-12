import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const assetDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../assets/os');
const source = await readFile(path.join(assetDir, 'manifest.json'), 'utf8');
const licenses = await readFile(path.join(assetDir, 'LICENSES.md'), 'utf8');
const manifest = JSON.parse(source);
const required = ['photos', 'files', 'browser', 'youtube', 'music', 'notes', 'calculator', 'devices', 'settings'];
const ids = manifest.apps?.map((app) => app.id);
if (manifest.schema_version !== 1 || !Array.isArray(ids) ||
    ids.length !== required.length || new Set(ids).size !== required.length ||
    required.some((id) => !ids.includes(id))) {
  throw new Error('OS manifest must contain exactly the nine built-in apps');
}
for (const app of manifest.apps) {
  if (!app.name_en?.trim() || !app.name_hi?.trim()) {
    throw new Error(`Missing English or Hindi label for ${app.id}`);
  }
}
if (/https?:\/\//i.test(source)) throw new Error('Remote URL in OS manifest');
for (const font of manifest.fonts ?? []) {
  if (!font.family || !font.license || !licenses.includes(font.family) ||
      !licenses.includes(font.license)) {
    throw new Error(`Missing license notice for ${font.family ?? 'unknown font'}`);
  }
}
if (manifest.fonts?.length !== 2) throw new Error('Expected two font entries');
console.log(`Validated ${manifest.apps.length} app records and ${manifest.fonts.length} font license records`);
