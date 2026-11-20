import { createRequire } from 'node:module';
import { execFileSync, spawn } from 'node:child_process';
import { mkdtemp, writeFile, readFile, rename, unlink, mkdir, rm, copyFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url), { chromium } = require('@playwright/test');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dir = await mkdtemp(resolve(tmpdir(), 'mdviewer-test-'));
const appdata = resolve(dir, 'roaming'), local = resolve(dir, 'local');
await mkdir(appdata); await mkdir(local);
const documents = resolve(dir, 'documents'); await mkdir(documents);
const first = resolve(documents, 'first.md'), second = resolve(documents, 'second.md');
await writeFile(first, '# First\n\n' + 'Long content for scroll.\n\n'.repeat(250));
await writeFile(second, '# Second\n\nAnother document.');
const picture = process.env.MDVIEWER_TEST_IMAGE ? resolve(documents, 'provided-picture.png') : null;
const pictureDoc = picture ? resolve(documents, 'picture.md') : null;
if (picture && pictureDoc) {
  await copyFile(process.env.MDVIEWER_TEST_IMAGE, picture);
  await writeFile(pictureDoc, '# Picture test\n\n![Provided picture](provided-picture.png)\n');
}
const env = { ...process.env, APPDATA: appdata, LOCALAPPDATA: local, MDVIEWER_DEBUG_PORT: '19347' };
const exe = resolve(root, 'target', 'x86_64-pc-windows-msvc', 'debug', 'mdviewer.exe');
await mkdir(resolve(appdata, 'MdViewer'));
const wide = text => Array.from({length:text.length}, (_,i) => text.charCodeAt(i));
await writeFile(resolve(appdata, 'MdViewer', 'settings.json'), JSON.stringify({version:1,tabs:[
  {path:wide(resolve(documents,'missing.md')),scroll:0},{path:wide(first),scroll:0}
],active:1}));
let child, browser;
async function connect(child, port = 19347) {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break; } catch { }
    if (child.exitCode != null) throw new Error(`Native viewer exited ${child.exitCode}`);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  return chromium.connectOverCDP(`http://127.0.0.1:${port}`);
}
async function waitSettings(predicate) {
  for (let i = 0; i < 50; i++) {
    const settings = JSON.parse(await readFile(resolve(appdata, 'MdViewer', 'settings.json'), 'utf8'));
    if (predicate(settings)) return settings;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw new Error('Timed out waiting for settings to update');
}
try {
  child = spawn(exe, [], { env, stdio: 'pipe' });
  child.stderr.on('data', data => process.stderr.write(data));
  browser = await connect(child);
  const context = browser.contexts()[0];
  let page;
  for (let i = 0; i < 50; i++) {
    page = context.pages().find(p => p.url().startsWith('https://mdv.localhost/'));
    if (page) break;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  assert.ok(page, 'native application page');
  await page.locator('#content h1').waitFor();
  assert.equal(await page.locator('#content h1').innerText(), 'First');
  await page.waitForFunction(path => document.getElementById('path').textContent === path, first);
  assert.equal(execFileSync('powershell.exe', ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).MainWindowTitle`], {encoding:'utf8'}).trim(), `MdViewer | Path: ${first}`);
  await page.keyboard.press('Control+Shift+s');
  await page.locator('#pdf-orientation').waitFor({timeout:5000});
  assert.equal(await page.locator('#pdf-orientation').inputValue(), 'portrait');
  await page.locator('#print-cancel').click();
  const initialViewport = await page.evaluate(() => ({width:innerWidth,height:innerHeight}));
  assert.equal(await page.locator('.tab').count(), 1, 'missing saved file silently discarded');
  const requests = [];
  page.on('request', request => requests.push(new URL(request.url()).pathname));
  await page.locator('#viewport').evaluate(node => { node.scrollTop = 500; });
  await page.waitForTimeout(300);
  const temporary = resolve(dir, 'replacement.tmp');
  await writeFile(temporary, '# First reloaded\n\n' + 'Long content for scroll.\n\n'.repeat(250));
  await unlink(first); await rename(temporary, first);
  await page.waitForFunction(() => document.querySelector('#content h1')?.textContent === 'First reloaded');
  assert.ok(await page.locator('#viewport').evaluate(node => node.scrollTop) >= 450, 'reload scroll retained');
  await unlink(first); await page.waitForTimeout(500);
  assert.equal(await page.locator('#content h1').innerText(), 'First reloaded');
  assert.equal(await page.locator('#notice').innerText(), '');
  await writeFile(first, '# Reappeared');
  await page.waitForFunction(() => document.querySelector('#content h1')?.textContent === 'Reappeared');
  await rename(documents, documents + '-moved'); await page.waitForTimeout(500);
  assert.equal(await page.locator('#content h1').innerText(), 'Reappeared');
  await rename(documents + '-moved', documents); await page.waitForTimeout(500);
  await writeFile(first, '# Folder recovered');
  await page.waitForFunction(() => document.querySelector('#content h1')?.textContent === 'Folder recovered');
  const secondLaunch = spawn(exe, [second, first], { env, stdio: 'pipe' });
  const exit = await new Promise(resolve => secondLaunch.once('exit', resolve));
  assert.equal(exit, 0);
  await page.waitForFunction(() => document.querySelectorAll('.tab').length === 2);
  assert.equal(await page.locator('.tab').count(), 2);
  assert.ok(!requests.includes('/document/2'), 'inactive tab was not read/rendered');
  await writeFile(second, '# Second changed'); await page.waitForTimeout(400);
  assert.ok(!requests.includes('/document/2'), 'inactive change did not render');
  const target = await page.locator('.tab').nth(1).boundingBox();
  await page.locator('.tab').nth(0).dragTo(page.locator('.tab').nth(1), {targetPosition:{x:target.width - 4,y:10}});
  await page.waitForFunction(() => document.querySelector('.tab .name')?.textContent === 'second.md');
  await page.keyboard.press('Control+Tab');
  await page.waitForFunction(() => document.querySelector('#content h1')?.textContent === 'Second changed');
  await page.keyboard.press('Control+w');
  await page.waitForFunction(() => document.querySelectorAll('.tab').length === 1);
  if (pictureDoc) {
    const pictureLaunch = spawn(exe, [pictureDoc], {env, stdio:'pipe'});
    assert.equal(await new Promise(resolve => pictureLaunch.once('exit', resolve)), 0);
    await page.waitForFunction(() => {
      const image = document.querySelector('#content img[alt="Provided picture"]');
      return image?.complete && image.naturalWidth > 0;
    }, {timeout:10000});
    console.log(`PASS image: loaded provided PNG (${(await readFile(picture)).length} bytes) through the native local-image protocol.`);
    await page.keyboard.press('Control+w');
    await page.waitForFunction(() => document.querySelectorAll('.tab').length === 1);
  }
  await page.keyboard.press('Control+=');
  await page.waitForFunction(() => document.getElementById('zoom').textContent === '110%');
  await page.evaluate(() => window.receive({type:'command',command:'custom'}));
  await page.locator('#hex-link').fill('#fedcba');
  await page.locator('button[value=apply]').click();
  let saved = await waitSettings(settings => settings.custom_colors?.link === '#fedcba');
  await page.evaluate(() => window.receive({type:'command',command:'reset-theme'}));
  saved = await waitSettings(settings => settings.theme === 'sepia' && Object.keys(settings.custom_colors || {}).length === 0);
  const featureLaunch = spawn(exe, [resolve(root, 'tests', 'fixtures', 'features.md')], {env, stdio:'pipe'});
  assert.equal(await new Promise(resolve => featureLaunch.once('exit', resolve)), 0);
  await page.locator('#content .mermaid svg').waitFor({timeout:30000});
  assert.ok(await page.locator('#content .katex').count() >= 2, 'native optional renderer assets');
  await page.locator('#content a[href="plain.md"]').click();
  await page.waitForFunction(() => document.querySelector('#content h1')?.textContent === 'Plain document');
  assert.equal(await page.locator('.tab').count(), 3, 'relative Markdown link opens a new tab');
  await page.waitForFunction(path => document.getElementById('path').textContent === path, resolve(root, 'tests', 'fixtures', 'plain.md'));
  await page.keyboard.press('Control+w');
  await page.waitForFunction(() => document.querySelector('#content h1')?.textContent === 'MdViewer feature gallery');
  await page.keyboard.press('Control+w');
  await page.waitForFunction(() => document.querySelectorAll('.tab').length === 1);
  await page.evaluate(() => window.receive({ type:'command', command:'exit' }));
  await new Promise(resolve => child.once('exit', resolve));
  child = null;
  const settings = JSON.parse(await readFile(resolve(appdata, 'MdViewer', 'settings.json'), 'utf8'));
  assert.equal(settings.tabs.length, 1);
  assert.equal(settings.zoom, 110);
  assert.equal(settings.theme, 'sepia');
  await browser.close().catch(() => {});
  child = spawn(exe, [], {env:{...env,MDVIEWER_DEBUG_PORT:'19348'},stdio:'pipe'});
  browser = await connect(child, 19348);
  const restored = browser.contexts()[0].pages()[0];
  await restored.locator('#content h1').waitFor();
  assert.deepEqual(await restored.evaluate(() => ({width:innerWidth,height:innerHeight})), initialViewport, 'native menu must not shrink restored client bounds');
  assert.equal(await restored.locator('#zoom').innerText(), '110%');
  await restored.evaluate(() => window.receive({type:'command',command:'exit'}));
  await new Promise(resolve => child.once('exit', resolve));
  child = null;
  const log = await readFile(resolve(appdata, 'MdViewer', 'mdviewer.log'), 'utf8');
  assert.ok(!/failed|error|busy/i.test(log), log);
  console.log('PASS native: embedded protocol, worker rendering, atomic reload, scroll preservation, silent deletion/reappearance, single-instance forwarding/deduplication, Ctrl+Tab, Ctrl+W, zoom accelerator, settings and clean exit.');
  console.log(log);
} catch (error) {
  console.error('NATIVE LOG:', await readFile(resolve(appdata, 'MdViewer', 'mdviewer.log'), 'utf8').catch(String));
  if (browser) {
    for (const context of browser.contexts()) for (const page of context.pages()) {
      console.error('PAGE STATE:', await page.locator('body').innerText({timeout:2000}).catch(String));
    }
  }
  throw error;
} finally {
  if (child && child.exitCode == null) child.kill();
  await browser?.close().catch(() => {});
  await new Promise(resolve => setTimeout(resolve, 300));
  await rm(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
}
