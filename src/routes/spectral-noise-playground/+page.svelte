<script lang="ts" context="module">
  const description =
    'Interactive playground for the spectral_noise texture synthesizer: hand-edit the ' +
    'log-polar band spectrum, spectral peaks, and color ramp with live feedback.';
</script>

<script lang="ts">
  import type * as Comlink from 'comlink';
  import { browser } from '$app/environment';
  import { onMount } from 'svelte';
  import SvelteSeo from 'svelte-seo';

  import type { WorkerInterface } from 'src/wasmWorker.worker';
  import { getWorkers } from 'src/workerPool';
  import { buildFingerprintSnippet, buildRampSnippet, type NsSpectralParams } from 'src/nsSnippets';
  import BandsGrid, { KA, KR } from './BandsGrid.svelte';
  import KernelList, { kernelF0, type KernelUI } from './KernelList.svelte';
  import ZRampEditor from './ZRampEditor.svelte';
  import { buildRampLut, hexToRgb8, rgb8ToHex, type RampSpace, type RampStop } from './rampSample';

  const SYNTH_SIZES = [256, 512, 1024];
  const MAX_TILED_BACKING = 2048;
  const SPEC_N = 160;

  let bands: number[][] = Array.from({ length: KR }, (_, i) =>
    Array.from({ length: KA }, () => -0.693 * i)
  );
  let isotropic = true;
  let kernels: KernelUI[] = [];
  let stops: RampStop[] = [
    { pos: -2.5, hex: '#3a3632', ease: 'linear' },
    { pos: 2.5, hex: '#d8d2c8', ease: 'linear' },
  ];
  let space: RampSpace = 'oklab';
  let seed = 0;
  let size = 256;
  let grayscale = false;

  let synthesizing = false;
  let rev = 0;
  let copiedIx: number | null = null;

  let synthCanvas: HTMLCanvasElement | null = null;
  let tiledCanvas: HTMLCanvasElement | null = null;
  let specCanvas: HTMLCanvasElement | null = null;
  let panelWidth = 0;

  const workerP: Promise<Comlink.Remote<WorkerInterface>> = browser
    ? getWorkers().then(workers => workers.getWorker(0))
    : new Promise(() => {});

  const r3 = (v: number) => Math.round(v * 1000) / 1000;
  const r6 = (v: number) => Math.round(v * 1e6) / 1e6;

  // Snippet, engine preview, and spectrum viz all consume the same rounded params, so
  // what you see is exactly what a paste into geoscript produces.
  const currentParams = (): NsSpectralParams => ({
    bands: bands.map(row => row.map(r3)),
    kernels: kernels.map(k => {
      const [f0y, f0x] = kernelF0(k);
      return {
        f0: [r6(f0y), r6(f0x)] as [number, number],
        sig: [r3(Math.min(k.sig1, -0.5)), r3(Math.min(k.sig2, -0.5))] as [number, number],
        angle: r3((k.angleDeg * Math.PI) / 180),
        energy: r3(k.energy),
      };
    }),
  });

  // ---------- field synthesis (debounced, in the wasm worker) ----------

  let field: Float32Array | null = null;
  let fieldSize = 0;
  let fieldGen = 0;

  const syncField = async () => {
    const gen = ++fieldGen;
    synthesizing = true;
    try {
      const worker = await workerP;
      const f = await worker.nsField(JSON.stringify(currentParams()), size, size, seed);
      if (gen !== fieldGen) return;
      field = f;
      fieldSize = size;
      recolor();
    } finally {
      if (gen === fieldGen) {
        synthesizing = false;
      }
    }
  };

  let fieldTimer: ReturnType<typeof setTimeout> | null = null;
  const scheduleField = () => {
    if (fieldTimer) {
      clearTimeout(fieldTimer);
    }
    fieldTimer = setTimeout(syncField, 150);
  };

  // ---------- coloring (instant, client-side over the cached field) ----------

  const erf = (x: number): number => {
    const sign = x < 0 ? -1 : 1;
    x = Math.abs(x);
    const t = 1 / (1 + 0.3275911 * x);
    const y =
      1 -
      ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) *
        t *
        Math.exp(-x * x);
    return sign * y;
  };

  const recolor = () => {
    if (!field || !synthCanvas || !tiledCanvas) return;
    const n = fieldSize;
    const img = new ImageData(n, n);
    if (grayscale) {
      for (let i = 0; i < n * n; i += 1) {
        const v = Math.round(0.5 * (1 + erf(field[i] / Math.SQRT2)) * 255);
        img.data[i * 4] = v;
        img.data[i * 4 + 1] = v;
        img.data[i * 4 + 2] = v;
        img.data[i * 4 + 3] = 255;
      }
    } else {
      const LUT_N = 512;
      const lut = buildRampLut(stops, space, LUT_N);
      const lo = stops[0].pos;
      const hi = stops[stops.length - 1].pos;
      const scale = hi > lo ? (LUT_N - 1) / (hi - lo) : 0;
      for (let i = 0; i < n * n; i += 1) {
        const t = Math.min(LUT_N - 1, Math.max(0, Math.round((field[i] - lo) * scale)));
        img.data[i * 4] = lut[t * 3];
        img.data[i * 4 + 1] = lut[t * 3 + 1];
        img.data[i * 4 + 2] = lut[t * 3 + 2];
        img.data[i * 4 + 3] = 255;
      }
    }
    synthCanvas.width = n;
    synthCanvas.height = n;
    synthCanvas.getContext('2d')!.putImageData(img, 0, 0);

    const backing = Math.min(n * 2, MAX_TILED_BACKING);
    const half = backing / 2;
    tiledCanvas.width = backing;
    tiledCanvas.height = backing;
    const ctx = tiledCanvas.getContext('2d')!;
    ctx.imageSmoothingEnabled = half < n;
    for (const [dx, dy] of [
      [0, 0],
      [half, 0],
      [0, half],
      [half, half],
    ]) {
      ctx.drawImage(synthCanvas, dx, dy, half, half);
    }
  };

  // ---------- model spectrum visualization ----------

  const bandCenters = (() => {
    const c0 = Math.log(1 / 256);
    const step = (Math.log(0.5) - c0) / (KR - 1);
    return { c0, step, last: c0 + step * (KR - 1) };
  })();

  const evalBandsAt = (b: number[][], lnr: number, th: number): number => {
    const { c0, step, last } = bandCenters;
    lnr = Math.min(last, Math.max(c0, lnr));
    const ri = Math.min(KR - 2, Math.max(0, Math.floor((lnr - c0) / step)));
    const t = Math.min(1, Math.max(0, (lnr - (c0 + step * ri)) / step));
    const ap = (th / Math.PI) * KA - 0.5;
    const a0 = ((Math.floor(ap) % KA) + KA) % KA;
    const a1 = (a0 + 1) % KA;
    const at = ap - Math.floor(ap);
    return Math.exp(
      b[ri][a0] * (1 - t) * (1 - at) +
        b[ri + 1][a0] * t * (1 - at) +
        b[ri][a1] * (1 - t) * at +
        b[ri + 1][a1] * t * at
    );
  };

  const drawSpectrum = () => {
    if (!specCanvas) return;
    const n = SPEC_N;
    const params = currentParams();
    const s = new Float64Array(n * n);
    let eResid = 0;
    const fAt = (p: number) => (p - n / 2) / n;
    for (let y = 0; y < n; y += 1) {
      const fy = fAt(y);
      for (let x = 0; x < n; x += 1) {
        const fx = fAt(x);
        if (fx === 0 && fy === 0) continue;
        const r = Math.hypot(fx, fy);
        const th = (((Math.atan2(fy, fx) % Math.PI) + Math.PI) % Math.PI);
        const v = evalBandsAt(params.bands, Math.log(Math.max(r, 1e-9)), th);
        s[y * n + x] = v;
        eResid += v;
      }
    }
    for (const k of params.kernels) {
      const s1 = Math.pow(10, k.sig[0]);
      const s2 = Math.pow(10, k.sig[1]);
      const ca = Math.cos(k.angle);
      const sa = Math.sin(k.angle);
      const i11 = (ca * ca) / (s1 * s1) + (sa * sa) / (s2 * s2);
      const i22 = (sa * sa) / (s1 * s1) + (ca * ca) / (s2 * s2);
      const i12 = ca * sa * (1 / (s1 * s1) - 1 / (s2 * s2));
      const lobe = new Float64Array(n * n);
      let lsum = 0;
      for (let y = 0; y < n; y += 1) {
        const fy = fAt(y);
        for (let x = 0; x < n; x += 1) {
          const fx = fAt(x);
          if (fx === 0 && fy === 0) continue;
          let v = 0;
          for (const sgn of [1, -1]) {
            const dy = ((((fy - sgn * k.f0[0] + 0.5) % 1) + 1) % 1) - 0.5;
            const dx = ((((fx - sgn * k.f0[1] + 0.5) % 1) + 1) % 1) - 0.5;
            const q = i11 * dy * dy + 2 * i12 * dy * dx + i22 * dx * dx;
            if (q < 40) v += Math.exp(-0.5 * q);
          }
          lobe[y * n + x] = v;
          lsum += v;
        }
      }
      if (lsum > 0) {
        const scale = (Math.pow(10, k.energy) * eResid) / lsum;
        for (let i = 0; i < n * n; i += 1) s[i] += lobe[i] * scale;
      }
    }
    let lnMax = -Infinity;
    for (let i = 0; i < n * n; i += 1) {
      if (s[i] > 0) lnMax = Math.max(lnMax, Math.log(s[i]));
    }
    const RANGE = 16;
    const img = new ImageData(n, n);
    for (let i = 0; i < n * n; i += 1) {
      const b =
        s[i] > 0 ? Math.min(1, Math.max(0, (Math.log(s[i]) - lnMax) / RANGE + 1)) : 0;
      const v = Math.round(Math.pow(b, 1.5) * 255);
      img.data[i * 4] = v;
      img.data[i * 4 + 1] = v;
      img.data[i * 4 + 2] = v;
      img.data[i * 4 + 3] = 255;
    }
    specCanvas.width = n;
    specCanvas.height = n;
    specCanvas.getContext('2d')!.putImageData(img, 0, 0);
  };

  // ---------- change plumbing ----------

  const onParamsChange = () => {
    rev += 1;
    drawSpectrum();
    scheduleField();
  };

  const onRampChange = () => {
    rev += 1;
    recolor();
  };

  const setSize = (s: number) => {
    size = s;
    scheduleField();
  };

  const setSeed = (s: number) => {
    seed = s;
    scheduleField();
  };

  let fingerprintSnippet = '';
  let rampSnippet = '';
  $: {
    rev;
    fingerprintSnippet = buildFingerprintSnippet(currentParams(), seed);
    rampSnippet = buildRampSnippet(
      stops.map(s => ({ pos: s.pos, rgb: hexToRgb8(s.hex), ease: s.ease })),
      space
    );
  }
  $: tiledBacking = Math.min(fieldSize * 2, MAX_TILED_BACKING) || size;
  $: grayscale, recolor();

  // One-way handoff from the noise-signature extractor (sessionStorage; consumed on load)
  const applyImport = () => {
    const raw = sessionStorage.getItem('nsPlaygroundImport');
    if (!raw) {
      return;
    }
    sessionStorage.removeItem('nsPlaygroundImport');
    try {
      const imp: {
        params: {
          bands: number[][];
          kernels: { f0: [number, number]; sig: [number, number]; angle: number; energy: number }[];
        };
        ramp: { pos: number; rgb: [number, number, number] }[];
        seed: number;
      } = JSON.parse(raw);
      bands = imp.params.bands.map(row => row.map(v => Math.min(0, Math.max(-14, v))));
      isotropic = bands.every(row => row.every(v => Math.abs(v - row[0]) < 1e-6));
      kernels = imp.params.kernels.map(k => ({
        freq: Math.min(0.5, Math.max(1 / 256, Math.hypot(k.f0[0], k.f0[1]))),
        orientDeg: ((((Math.atan2(k.f0[0], k.f0[1]) * 180) / Math.PI) % 180) + 180) % 180,
        sig1: Math.min(-0.5, Math.max(-3, k.sig[0])),
        sig2: Math.min(-0.5, Math.max(-3, k.sig[1])),
        angleDeg: Math.min(180, Math.max(0, (k.angle * 180) / Math.PI)),
        energy: Math.min(2, Math.max(-4, k.energy)),
      }));
      stops = imp.ramp.map(s => ({ pos: s.pos, hex: rgb8ToHex(s.rgb), ease: 'linear' as const }));
      if (typeof imp.seed === 'number') {
        seed = imp.seed;
      }
    } catch (err) {
      console.error('bad playground import payload', err);
    }
  };

  onMount(() => {
    applyImport();
    drawSpectrum();
    void syncField();
  });

  const copySnippet = (text: string, ix: number) => {
    navigator.clipboard.writeText(text);
    copiedIx = ix;
    setTimeout(() => {
      if (copiedIx === ix) {
        copiedIx = null;
      }
    }, 1500);
  };
