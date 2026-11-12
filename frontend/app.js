import { applyTheme, complement, contrast, luminance } from './themes.js';
import { Find } from './find.js';
import { sanitize, trustedMath, trustedDiagram } from './sanitize.js';

const $ = id => document.getElementById(id);
const viewport = $('viewport'), content = $('content'), tabstrip = $('tabs');
const find = new Find(content, viewport, $('count'));
const worker = new Worker('/renderer.worker.js', { type: 'module' });
const cache = new Map(), revisions = new Map(), pending = new Map();
let tabs = [], active = null, theme = 'light', colors = ['#ffffff', '#2864b4'], zoom = 100;
let generation = 0, scrollTimer, findTimer, dragging = null, currentPalette, anchor = null;
let preview = null, diagramSerial = 0, restoring = false, scrollEpoch = 0;
let firstRender = true;
let previewTimer;
let mathStyles;
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
function send(value) { window.ipc.postMessage(JSON.stringify(value)); }
function report(error) {
  const message = String(error);
  $('notice').textContent = message; $('notice').title = message;
  send({ type: 'diagnostic', message: message.split('\n')[0].slice(0, 512) });
}
window.addEventListener('error', event => report(event.message));
window.addEventListener('unhandledrejection', event => report(event.reason));
function persistPreferences() { send({ type: 'preferences', theme, colors, zoom }); }
function saveScroll() {
  clearTimeout(scrollTimer);
  if (active == null || restoring) return;
  const tab = tabs.find(t => t.id === active);
  if (tab) { tab.scroll = viewport.scrollTop; send({ type: 'scroll', id: active, scroll: tab.scroll }); }
}
function preferences() {
  currentPalette = applyTheme(theme, colors);
  document.documentElement.style.setProperty('--zoom', zoom / 100);
  $('zoom').textContent = `${zoom}%`;
}
function search(scroll = true) {
  clearTimeout(findTimer);
  if (!$('find-bar').hidden) return find.search($('query').value, $('case').checked, scroll);
  return Promise.resolve();
}
function evict() {
  let total = 0;
  for (const [id, entry] of [...cache].reverse()) {
    total += entry.blocks.reduce((n, b) => n + b.length * 2, 0) + entry.math.reduce((n, m) => n + m.html.length * 2, 0);
    if (total > 32 * 1024 * 1024 && id !== active && !entry.missing) cache.delete(id);
  }
}
function request(id, retries = 0) {
  if (id == null) return;
  const revision = revisions.get(id) || 0;
  if (pending.get(id)?.revision === revision) return;
  pending.set(id, { revision, retries });
  worker.postMessage({ id, revision });
}
worker.onmessage = event => {
  const result = event.data, job = pending.get(result.id);
  if (!job || job.revision !== result.revision) return;
  pending.delete(result.id);
  if (result.cancelled) { if (result.id === active) request(active); return; }
  if ((revisions.get(result.id) || 0) !== result.revision || !tabs.some(t => t.id === result.id)) return;
  if (result.missing) {
    const entry = cache.get(result.id);
    if (entry) entry.missing = true;
    if (active === result.id && entry) void display(entry);
    return;
  }
  if (result.error) {
    if (job.retries < 3) setTimeout(() => {
      if (tabs.some(t => t.id === result.id) && (revisions.get(result.id) || 0) === result.revision) request(result.id, job.retries + 1);
    }, 100 * (job.retries + 1));
    else report(result.error);
    return;
  }
  cache.delete(result.id); cache.set(result.id, { ...result, missing: false });
  if (result.warning) report(result.warning);
  if (active === result.id) void display(cache.get(result.id));
  evict();
};
worker.onerror = event => report(`Markdown worker failed: ${event.message}`);

