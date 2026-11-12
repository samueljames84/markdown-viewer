import { render } from './vendor/markdown.js';
const jobs = new Map();
let busy = false;
let wanted = null;
self.onmessage = event => {
  wanted = event.data;
  for (const queued of jobs.values()) self.postMessage({ ...queued, cancelled: true });
  jobs.clear();
  jobs.set(event.data.id, event.data);
  if (!busy) void run();
};
async function run() {
  busy = true;
  while (jobs.size) {
    const job = [...jobs.values()].at(-1); jobs.delete(job.id);
    try {
      const response = await fetch(`/document/${job.id}`, { cache: 'no-store' });
      const document = await response.json();
      if (wanted.id !== job.id || wanted.revision !== job.revision) { self.postMessage({ ...job, cancelled: true }); continue; }
      if (!response.ok) { self.postMessage({ ...job, missing: document.missing, error: document.error }); continue; }
      const output = await render(document.source);
      self.postMessage({ id: job.id, revision: job.revision, ...output, warning: document.warning });
    } catch (error) { self.postMessage({ ...job, error: String(error) }); }
  }
  busy = false;
}
