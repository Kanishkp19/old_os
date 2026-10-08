// Run only after build. Stable filenames match hh-net's embedded dashboard assets.
import { readFile, writeFile, access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const source = new URL('../dist/', import.meta.url);
const destination = new URL('../../hub/dashboard/', import.meta.url);
for (const name of ['index.html', 'app.js', 'style.css']) await access(new URL(name, source));
const html = await readFile(new URL('index.html', source), 'utf8');
if (!html.includes('app.js') || !html.includes('style.css')) throw new Error('Build did not emit embedded dashboard filenames');
for (const name of ['index.html', 'app.js', 'style.css']) await writeFile(new URL(name, destination), await readFile(new URL(name, source)));
console.log(`Dashboard bundle copied to ${fileURLToPath(destination)}`);