async function display(entry) {
  const id = active, token = ++generation, epoch = scrollEpoch;
  find.clear(); restoring = true;
  $('notice').textContent = '';
  const scroll = tabs.find(t => t.id === id)?.scroll || 0;
  const math = new Map(entry.math.map(m => [m.key, m.html]));
  if (entry.needs.math && !$('katex-style')) {
    const link = document.createElement('link'); link.id = 'katex-style'; link.rel = 'stylesheet'; link.href = '/vendor/katex.css';
    mathStyles = new Promise((resolve, reject) => {
      link.onload = resolve; link.onerror = () => reject(new Error('Bundled math stylesheet could not load'));
    });
    document.head.append(link);
  }
  content.replaceChildren(); $('empty').hidden = true;
  for (const html of entry.blocks) {
    const fragment = sanitize(html, id);
    for (const placeholder of fragment.querySelectorAll('[data-md-math]')) {
      const trusted = math.get(placeholder.dataset.mdMath);
      if (trusted) placeholder.replaceChildren(trustedMath(trusted));
      placeholder.removeAttribute('data-md-math');
    }
    content.append(fragment);
    await tick(); if (token !== generation) return;
  }
  viewport.scrollTop = scroll;
  restoring = false;
  const settle = () => {
    if (token === generation && epoch === scrollEpoch) {
      restoring = true; viewport.scrollTop = scroll; restoring = false; applyAnchor(); void search(false);
    }
  };
  for (const image of content.querySelectorAll('img')) image.addEventListener('load', settle, { once: true });
  document.fonts.ready.then(settle);
  applyAnchor(); void search(false);
  if (entry.needs.mermaid) await diagrams(token, settle);
  if (entry.needs.math) { await mathStyles; await document.fonts.ready; }
  if (token === generation) {
    settle(); evict();
    if (firstRender) { firstRender = false; send({ type: 'metric', stage: 'active-content' }); }
  }
}

