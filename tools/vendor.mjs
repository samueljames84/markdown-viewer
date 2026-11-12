import { build } from 'esbuild';
import { cp, mkdir, writeFile, readdir, readFile, unlink } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const target = resolve(here, '..', 'frontend', 'vendor');
await mkdir(target, { recursive: true });
let previous;
try { previous = JSON.parse(await readFile(resolve(target, 'manifest.json'), 'utf8')); }
catch (error) { if (error.code !== 'ENOENT') throw error; }
for (const name of Object.keys(previous?.files || {})) {
  const file = resolve(target, name);
  if (!file.startsWith(target + '\\')) throw new Error('Invalid generated asset manifest path');
  await unlink(file);
}
const options = { bundle: true, minify: true, legalComments: 'eof', platform: 'browser', target: 'chrome105', nodePaths: [resolve(here, 'node_modules')] };
await build({ ...options, entryPoints: [resolve(here, '..', 'frontend', 'markdown.js')], outfile: resolve(target, 'markdown.js'), format: 'esm', external: ['./katex.js', './highlight.js'] });
await build({ ...options, stdin: { contents: "export {default} from 'dompurify'", resolveDir: here }, outfile: resolve(target, 'purify.js'), format: 'esm' });
await build({ ...options, stdin: { contents: "export {default} from 'katex'", resolveDir: here }, outfile: resolve(target, 'katex.js'), format: 'esm' });
await build({ ...options, stdin: { contents: "export {default} from 'highlight.js'", resolveDir: here }, outfile: resolve(target, 'highlight.js'), format: 'esm' });
await build({ ...options, stdin: { contents: "export {default} from 'mermaid'", resolveDir: here }, outdir: resolve(target, 'mermaid'), format: 'esm', splitting: true, entryNames: 'mermaid', chunkNames: '[name]-[hash]' });
await cp(resolve(here, 'node_modules', 'katex', 'dist', 'katex.min.css'), resolve(target, 'katex.css'));
await cp(resolve(here, 'node_modules', 'katex', 'dist', 'fonts'), resolve(target, 'fonts'), { recursive: true });

const packages = JSON.parse(await readFile(resolve(here, 'package.json'), 'utf8')).devDependencies;
const licenses = [];
async function collectLicenses(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name.startsWith('.')) continue;
    const subdir = resolve(dir, entry.name);
    if (entry.name.startsWith('@')) { await collectLicenses(subdir); continue; }
    let pkg;
    try { pkg = JSON.parse(await readFile(resolve(subdir, 'package.json'), 'utf8')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; continue; }
    const names = await readdir(subdir);
    for (const file of names.filter(n => /^(license|licence|copying|notice)(\.|$)/i.test(n))) {
      const text = await readFile(resolve(subdir, file), 'utf8');
      licenses.push(`\n===== ${pkg.name} ${pkg.version}: ${file} =====\n${text}`);
    }
    if (names.includes('node_modules')) await collectLicenses(resolve(subdir, 'node_modules'));
  }
}
await collectLicenses(resolve(here, 'node_modules'));
await writeFile(resolve(target, 'LICENSES.txt'), licenses.join('\n'));
const files = {};
async function hash(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = resolve(dir, entry.name);
    if (entry.isDirectory()) await hash(path);
    else if (entry.name !== 'manifest.json') {
      const bytes = await readFile(path);
      files[path.slice(target.length + 1).replaceAll('\\', '/')] = { bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
    }
  }
}
await hash(target);
await writeFile(resolve(target, 'manifest.json'), JSON.stringify({ packages, files }, null, 2));
console.log(`Vendored ${Object.keys(files).length} offline assets.`);
