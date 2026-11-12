export class Find {
  constructor(content, viewport, count) { this.content = content; this.viewport = viewport; this.count = count; this.ranges = []; this.current = -1; this.generation = 0; }
  clear() {
    this.generation++;
    if (globalThis.CSS?.highlights) { CSS.highlights.delete('find-all'); CSS.highlights.delete('find-current'); }
    for (const node of this.content.querySelectorAll('mark[data-find]')) node.replaceWith(...node.childNodes);
    this.content.normalize();
    this.ranges = []; this.current = -1; this.update();
  }
  async search(query, matchCase, scrollToMatch = true) {
    this.clear();
    if (!query) return;
    const generation = this.generation, nodes = [], parts = [];
    const walker = document.createTreeWalker(this.content, NodeFilter.SHOW_TEXT, { acceptNode: node =>
      node.parentElement.closest('script,style,[hidden],.katex-mathml,annotation') || !node.parentElement.getClientRects().length ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT });
    let node, offset = 0, i = 0;
    while ((node = walker.nextNode())) {
      nodes.push({ node, start: offset }); parts.push(node.data); offset += node.length;
      if (++i % 2000 === 0) { await new Promise(resolve => setTimeout(resolve, 0)); if (generation !== this.generation) return; }
    }
    const original = parts.join('');
    // Length-preserving case folding keeps DOM offsets valid for Unicode.
    const fold = text => Array.from(text, c => { const f = c.toLowerCase(); return f.length === c.length ? f : c; }).join('');
    const text = matchCase ? original : fold(original), needle = matchCase ? query : fold(query);
    let at = 0, startNode = 0, endNode = 0;
    const ranges = [];
    while ((at = text.indexOf(needle, at)) !== -1) {
      const end = at + needle.length;
      while (startNode + 1 < nodes.length && nodes[startNode + 1].start <= at) startNode++;
      endNode = Math.max(endNode, startNode);
      while (endNode + 1 < nodes.length && nodes[endNode + 1].start < end) endNode++;
      const range = document.createRange();
      range.setStart(nodes[startNode].node, at - nodes[startNode].start);
      range.setEnd(nodes[endNode].node, end - nodes[endNode].start);
      ranges.push(range); at = end;
      if (ranges.length % 2000 === 0) { await new Promise(resolve => setTimeout(resolve, 0)); if (generation !== this.generation) return; }
    }
    if (generation !== this.generation) return;
    this.ranges = ranges;
    if (globalThis.CSS?.highlights) CSS.highlights.set('find-all', new Highlight(...ranges));
    else {
      for (const range of [...ranges].reverse()) {
        const mark = document.createElement('mark'); mark.dataset.find = ''; mark.className = 'find-all';
        mark.append(range.extractContents()); range.insertNode(mark);
      }
      this.ranges = Array.from(this.content.querySelectorAll('mark[data-find]'), node => {
        const range = document.createRange(); range.selectNodeContents(node); return range;
      });
    }
    this.next(1, scrollToMatch);
  }
  next(direction, scroll = true) {
    if (!this.ranges.length) { this.update(); return; }
    this.current = (this.current + direction + this.ranges.length) % this.ranges.length;
    if (globalThis.CSS?.highlights) CSS.highlights.set('find-current', new Highlight(this.ranges[this.current]));
    else {
      this.content.querySelector('.find-current')?.classList.remove('find-current');
      this.ranges[this.current].startContainer.parentElement.closest('mark')?.classList.add('find-current');
    }
    if (scroll) {
      const rect = this.ranges[this.current].getBoundingClientRect(), view = this.viewport.getBoundingClientRect();
      if (rect.top < view.top || rect.bottom > view.bottom) this.viewport.scrollTop += rect.top - view.top - view.height / 3;
    }
    this.update();
  }
  update() { this.count.textContent = `${this.ranges.length ? this.current + 1 : 0} of ${this.ranges.length}`; }
}
