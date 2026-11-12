export function rgb(hex) { return [1, 3, 5].map(i => Number.parseInt(hex.slice(i, i + 2), 16)); }
export function hex(values) { return '#' + values.map(v => Math.round(v).toString(16).padStart(2, '0')).join(''); }
export function luminance(color) {
  const c = rgb(color).map(v => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; });
  return c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
}
export function contrast(a, b) { const x = luminance(a), y = luminance(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); }
export function mix(a, b, amount) { const x = rgb(a), y = rgb(b); return hex(x.map((c, i) => c * (1 - amount) + y[i] * amount)); }
export function readable(base) { return contrast(base, '#000000') >= contrast(base, '#ffffff') ? '#000000' : '#ffffff'; }
export function palette(base, accent) {
  const text = readable(base);
  const surface = amount => {
    for (let i = amount; i >= 0; i -= 0.005) {
      const color = mix(base, text, i);
      if (contrast(text, color) >= 4.5) return color;
    }
    return base;
  };
  const surfaces = { surface: surface(0.06), code: surface(0.08), stripe: surface(0.025) };
  const onSurfaces = color => {
    for (let i = 0; i <= 100; i++) {
      const next = mix(color, text, i / 100);
      if ([base, ...Object.values(surfaces)].every(background => contrast(next, background) >= 4.5)) return next;
    }
    return text;
  };
  const selection = mix(base, accent, 0.3);
  return { base, accent, text, ...surfaces, border: mix(base, text, 0.23),
    muted: onSurfaces(mix(base, text, 0.6)), link: onSurfaces(accent),
    selection, 'selection-text': readable(selection), syntax: onSurfaces(mix(accent, '#ba4779', 0.5)),
    number: onSurfaces(mix(accent, '#29817b', 0.5)), comment: onSurfaces(mix(base, text, 0.58)) };
}
export function complement(color) {
  let [r, g, b] = rgb(color).map(v => v / 255);
  const max = Math.max(r, g, b), min = Math.min(r, g, b), delta = max - min, l = (max + min) / 2;
  if (!delta) return color;
  const s = delta / (1 - Math.abs(2 * l - 1));
  let h = max === r ? ((g - b) / delta) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
  h = ((h * 60 + 180 + 360) % 360) / 60;
  const c = (1 - Math.abs(2 * l - 1)) * s, x = c * (1 - Math.abs(h % 2 - 1)), m = l - c / 2;
  [r, g, b] = h < 1 ? [c, x, 0] : h < 2 ? [x, c, 0] : h < 3 ? [0, c, x] : h < 4 ? [0, x, c] : h < 5 ? [x, 0, c] : [c, 0, x];
  return hex([r, g, b].map(v => (v + m) * 255));
}
export function applyTheme(mode, colors) {
  const values = mode === 'dark' ? palette('#1e2025', '#8eb8fa') : mode === 'custom' ? palette(...colors) : palette('#ffffff', '#2864b4');
  for (const [name, value] of Object.entries(values)) document.documentElement.style.setProperty(`--${name}`, value);
  document.documentElement.style.colorScheme = luminance(values.base) < 0.18 ? 'dark' : 'light';
  return values;
}
