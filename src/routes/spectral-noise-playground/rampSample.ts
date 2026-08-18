/**
 * Client-side ramp sampling for editor previews + field coloring. Mirrors the geoscript
 * `color_ramp` math: named easings, all four mix spaces, shorter-arc OKLCH hue.
 * Adapted from dream's src/geoscript/rampPreview.ts.
 */

export type Ease = 'linear' | 'smooth' | 'smoother' | 'step';
export type RampSpace = 'oklab' | 'oklch' | 'linear' | 'srgb';

export interface RampStop {
  pos: number;
  /** '#rrggbb', sRGB-encoded */
  hex: string;
  ease: Ease;
}

type V3 = [number, number, number];

const srgbC2Lin = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const linC2Srgb = (c: number) => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055);

export const linearToSrgb = (c: V3): V3 => [linC2Srgb(c[0]), linC2Srgb(c[1]), linC2Srgb(c[2])];
export const srgbToLinear = (c: V3): V3 => [srgbC2Lin(c[0]), srgbC2Lin(c[1]), srgbC2Lin(c[2])];

export const hexToLinear = (hex: string): V3 =>
  srgbToLinear([
    parseInt(hex.slice(1, 3), 16) / 255,
    parseInt(hex.slice(3, 5), 16) / 255,
    parseInt(hex.slice(5, 7), 16) / 255,
  ]);

export const hexToRgb8 = (hex: string): [number, number, number] => [
  parseInt(hex.slice(1, 3), 16),
  parseInt(hex.slice(3, 5), 16),
  parseInt(hex.slice(5, 7), 16),
];

export const rgb8ToHex = (rgb: [number, number, number]): string =>
  `#${rgb.map(c => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, '0')).join('')}`;

const linearToOklab = (c: V3): V3 => {
  const l = Math.cbrt(0.4122214708 * c[0] + 0.5363325363 * c[1] + 0.0514459929 * c[2]);
  const m = Math.cbrt(0.2119034982 * c[0] + 0.6806995451 * c[1] + 0.1073969566 * c[2]);
  const s = Math.cbrt(0.0883024619 * c[0] + 0.2817188376 * c[1] + 0.6299787005 * c[2]);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
};

const oklabToLinear = (c: V3): V3 => {
  const l = (c[0] + 0.3963377774 * c[1] + 0.2158037573 * c[2]) ** 3;
  const m = (c[0] - 0.1055613458 * c[1] - 0.0638541728 * c[2]) ** 3;
  const s = (c[0] - 0.0894841775 * c[1] - 1.291485548 * c[2]) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
};

const ease = (name: Ease, t: number): number => {
  switch (name) {
    case 'linear':
      return t;
    case 'smooth':
      return t * t * (3 - 2 * t);
    case 'smoother':
      return t * t * t * (t * (t * 6 - 15) + 10);
    case 'step':
      return 0;
  }
};

const toSpace = (space: RampSpace, v: V3): V3 => {
  switch (space) {
    case 'linear':
      return v;
    case 'srgb':
      return linearToSrgb(v);
    case 'oklab':
      return linearToOklab(v);
    case 'oklch': {
      const [L, a, b] = linearToOklab(v);
      return [L, Math.hypot(a, b), Math.atan2(b, a)];
    }
  }
};

const fromSpace = (space: RampSpace, v: V3): V3 => {
  const clamp01 = (c: V3): V3 => [
    Math.min(1, Math.max(0, c[0])),
    Math.min(1, Math.max(0, c[1])),
    Math.min(1, Math.max(0, c[2])),
  ];
  switch (space) {
    case 'linear':
      return v;
    case 'srgb':
      return clamp01(srgbToLinear(v));
    case 'oklab':
      return clamp01(oklabToLinear(v));
    case 'oklch':
      return clamp01(oklabToLinear([v[0], v[1] * Math.cos(v[2]), v[1] * Math.sin(v[2])]));
  }
};

const mix = (space: RampSpace, a: V3, b: V3, t: number): V3 => {
  if (space === 'oklch') {
    let [ha, hb] = [a[2], b[2]];
    if (a[1] < 1e-4) ha = hb;
    if (b[1] < 1e-4) hb = ha;
    const dh = ((((hb - ha + Math.PI) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI)) - Math.PI;
    return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, ha + dh * t];
  }
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
};

/** Sample the ramp at `x` (stop-position units, clamped to the extent) → linear RGB. */
export const sampleRamp = (stops: RampStop[], space: RampSpace, x: number): V3 => {
  if (stops.length === 0) return [0, 0, 0];
  if (stops.length === 1) return hexToLinear(stops[0].hex);
  const u = Math.min(stops[stops.length - 1].pos, Math.max(stops[0].pos, x));
  let idx = stops.length;
  for (let i = 0; i < stops.length; i += 1) {
    if (stops[i].pos > u) {
      idx = i;
      break;
    }
  }
  if (idx === 0) return hexToLinear(stops[0].hex);
  if (idx >= stops.length) return hexToLinear(stops[stops.length - 1].hex);
  const [s0, s1] = [stops[idx - 1], stops[idx]];
  const t = s1.pos > s0.pos ? (u - s0.pos) / (s1.pos - s0.pos) : 1;
  return fromSpace(
    space,
    mix(space, toSpace(space, hexToLinear(s0.hex)), toSpace(space, hexToLinear(s1.hex)), ease(s0.ease, t))
  );
};

/** sRGB LUT over the stop extent: n entries × 3 bytes. */
export const buildRampLut = (stops: RampStop[], space: RampSpace, n = 512): Uint8ClampedArray => {
  const lut = new Uint8ClampedArray(n * 3);
  const lo = stops[0]?.pos ?? 0;
  const hi = stops[stops.length - 1]?.pos ?? 1;
  for (let i = 0; i < n; i += 1) {
    const s = linearToSrgb(sampleRamp(stops, space, lo + ((hi - lo) * i) / (n - 1)));
    lut[i * 3] = Math.round(Math.min(1, Math.max(0, s[0])) * 255);
    lut[i * 3 + 1] = Math.round(Math.min(1, Math.max(0, s[1])) * 255);
    lut[i * 3 + 2] = Math.round(Math.min(1, Math.max(0, s[2])) * 255);
  }
  return lut;
};

/** Paint the full gradient (sRGB-encoded) across a canvas. */
export const drawRampBar = (canvas: HTMLCanvasElement, stops: RampStop[], space: RampSpace) => {
  const ctx2d = canvas.getContext('2d');
  if (!ctx2d || stops.length === 0) return;
  const { width: w, height: h } = canvas;
  const lut = buildRampLut(stops, space, w);
  const img = ctx2d.createImageData(w, 1);
  for (let i = 0; i < w; i += 1) {
    img.data[i * 4] = lut[i * 3];
    img.data[i * 4 + 1] = lut[i * 3 + 1];
    img.data[i * 4 + 2] = lut[i * 3 + 2];
    img.data[i * 4 + 3] = 255;
  }
  for (let y = 0; y < h; y += 1) ctx2d.putImageData(img, 0, y);
};