async function diagrams(token, settle) {
  const sources = [...content.querySelectorAll('.mermaid-source')];
  if (!sources.length) return;
  const mermaid = (await import('/vendor/mermaid/mermaid.js')).default;
  if (token !== generation) return;
  mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', suppressErrorRendering: true,
    htmlLabels: false, flowchart: { htmlLabels: false }, maxTextSize: 200000, maxEdges: 2000,
    secure: ['securityLevel', 'startOnLoad', 'htmlLabels', 'maxTextSize', 'maxEdges', 'secure'],
    theme: 'base', themeVariables: { darkMode: luminance(currentPalette.base) < 0.18,
      background: currentPalette.base, primaryColor: currentPalette.surface, primaryTextColor: currentPalette.text,
      primaryBorderColor: currentPalette.accent, lineColor: currentPalette.muted, textColor: currentPalette.text,
      secondaryColor: currentPalette.code, tertiaryColor: currentPalette.stripe } });
  for (const source of sources) {
    await tick(); if (token !== generation) return;
    try {
      const code = source.textContent.replace(/%%\{[\s\S]*?\}%%/g, '').replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '');
      const { svg } = await mermaid.render(`diagram-${++diagramSerial}`, code);
      if (token !== generation) return;
      const div = document.createElement('div'); div.className = 'mermaid'; div.append(trustedDiagram(svg)); source.replaceWith(div);
      settle();
    } catch (error) {
      if (token !== generation) return;
      source.classList.add('render-error');
      const message = document.createElement('div'); message.textContent = `Diagram could not render: ${String(error)}`; source.before(message);
      report(error);
    }
  }
}
function applyAnchor() {
  if (anchor?.id !== active) return;
  let value;
  try { value = decodeURIComponent(anchor.anchor); } catch { value = anchor.anchor; }
  const element = content.querySelector(`#${CSS.escape(`doc-${active}-${value}`)}`);
  if (element) { element.scrollIntoView(); anchor = null; }
}
function drawTabs() {
  tabstrip.replaceChildren();
  for (const tab of tabs) {
    const item = document.createElement('div'); item.className = 'tab'; item.role = 'tab';
    item.dataset.id = tab.id; item.title = tab.display; item.draggable = true;
    item.setAttribute('aria-selected', String(tab.id === active)); item.tabIndex = tab.id === active ? 0 : -1;
    const name = document.createElement('span'); name.className = 'name'; name.textContent = tab.name;
    const close = document.createElement('button'); close.className = 'close'; close.textContent = 'x'; close.title = `Close ${tab.name}`; close.setAttribute('aria-label', `Close ${tab.name}`);
    close.onclick = event => { event.stopPropagation(); saveScroll(); send({ type: 'close', id: tab.id }); };
    item.append(name, close);
    item.onclick = () => { saveScroll(); send({ type: 'activate', id: tab.id }); };
    item.onauxclick = event => { if (event.button === 1) { event.preventDefault(); saveScroll(); send({ type: 'close', id: tab.id }); } };
    item.oncontextmenu = event => { event.preventDefault(); send({ type: 'context', id: tab.id }); };
    item.onkeydown = event => {
      if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); item.click(); }
      if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') { event.preventDefault(); cycle(event.key === 'ArrowRight' ? 1 : -1); }
    };
    item.ondragstart = event => { dragging = tab.id; event.dataTransfer.effectAllowed = 'move'; event.dataTransfer.setData('text/plain', String(tab.id)); };
    item.ondragend = () => { dragging = null; };
    item.ondragover = event => { if (dragging != null) event.preventDefault(); };
    item.ondrop = event => {
      if (dragging == null) return;
      event.preventDefault(); event.stopPropagation(); saveScroll();
      const ids = tabs.map(t => t.id).filter(id => id !== dragging);
      ids.splice(ids.indexOf(tab.id) + (event.clientX > item.getBoundingClientRect().left + item.clientWidth / 2 ? 1 : 0), 0, dragging);
      dragging = null; send({ type: 'reorder', ids });
    };
    tabstrip.append(item);
  }
  tabstrip.querySelector('[aria-selected=true]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  updateArrows();
}
function updateArrows() {
  $('left').hidden = $('right').hidden = tabstrip.scrollWidth <= tabstrip.clientWidth + 1;
}
function cycle(direction) {
  if (!tabs.length) return;
  saveScroll();
  const index = tabs.findIndex(t => t.id === active);
  send({ type: 'activate', id: tabs[(index + direction + tabs.length) % tabs.length].id });
}
function showFind() {
  const hidden = $('find-bar').hidden;
  $('find-bar').hidden = false; $('query').focus(); $('query').select();
  return hidden ? search() : Promise.resolve();
}
function hideFind() { $('find-bar').hidden = true; find.clear(); viewport.focus(); }
function changeZoom(value) { saveScroll(); zoom = Math.max(50, Math.min(300, value)); preferences(); persistPreferences(); }
function command(name) {
  switch (name) {
    case 'open': case 'open-new': send({ type: 'command', command: name }); break;
    case 'close': if (active != null) { saveScroll(); send({ type: 'close', id: active }); } break;
    case 'close-all': saveScroll(); for (const tab of tabs) send({ type: 'close', id: tab.id }); break;
    case 'exit': saveScroll(); send({ type: 'exitReady' }); break;
    case 'reload': if (active != null) invalidate([active]); break;
    case 'find': showFind(); break;
    case 'find-next': if ($('find-bar').hidden) void showFind(); else find.next(1); break;
    case 'find-previous': if ($('find-bar').hidden) void showFind().then(() => find.next(-1)); else find.next(-1); break;
    case 'zoom-in': changeZoom(zoom + 10); break;
    case 'zoom-out': changeZoom(zoom - 10); break;
    case 'zoom-reset': changeZoom(100); break;
    case 'light': case 'dark':
      saveScroll(); theme = name; preferences(); persistPreferences();
      if (cache.has(active)) void display(cache.get(active)); break;
    case 'custom': openCustom(); break;
  }
}
function invalidate(ids, missing = []) {
  saveScroll();
  for (const id of ids) {
    if (cache.has(id)) cache.get(id).missing = missing.includes(id);
    revisions.set(id, (revisions.get(id) || 0) + 1);
    if (id === active) request(id);
  }
}
window.receive = message => {
  switch (message.type) {
    case 'tabs': {
      const previous = active, existing = new Map(tabs.map(t => [t.id, t]));
      tabs = message.tabs.map(t => ({ ...t, scroll: existing.get(t.id)?.scroll ?? t.scroll }));
      active = message.active; theme = message.theme; colors = message.colors; zoom = message.zoom;
      preferences(); drawTabs();
      $('path').textContent = tabs.find(t => t.id === active)?.display || ''; $('path').title = $('path').textContent;
      for (const id of cache.keys()) if (!tabs.some(t => t.id === id)) cache.delete(id);
      if (active == null) { generation++; content.replaceChildren(); find.clear(); $('empty').hidden = false; }
      else if (active !== previous) {
        generation++;
        const entry = cache.get(active);
        if (entry && (entry.revision === (revisions.get(active) || 0) || entry.missing)) {
          cache.delete(active); cache.set(active, entry); void display(entry);
          if (entry.missing || entry.revision !== (revisions.get(active) || 0)) request(active);
        } else {
          find.clear(); content.replaceChildren(); $('empty').hidden = true; restoring = true; request(active);
        }
      }
      break;
    }
    case 'invalidate': invalidate(message.ids, message.missing); break;
    case 'command': command(message.command); break;
    case 'anchor': anchor = message; applyAnchor(); break;
    case 'copy': navigator.clipboard.writeText(message.text).catch(report); break;
    case 'color': {
      const input = $(message.field); if (['base', 'accent'].includes(message.field)) { input.value = message.value; previewCustom(); } break;
    }
  }
};

