import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, dirname, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const csp = "default-src 'none'; script-src 'self'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";
export async function server() {
  const documents = new Map();
  const instance = createServer(async (req, res) => {
    try {
      const url = new URL(req.url, 'http://localhost'), path = decodeURIComponent(url.pathname);
      res.setHeader('Content-Security-Policy', csp); res.setHeader('Cache-Control', 'no-store');
      if (path === '/test-bridge.js') {
        res.setHeader('Content-Type', 'text/javascript');
        res.end(`window.testMessages=[];window.ipc={postMessage:s=>{const m=JSON.parse(s);testMessages.push(m);if(m.type==='ready')setTimeout(()=>window.receive({type:'tabs',tabs:[{id:1,name:'plain.md',display:'D:\\\\\\\\test\\\\\\\\plain.md',scroll:0},{id:2,name:'features.md',display:'D:\\\\\\\\test\\\\\\\\features.md',scroll:0}],active:1,theme:'light',colors:['#ffffff','#2864b4'],customColors:{},zoom:100}),0)}};`); return;
      }
      if (path.startsWith('/document/')) {
        const name = path === '/document/1' ? 'plain.md' : 'features.md';
        res.setHeader('Content-Type', 'application/json');
        res.end(JSON.stringify({ source: documents.get(path) ?? await readFile(resolve(root, 'tests', 'fixtures', name), 'utf8') })); return;
      }
      const name = path === '/' ? 'index.html' : path.slice(1);
      const base = resolve(root, 'frontend'), file = resolve(base, name);
      if (!file.startsWith(base + sep)) { res.writeHead(403).end(); return; }
      const mime = { '.js':'text/javascript', '.css':'text/css', '.html':'text/html', '.woff2':'font/woff2', '.woff':'font/woff', '.ttf':'font/ttf' }[extname(file)] || 'application/octet-stream';
      res.setHeader('Content-Type', mime);
      let bytes = await readFile(file);
      if (name === 'index.html') bytes = bytes.toString().replace('<head>', '<head><script src="/test-bridge.js"></script>');
      res.end(bytes);
    } catch (error) { res.writeHead(404, { 'Content-Type': 'text/plain' }).end(String(error)); }
  });
  await new Promise(resolve => instance.listen(0, '127.0.0.1', resolve));
  instance.documents = documents;
  return instance;
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const instance = await server();
  console.log(`http://127.0.0.1:${instance.address().port}`);
}
