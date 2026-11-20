import { test, after, before } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { server } from '../../tools/serve.mjs';
import { contrast, palette, complement } from '../../frontend/themes.js';
import { parse } from '../../frontend/vendor/markdown.js';
const require = createRequire(new URL('../../tools/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
let http, browser, page;
before(async () => { http = await server(); browser = await chromium.launch({ channel: 'msedge', headless: true }); });
after(async () => { await browser?.close(); http?.close(); });

test('derived palette maintains WCAG AA text contrast', () => {
  for (const base of ['#ffffff', '#000000', '#777777', '#ff0000', '#00ff00', '#0000ff', '#e4bc70']) {
    for (const accent of ['#ffffff', '#000000', '#777777', '#11aabb']) {
      const p = palette(base, accent);
      for (const fg of ['text', 'muted', 'link', 'syntax', 'number', 'comment']) {
        for (const background of [base, p.code, p.surface, p.stripe]) assert.ok(contrast(p[fg], background) >= 4.5, `${base}/${accent}/${fg}`);
      }
      for (const surface of ['code','surface','stripe']) assert.ok(contrast(p.text, p[surface]) >= 4.5);
    }
  }
  assert.equal(complement('#ff0000'), '#00ffff');
  assert.equal(complement('#808080'), '#808080');
  assert.notEqual(palette('#ffffff','#2864b4').code, '#ffffff');
  const sepia = palette('#d9c48e', '#737519');
  for (const fg of ['text', 'muted', 'link', 'syntax', 'number', 'comment']) {
    for (const bg of ['base', 'surface', 'code', 'stripe']) assert.ok(contrast(sepia[fg], sepia[bg]) >= 4.5);
  }
  assert.ok(contrast(sepia['selection-text'], sepia.selection) >= 4.5);
});

test('empty front matter is hidden and heading IDs avoid suffix collisions', () => {
  const parsed = parse('---\n---\n# Foo\n# Foo\n# Foo-1');
  assert.deepEqual(parsed.env.headings.map(h => h.id), ['foo','foo-1','foo-1-1']);
  assert.ok(!parsed.tokens.some(t => t.type === 'hr'));
});
test('plain document is full-width and optional libraries remain unloaded', async () => {
  page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
  const requests = []; page.on('request', request => requests.push(request.url()));
  await page.goto(`http://127.0.0.1:${http.address().port}/`);
  await page.locator('#content h1').waitFor();
  assert.equal(await page.locator('#content h1').innerText(), 'Plain document');
  assert.ok(!requests.some(r => /\/(?:katex|highlight)\.js|\/mermaid\//.test(r)));
  const dimensions = await page.locator('#viewport').boundingBox();
  assert.equal(dimensions.width, 1200);
  assert.ok(dimensions.height > 700);
  assert.equal(await page.locator('[contenteditable=true]').count(), 0);
});

test('find spans inline nodes and navigates without clearing results', async () => {
  await page.keyboard.press('Control+f');
  await page.locator('#query').fill('cross inline search');
  await page.waitForFunction(() => document.getElementById('count').textContent === '1 of 1');
  await page.locator('#query').fill('Find this twice');
  await page.waitForFunction(() => document.getElementById('count').textContent === '1 of 2');
  await page.keyboard.press('F3');
  assert.equal(await page.locator('#count').innerText(), '2 of 2');
  await page.keyboard.press('Escape');
  assert.ok(await page.locator('#find-bar').isHidden());
});

test('zoom obeys limits; custom colors preview, reset to sepia and wait for confirmation', async () => {
  await page.keyboard.press('Control+=');
  assert.equal(await page.locator('#zoom').innerText(), '110%');
  await page.keyboard.press('Control+0');
  for (let i = 0; i < 30; i++) await page.keyboard.press('Control+-');
  assert.equal(await page.locator('#zoom').innerText(), '50%');
  for (let i = 0; i < 30; i++) await page.keyboard.press('Control+=');
  assert.equal(await page.locator('#zoom').innerText(), '300%');
  await page.keyboard.press('Control+0');
  await page.evaluate(() => window.receive({ type:'command', command:'custom' }));
  assert.equal(await page.locator('#theme-colors input[type=color]').count(), 14);
  await page.locator('#hex-base').fill('#777777');
  await page.locator('#hex-accent').fill('#777777');
  assert.match(await page.locator('#contrast').innerText(), /Contrast warning/);
  await page.locator('#close-theme').click();
  await page.waitForFunction(() => getComputedStyle(document.body).backgroundColor === 'rgb(255, 255, 255)');
  const background = await page.locator('body').evaluate(node => getComputedStyle(node).backgroundColor);
  assert.equal(background, 'rgb(255, 255, 255)');
  await page.evaluate(() => window.receive({ type:'command', command:'custom' }));
  await page.locator('#reset-sepia').click();
  assert.equal(await page.locator('#custom').evaluate(node => node.open), true, 'Reset must leave theme dialog open');
  const expectedSepia = palette('#d9c48e', '#737519');
  const editedColors = await page.evaluate(() => Object.fromEntries([...document.querySelectorAll('#theme-colors input[id^="hex-"]')].map(input => [input.id.slice(4), input.value])));
  assert.deepEqual(editedColors, expectedSepia, 'Reset should replace every theme color with its Sepia value');
  assert.equal(await page.locator('body').evaluate(node => getComputedStyle(node).backgroundColor), 'rgb(217, 196, 142)');
  await page.locator('#close-theme').click();
  await page.waitForFunction(() => !document.getElementById('custom').open && getComputedStyle(document.body).backgroundColor === 'rgb(255, 255, 255)');
  await page.evaluate(() => window.receive({ type:'command', command:'custom' }));
  await page.locator('#reset-sepia').click();
  await page.locator('button[value=apply]').click();
  await page.waitForFunction(() => !document.getElementById('custom').open);
  await page.waitForFunction(() => window.testMessages.filter(message => message.type === 'preferences').at(-1).theme === 'sepia');
  assert.equal(await page.locator('#zoom').innerText(), '100%');
  assert.equal(await page.locator('body').evaluate(node => getComputedStyle(node).backgroundColor), 'rgb(217, 196, 142)');
});

test('full renderer supports extensions, sanitized HTML, math and diagrams offline', async () => {
  const remote = [], errors = [];
  page.on('request', req => { if (!req.url().startsWith(`http://127.0.0.1:${http.address().port}/`)) remote.push(req.url()); });
  page.on('pageerror', e => errors.push(String(e)));
  await page.evaluate(() => window.receive({ type:'tabs', tabs:[{id:2,name:'features.md',display:'features.md',scroll:0}],active:2,theme:'dark',colors:['#fffFFF','#2864b4'],zoom:100 }));
  await page.locator('#content .mermaid svg').waitFor({ timeout: 30000 });
  assert.ok(await page.locator('#content .katex').count() >= 2);
  for (const selector of ['table', 's', 'ins', 'mark', 'sub', 'sup', 'dl', 'abbr', '.footnotes', '.toc', '.hljs-keyword']) assert.ok(await page.locator(`#content ${selector}`).count(), selector);
  assert.equal(await page.locator('#content script,#content [contenteditable],#content input:not([type=checkbox])').count(), 0);
  assert.equal(await page.locator('#content input[type=checkbox]:not(:disabled)').count(), 0);
  assert.equal(await page.locator('#content th').nth(1).evaluate(node => getComputedStyle(node).textAlign), 'right');
  assert.ok(!(await page.locator('#content').innerText()).includes('This front matter must be hidden'));
  assert.equal(await page.evaluate(() => window.__untrusted), undefined);
  await page.locator('#content a[href="https://example.com"]').hover();
  assert.equal(await page.locator('#path').innerText(), 'https://example.com');
  await page.locator('#content a[href="plain.md"]').hover();
  assert.equal(await page.locator('#path').innerText(), 'file:///plain.md');
  await page.locator('#content a[href="plain.md"]').dispatchEvent('contextmenu', {button:2});
  const linkContext = await page.evaluate(() => window.testMessages.filter(message => message.type === 'linkContext').at(-1));
  assert.equal(linkContext.href, 'plain.md');
  assert.equal(linkContext.address, 'file:///plain.md');
  assert.equal(linkContext.text, 'Relative Markdown');
  await page.evaluate(() => window.receive({type:'linkContextAction',action:'open',id:2,href:'plain.md'}));
  const openedLink = await page.evaluate(() => window.testMessages.filter(message => message.type === 'link').at(-1));
  assert.deepEqual({id:openedLink.id,href:openedLink.href}, {id:2,href:'plain.md'});
  await page.keyboard.press('Control+f');
  await page.locator('#case').check();
  await page.locator('#query').fill('Render');
  await page.waitForFunction(() => document.getElementById('count').textContent === '1 of 1');
  await page.locator('#query').fill('mc');
  await page.waitForFunction(() => document.getElementById('count').textContent === '1 of 1');
  await page.evaluate(() => window.receive({type:'command',command:'save-pdf'}));
  await page.locator('#print-submit').click();
  await page.waitForFunction(() => testMessages.some(m => m.type === 'documentOutput'));
  assert.equal(await page.locator('#print-content .mermaid svg').count(), 1);
  assert.equal(await page.locator('#print-content .katex').count(), await page.locator('#content .katex').count());
  assert.equal(await page.locator('#print-content table').count(), 1);
  assert.equal(await page.locator('#print-content').evaluate(el => el.classList.contains('numbered')), false);
  await page.evaluate(() => window.receive({type:'outputDone'}));
  assert.deepEqual(remote, []);
  assert.deepEqual(errors, []);
  await page.close();
});

test('print options paginate a white A4 PDF in either orientation', async () => {
  const path = '/document/1';
  http.documents.set(path, '# Print fixture\n\n' + 'Print this paragraph.\n\n'.repeat(160));
  const printPage = await browser.newPage();
  try {
    await printPage.goto(`http://127.0.0.1:${http.address().port}/`);
    await printPage.waitForFunction(() => testMessages.some(m => m.stage === 'active-content'));
    await printPage.evaluate(() => {
      window.receive({type:'tabs',tabs:[{id:1,name:'print.md',display:'print.md',scroll:0}],active:1,
        theme:'dark',colors:['#ffffff','#2864b4'],zoom:200});
    });
    await printPage.keyboard.press('Control+p');
    await printPage.locator('#print-options').waitFor();
    assert.equal(await printPage.locator('#print-numbers').isChecked(), false);
    await printPage.locator('#print-cancel').click();
    assert.equal(await printPage.evaluate(() => testMessages.some(m => m.type === 'documentOutput')), false);
    await printPage.keyboard.press('Control+Shift+s');
    assert.equal(await printPage.locator('#pdf-orientation').inputValue(), 'portrait');
    await printPage.locator('#print-numbers').check();
    await printPage.locator('#print-submit').click();
    await printPage.waitForFunction(() => testMessages.some(m => m.type === 'documentOutput' && m.mode === 'pdf' && m.id === 1));
    assert.equal(await printPage.locator('#print-content p').count(), 160);
    await printPage.emulateMedia({media:'print'});
    assert.equal(await printPage.locator('#tabs-bar').isHidden(), true);
    assert.equal(await printPage.locator('#print-content').evaluate(el => getComputedStyle(el).backgroundColor), 'rgb(255, 255, 255)');
    assert.equal(await printPage.locator('#print-content').evaluate(el => getComputedStyle(el).page), 'numbered');
    const pdf = (await printPage.pdf({preferCSSPageSize:true})).toString('latin1');
    assert.ok((pdf.match(/\/Type \/Page\b/g) || []).length > 1, 'output must paginate the entire document');
    assert.match(pdf, /\/MediaBox \[0 0 59[45](?:\.\d+)? 84[12](?:\.\d+)?\]/);
    await printPage.evaluate(() => window.receive({type:'outputDone',message:'PDF saved'}));
    await printPage.emulateMedia({media:'screen'});
    assert.equal(await printPage.locator('#print-content p').count(), 0);
    assert.equal(await printPage.locator('#zoom').innerText(), '200%');

    await printPage.keyboard.press('Control+Shift+s');
    await printPage.locator('#pdf-orientation').selectOption('landscape');
    await printPage.locator('#print-submit').click();
    await printPage.waitForFunction(() => testMessages.some(m => m.type === 'documentOutput' && m.mode === 'pdf' && m.orientation === 'landscape'));
    await printPage.emulateMedia({media:'print'});
    assert.equal(await printPage.locator('#print-content').evaluate(el => getComputedStyle(el).page), 'landscape-numbered');
    const landscapePdf = (await printPage.pdf({preferCSSPageSize:true})).toString('latin1');
    const landscapeBoxes = landscapePdf.match(/\/MediaBox \[[^\]]+\]/g) || [];
    assert.ok(landscapeBoxes.length > 1, 'landscape output must also paginate');
    for (const box of landscapeBoxes) assert.match(box, /\/MediaBox \[0 0 84[12](?:\.\d+)? 59[45](?:\.\d+)?\]/);
    await printPage.evaluate(() => window.receive({type:'outputDone',message:'PDF saved'}));
    await printPage.emulateMedia({media:'screen'});
    await printPage.keyboard.press('Control+Shift+s');
    assert.equal(await printPage.locator('#pdf-orientation').inputValue(), 'portrait');
  } finally {
    await printPage.close();
    http.documents.delete(path);
  }
});

test('several-MB document renders in batches without long UI stalls', async () => {
  const paragraph = 'A substantial paragraph with **bold text**, a [local link](#large-document), and enough ordinary prose to exercise rendering without optional libraries. '.repeat(5);
  const source = '# Large document\n\n' + (paragraph + '\n\n').repeat(4500);
  assert.ok(Buffer.byteLength(source) > 3 * 1024 * 1024);
  http.documents.set('/document/3', source);
  const large = await browser.newPage();
  await large.goto(`http://127.0.0.1:${http.address().port}/`);
  await large.locator('#content h1').waitFor();
  await large.evaluate(() => {
    window.uiStalls = []; let previous = performance.now();
    window.uiTimer = setInterval(() => { const now = performance.now(); window.uiStalls.push(now - previous); previous = now; }, 20);
    window.receive({type:'tabs',tabs:[{id:3,name:'large.md',display:'large.md',scroll:0}],active:3,theme:'light',colors:['#ffffff','#2864b4'],zoom:100});
  });
  await large.waitForFunction(() => document.querySelectorAll('#content p').length === 4500, { timeout: 30000 });
  const longest = await large.evaluate(() => { clearInterval(window.uiTimer); return Math.max(...window.uiStalls); });
  console.log(`3+ MiB document maximum 20ms input-heartbeat gap: ${Math.round(longest)} ms`);
  assert.ok(longest < 250, `UI stalled for ${longest}ms`);
  await large.close();
});