</script>

<SvelteSeo
  title="Spectral Noise Playground"
  {description}
  openGraph={{ title: 'Spectral Noise Playground', description, type: 'website', images: [] }}
/>

<div class="root">
  <p class="blurb">
    Hand-drive the <code>spectral_noise</code> synthesizer: the texture is random noise shaped to
    exactly the spectrum you set below. Hover any label for an explanation. Both outputs are
    copy-pastable Geoscript.
  </p>

  <div class="previews">
    <div class="preview" bind:clientWidth={panelWidth}>
      <canvas bind:this={synthCanvas} class:pixelated={panelWidth > fieldSize} />
      <span>synthesized {fieldSize || size}²{synthesizing ? ' — synthesizing…' : ''}</span>
    </div>
    <div class="preview">
      <canvas bind:this={tiledCanvas} class:pixelated={panelWidth > tiledBacking} />
      <span>tiled 2×2 (always seamless)</span>
    </div>
    <div class="preview">
      <canvas bind:this={specCanvas} class="spec" />
      <span
        title="Log power of the model spectrum, DC (mean) at center. Distance from center = frequency (fine detail toward the edge); direction = orientation of variation; brightness = power. The bands make the smooth backdrop; peaks show as bright dots/streaks (always in symmetric ± pairs)."
      >
        model spectrum (log, DC centered) ⓘ
      </span>
    </div>
  </div>

  <div class="controls">
    <label title="Noise instance: same spectrum + same seed reproduces the exact same texture. Different seeds are independent instances with identical statistics.">
      seed:
      <input type="number" min="0" value={seed} on:change={e => setSeed(parseInt(e.currentTarget.value) || 0)} />
      <button on:click={() => setSeed(Math.floor(Math.random() * 1000))}>shuffle</button>
    </label>
    <span class="sizebtns">
      preview size:
      {#each SYNTH_SIZES as s}
        <button class:on={size === s} on:click={() => setSize(s)}>
          {s >= 1024 ? `${s / 1024}k` : s}
        </button>
      {/each}
    </span>
    <label title="Show the raw field mapped through the Gaussian CDF (mid-gray = mean) instead of the color ramp.">
      <input type="checkbox" bind:checked={grayscale} />
      grayscale field
    </label>
  </div>

  <div class="editors">
    <div class="col">
      <BandsGrid bind:bands bind:isotropic on:change={onParamsChange} />
      <ZRampEditor bind:stops bind:space on:change={onRampChange} />
    </div>
    <div class="col kernels-col">
      <KernelList bind:kernels on:change={onParamsChange} />
    </div>
  </div>

  <div class="snippets">
    <div class="snippet">
      <div class="snippet-head">
        <h3>fingerprint → <code>spectral_noise</code></h3>
        <button on:click={() => copySnippet(fingerprintSnippet, 0)}>
          {copiedIx === 0 ? 'copied!' : 'copy'}
        </button>
      </div>
      <textarea readonly rows={14} value={fingerprintSnippet} />
    </div>
    <div class="snippet">
      <div class="snippet-head">
        <h3>color ramp → <code>color_ramp</code></h3>
        <button on:click={() => copySnippet(rampSnippet, 1)}>
          {copiedIx === 1 ? 'copied!' : 'copy'}
        </button>
      </div>
      <textarea readonly rows={14} value={rampSnippet} />
    </div>
  </div>
</div>

<style lang="css">
  :global(html) {
    scrollbar-gutter: stable;
  }

  .root {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 16px 12px 60px;
  }

  .blurb {
    max-width: 860px;
    margin: 0;
    font-size: 13px;
    color: #bbb;
  }

  .previews {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
    width: 100%;
    max-width: 1400px;
  }

  .preview {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: #aaa;
    min-width: 0;
  }

  .preview canvas {
    width: 100%;
    height: auto;
    aspect-ratio: 1;
    border: 1px solid #333;
  }

  .preview canvas.pixelated {
    image-rendering: pixelated;
  }

  .preview span[title] {
    cursor: help;
  }

  .controls {
    display: flex;
    gap: 18px;
    flex-wrap: wrap;
    align-items: center;
    font-size: 13px;
  }

  .controls input[type='number'] {
    width: 56px;
  }

  .sizebtns button {
    margin-left: 2px;
  }

  .sizebtns button.on {
    background: #4a6;
    color: #111;
  }

  .editors {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: center;
    width: 100%;
    max-width: 1400px;
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .kernels-col {
    min-width: 420px;
    flex: 1;
    max-width: 560px;
  }

  .snippets {
    display: flex;
    gap: 16px;
    width: 100%;
    max-width: 1400px;
    flex-wrap: wrap;
  }

  .snippet {
    flex: 1;
    min-width: 320px;
    display: flex;
    flex-direction: column;
  }

  .snippet-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .snippet-head h3 {
    font-size: 14px;
    margin: 4px 0;
  }

  .snippet textarea {
    font-family: monospace;
    font-size: 12px;
    background: #1a1a1a;
    color: #ddd;
    border: 1px solid #333;
    white-space: pre;
  }
</style>
