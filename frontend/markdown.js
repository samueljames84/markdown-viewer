import MarkdownIt from 'markdown-it';
import tasks from 'markdown-it-task-lists';
import footnote from 'markdown-it-footnote';
import sub from 'markdown-it-sub';
import sup from 'markdown-it-sup';
import ins from 'markdown-it-ins';
import mark from 'markdown-it-mark';
import deflist from 'markdown-it-deflist';
import abbr from 'markdown-it-abbr';
import { full as emoji } from 'markdown-it-emoji';

const md = new MarkdownIt({ html: true, linkify: true, typographer: false })
  .use(tasks, { enabled: false, label: false }).use(footnote).use(sub).use(sup)
  .use(ins).use(mark).use(deflist).use(abbr).use(emoji);

for (const rule of ['th_open', 'td_open']) {
  md.renderer.rules[rule] = (tokens, index, options, _env, renderer) => {
    const token = tokens[index], align = token.attrGet('style')?.match(/^text-align:(left|center|right);?$/)?.[1];
    if (align) {
      token.attrs = token.attrs.filter(([name]) => name !== 'style');
      token.attrSet('class', `align-${align}`);
    }
    return renderer.renderToken(tokens, index, options);
  };
}

md.inline.ruler.before('escape', 'math_inline', (state, silent) => {
  const start = state.pos;
  if (state.src[start] !== '$' || state.src[start + 1] === '$' || /\s/.test(state.src[start + 1] || ' ')) return false;
  let end = start + 1;
  while ((end = state.src.indexOf('$', end)) !== -1) {
    let slashes = 0;
    for (let i = end - 1; i > start && state.src[i] === '\\'; i--) slashes++;
    if (!(slashes % 2) && !/\s/.test(state.src[end - 1]) && !/\d/.test(state.src[end + 1] || '')) break;
    end++;
  }
  if (end < 0 || state.src.slice(start, end).includes('\n')) return false;
  if (!silent) { const token = state.push('math_inline', 'math', 0); token.content = state.src.slice(start + 1, end); }
  state.pos = end + 1;
  return true;
});

md.block.ruler.before('fence', 'math_block', (state, start, end, silent) => {
  const text = state.src.slice(state.bMarks[start] + state.tShift[start], state.eMarks[start]);
  if (!text.startsWith('$$')) return false;
  if (silent) return true;
  let content = text.slice(2);
  let next = start;
  if (content.trimEnd().endsWith('$$')) content = content.trimEnd().slice(0, -2);
  else {
    let closed = false;
    while (++next < end) {
      const line = state.src.slice(state.bMarks[next] + state.tShift[next], state.eMarks[next]);
      if (line.trimEnd().endsWith('$$')) { content += '\n' + line.trimEnd().slice(0, -2); closed = true; break; }
      content += '\n' + line;
    }
    if (!closed) return false;
  }
  const token = state.push('math_block', 'math', 0);
  token.block = true; token.content = content; token.map = [start, next + 1];
  state.line = next + 1;
  return true;
}, { alt: ['paragraph', 'reference', 'blockquote', 'list'] });

md.block.ruler.before('paragraph', 'toc', (state, start, _end, silent) => {
  const line = state.src.slice(state.bMarks[start] + state.tShift[start], state.eMarks[start]).trim();
  if (!/^\[toc\]$/i.test(line)) return false;
  if (!silent) { state.push('toc', 'nav', 0); state.line = start + 1; }
  return true;
});

const escape = md.utils.escapeHtml;
function walk(tokens, fn) { for (const token of tokens) { fn(token); if (token.children) walk(token.children, fn); } }
export function parse(source) {
  source = source.replace(/^\uFEFF/, '').replace(/^---[ \t]*\r?\n(?:[\s\S]*?\r?\n)?(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/, '');
  const env = { headings: [] };
  const tokens = md.parse(source, env);
  const seen = new Set();
  for (let i = 0; i < tokens.length; i++) {
    if (tokens[i].type !== 'heading_open') continue;
    const text = (tokens[i + 1].children || []).filter(t => ['text', 'code_inline', 'emoji', 'math_inline'].includes(t.type)).map(t => t.content).join('');
    const base = text.toLowerCase().trim().replace(/[^\p{L}\p{N}\s_-]/gu, '').replace(/\s+/g, '-') || 'section';
    let id = base, n = 0;
    while (seen.has(id)) id = `${base}-${++n}`;
    seen.add(id);
    tokens[i].attrSet('id', id);
    env.headings.push({ id, text, level: Number(tokens[i].tag.slice(1)) });
  }
  let math = false, code = false, mermaid = false;
  walk(tokens, t => {
    if (t.type.startsWith('math_')) math = true;
    if (t.type === 'fence') { if (t.info.trim().split(/\s/)[0].toLowerCase() === 'mermaid') mermaid = true; else if (t.info.trim()) code = true; }
  });
  return { tokens, env, needs: { math, code, mermaid } };
}

export async function render(source) {
  const parsed = parse(source);
  const math = [], nonce = crypto.randomUUID();
  const katex = parsed.needs.math ? (await import('./katex.js')).default : null;
  const hl = parsed.needs.code ? (await import('./highlight.js')).default : null;
  const renderMath = (tokens, index) => {
    const t = tokens[index];
    try {
      const html = katex.renderToString(t.content, { displayMode: t.type === 'math_block', throwOnError: true, trust: false, strict: 'warn', maxExpand: 1000, output: 'htmlAndMathml' });
      const key = `${nonce}-${math.length}`; math.push({ key, html });
      return `<span data-md-math="${key}"></span>`;
    }
    catch (e) { return `<code class="render-error" title="${escape(String(e))}">${escape(t.content)}</code>`; }
  };
  md.renderer.rules.math_inline = renderMath;
  md.renderer.rules.math_block = renderMath;
  md.renderer.rules.toc = (_tokens, _index, _options, env) => `<nav class="toc" aria-label="Table of contents">${env.headings.map(h => `<div class="toc-level-${h.level}"><a href="#${escape(h.id)}">${escape(h.text)}</a></div>`).join('')}</nav>`;
  md.renderer.rules.fence = (tokens, index) => {
    const token = tokens[index], language = token.info.trim().split(/\s+/)[0].toLowerCase();
    if (language === 'mermaid') return `<pre class="mermaid-source"><code>${escape(token.content)}</code></pre>`;
    let code = escape(token.content);
    if (hl && hl.getLanguage(language)) {
      try { code = hl.highlight(token.content, { language, ignoreIllegals: true }).value; }
      catch (e) { return `<pre><code>${code}</code></pre><small class="render-error">${escape(String(e))}</small>`; }
    }
    return `<pre><code class="hljs">${code}</code></pre>`;
  };
  const blocks = [];
  let depth = 0, start = 0, chunk = '';
  for (let i = 0; i < parsed.tokens.length; i++) {
    depth += parsed.tokens[i].nesting;
    if (depth !== 0) continue;
    chunk += md.renderer.render(parsed.tokens.slice(start, i + 1), md.options, parsed.env);
    start = i + 1;
    if (chunk.length >= 24000) { blocks.push(chunk); chunk = ''; }
  }
  if (start < parsed.tokens.length) chunk += md.renderer.render(parsed.tokens.slice(start), md.options, parsed.env);
  if (chunk) blocks.push(chunk);
  return { blocks, math, needs: parsed.needs };
}
