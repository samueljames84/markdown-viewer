import purify from './vendor/purify.js';

export function sanitize(html, id) {
  const fragment = purify.sanitize(html, {
    RETURN_DOM_FRAGMENT: true, USE_PROFILES: { html: true },
    FORBID_TAGS: ['style', 'form', 'button', 'textarea', 'select', 'option', 'iframe', 'object', 'embed', 'audio', 'video', 'source', 'link', 'meta', 'base'],
    FORBID_ATTR: ['style', 'contenteditable', 'autofocus', 'srcset', 'name', 'form', 'tabindex'],
    ADD_ATTR: ['data-md-math'], ALLOW_DATA_ATTR: false, ALLOW_ARIA_ATTR: true,
  });
  for (const input of fragment.querySelectorAll('input')) {
    if (input.type !== 'checkbox') input.remove();
    else { input.disabled = true; input.tabIndex = -1; }
  }
  for (const element of fragment.querySelectorAll('[id]')) element.id = `doc-${id}-${element.id}`;
  for (const link of fragment.querySelectorAll('a')) {
    const href = link.getAttribute('href') || '';
    if (/^(?:https?:\/\/|#)/i.test(href) || (!/[:\\]/.test(href) && !href.startsWith('/'))) {
      link.setAttribute('href', href); link.removeAttribute('target'); link.removeAttribute('download');
    } else link.removeAttribute('href');
  }
  for (const image of fragment.querySelectorAll('img')) {
    const src = image.getAttribute('src') || '';
    image.removeAttribute('src');
    if (src && !/[:\\]/.test(src) && !src.startsWith('/')) {
      // One encoded path segment prevents URL normalization from hiding traversal.
      try { image.setAttribute('src', `/image/${id}/${encodeURIComponent(decodeURIComponent(src.split(/[?#]/)[0]))}`); }
      catch { image.setAttribute('alt', 'Invalid image URL'); }
      image.loading = 'lazy'; image.decoding = 'async';
    } else image.setAttribute('alt', `${image.getAttribute('alt') || 'Image'} (remote/unsupported image blocked)`);
  }
  return fragment;
}

export function trustedMath(html) {
  return purify.sanitize(html, { RETURN_DOM_FRAGMENT: true, USE_PROFILES: { html: true, mathMl: true }, ADD_ATTR: ['style'], FORBID_TAGS: ['style', 'form', 'input'] });
}
export function trustedDiagram(svg) {
  return purify.sanitize(svg, { RETURN_DOM_FRAGMENT: true, USE_PROFILES: { svg: true, svgFilters: true }, FORBID_TAGS: ['foreignObject', 'a', 'script'], FORBID_ATTR: ['onclick', 'href', 'xlink:href'] });
}