function openCustom() {
  preview = { theme, colors: [...colors] }; $('base').value = colors[0]; $('accent').value = colors[1];
  $('custom').showModal(); previewCustom();
}
function previewCustom() {
  const values = [$('base').value, $('accent').value];
  if (!values.every(c => /^#[0-9a-f]{6}$/i.test(c))) { $('contrast').textContent = 'Use two valid #RRGGBB colors.'; return; }
  currentPalette = applyTheme('custom', values);
  const ratio = contrast(...values);
  $('contrast').textContent = ratio < 4.5 ? `Raw accent contrast is ${ratio.toFixed(2)}:1. Link/text colors are adjusted to meet 4.5:1.` : `Base/accent contrast: ${ratio.toFixed(2)}:1.`;
  clearTimeout(previewTimer);
  if (cache.get(active)?.needs.mermaid) previewTimer = setTimeout(() => { saveScroll(); void display(cache.get(active)); }, 200);
}
$('custom').addEventListener('close', () => {
  clearTimeout(previewTimer);
  if (!preview) return;
  if ($('custom').returnValue === 'apply') { theme = 'custom'; colors = [$('base').value.toLowerCase(), $('accent').value.toLowerCase()]; persistPreferences(); }
  else { theme = preview.theme; colors = preview.colors; }
  preview = null; preferences(); if (cache.has(active)) void display(cache.get(active));
});
for (const field of ['base', 'accent']) {
  $(field).oninput = previewCustom;
  $(`pick-${field}`).onclick = () => send({ type: 'pickColor', field, value: $(field).value });
}
$('complement').onclick = () => { if (/^#[0-9a-f]{6}$/i.test($('base').value)) { $('accent').value = complement($('base').value); previewCustom(); } };
for (const id of ['open', 'empty-open', 'new']) $(id).onclick = () => command('open');
$('left').onclick = () => tabstrip.scrollBy({ left: -180 }); $('right').onclick = () => tabstrip.scrollBy({ left: 180 });
new ResizeObserver(updateArrows).observe(tabstrip);
$('query').oninput = () => { clearTimeout(findTimer); findTimer = setTimeout(search, 80); };
$('case').onchange = search;
$('next').onclick = () => find.next(1); $('previous').onclick = () => find.next(-1); $('close-find').onclick = hideFind;
viewport.addEventListener('wheel', event => {
  if (event.ctrlKey) { event.preventDefault(); changeZoom(zoom + (event.deltaY < 0 ? 10 : -10)); }
  else { scrollEpoch++; restoring = false; }
}, { passive: false });
viewport.addEventListener('pointerdown', () => { scrollEpoch++; restoring = false; });
viewport.addEventListener('scroll', () => { clearTimeout(scrollTimer); if (!restoring) scrollTimer = setTimeout(saveScroll, 150); }, { passive: true });
content.addEventListener('click', event => {
  const link = event.target.closest('a[href]'); if (!link) return;
  event.preventDefault(); const href = link.getAttribute('href');
  if (href.startsWith('#')) { anchor = { id: active, anchor: href.slice(1) }; applyAnchor(); }
  else send({ type: 'link', id: active, href });
});
document.addEventListener('keydown', event => {
  let action;
  if (event.ctrlKey && !event.altKey) {
    const key = event.key.toLowerCase();
    if (key === 'tab') { event.preventDefault(); cycle(event.shiftKey ? -1 : 1); return; }
    action = { o: 'open', t: 'open-new', w: 'close', f: 'find', '=': 'zoom-in', '+': 'zoom-in', '-': 'zoom-out', '0': 'zoom-reset' }[key];
    if (['s', 'p', 'e', 'u'].includes(key)) event.preventDefault();
  } else if (event.key === 'F3') action = event.shiftKey ? 'find-previous' : 'find-next';
  else if (event.key === 'F5') action = 'reload';
  else if (event.key === 'Escape' && !$('find-bar').hidden && !$('custom').open) { event.preventDefault(); hideFind(); }
  else if (event.key === 'Enter' && event.target === $('query')) { event.preventDefault(); find.next(event.shiftKey ? -1 : 1); }
  if (action) { event.preventDefault(); command(action); }
});
preferences();
send({ type: 'metric', stage: 'shell' });
send({ type: 'ready' });
