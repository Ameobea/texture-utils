export interface NsKernel {
  f0: [number, number];
  sig: [number, number];
  angle: number;
  energy: number;
}

export interface NsSpectralParams {
  bands: number[][];
  kernels: NsKernel[];
}

export interface NsRampStop {
  pos: number;
  rgb: [number, number, number];
  ease?: 'linear' | 'smooth' | 'smoother' | 'step';
}

const f3 = (v: number) => v.toFixed(3);

export const buildFingerprintSnippet = (params: NsSpectralParams, seed: number): string => {
  const bands = params.bands.map(row => `    [${row.map(f3).join(', ')}]`).join(',\n');
  const lines = [`spectral_noise(`, `  bands=[\n${bands}\n  ],`];
  if (params.kernels.length > 0) {
    const kernels = params.kernels
      .map(
        k =>
          `    [${k.f0[0].toFixed(6)}, ${k.f0[1].toFixed(6)}, ${f3(k.sig[0])}, ${f3(k.sig[1])}, ${f3(k.angle)}, ${f3(k.energy)}]`
      )
      .join(',\n');
    lines.push(`  kernels=[\n${kernels}\n  ],`);
  }
  lines.push(`  seed=${seed}`, `)`);
  return lines.join('\n');
};

export const buildTextonSnippet = (
  uri: string,
  ksize: number,
  scale: number,
  offset: number,
  meanH2: number,
  seed: number
): string =>
  `set_rng_seed(${seed})
n = 1024
s = ${ksize}.
cov = 20.
kern = load_image(
  "${uri}",
  srgb=false, scale=${scale.toFixed(6)}, offset=${offset.toFixed(6)}
)
count = int(cov * (n * n) / (s * s))
place = || (floor(randf() * n) + s * 0.5) / n
field = scatter(
  count,
  |ix| kern * (floor(randf() * 2.) * 2. - 1.) | scale(s / n) | trans_global(place(), place()),
  texture(n, n, |uv| 0.),
  blend="add",
  filter="nearest"
) * (1. / sqrt(cov * ${meanH2.toFixed(6)}))`;

export const buildRampSnippet = (stops: NsRampStop[], space = 'oklab'): string => {
  const hex = (rgb: [number, number, number]) =>
    rgb.map(c => c.toString(16).padStart(2, '0')).join('');
  const pos = (p: number) => `${parseFloat(p.toFixed(3))}`;
  const anyEase = stops.some(s => s.ease && s.ease !== 'linear');
  const stopStrs = stops
    .map(s =>
      anyEase
        ? `  [${pos(s.pos)}, srgb(0x${hex(s.rgb)}), "${s.ease ?? 'linear'}"]`
        : `  [${pos(s.pos)}, srgb(0x${hex(s.rgb)})]`
    )
    .join(',\n');
  const spaceKwarg = space === 'oklab' ? '' : `, space="${space}"`;
  return `color_ramp(stops=[\n${stopStrs}\n]${spaceKwarg})`;
};
