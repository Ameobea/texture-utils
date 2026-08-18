<script lang="ts" context="module">
  const description =
    'Extracts a compact spectral fingerprint + color ramp from a texture photo and re-synthesizes ' +
    'a seamless texture from it. Outputs copy-pastable Geoscript code.';

  interface RampStop {
    pos: number;
    rgb: [number, number, number];
  }
</script>

<script lang="ts">
  import {
    buildFingerprintSnippet,
    buildRampSnippet,
    buildTextonSnippet,
    type NsSpectralParams as SpectralParams,
  } from 'src/nsSnippets';
  import type * as Comlink from 'comlink';
  import { browser } from '$app/environment';
  import { goto } from '$app/navigation';
  import SvelteSeo from 'svelte-seo';
  import Dropzone from 'svelte-file-dropzone';

  import type { WorkerInterface } from 'src/wasmWorker.worker';
  import { getWorkers } from 'src/workerPool';
  import { setImageDataToCanvas } from 'src/imageHelpers';
  import CropStep, { type CropRect } from './CropStep.svelte';

  type ProcessState =
    | { type: 'notStarted' }
    | { type: 'cropping' }
    | { type: 'analyzing'; raw: Uint8ClampedArray; exemplar: Uint8ClampedArray; size: number }
    | {
        type: 'ready';
        raw: Uint8ClampedArray;
        exemplar: Uint8ClampedArray;
        size: number;
        params: SpectralParams;
        ramp: RampStop[];
      }
    | { type: 'error'; message: string };

  // High-pass pre-filter: slider 0 = off; 1..100 log-maps the cutoff wavelength from
  // 512px (whole-image gradients only) down to 16px. The engine subtracts a gaussian blur
  // of the luminance; its half-power wavelength ~= 5.34 * sigma.
  const HP_LAMBDA_MAX = 512;
  const HP_LAMBDA_MIN = 16;
  const HP_SIGMA_PER_LAMBDA = 1 / 5.34;

  const SYNTH_SIZES = [256, 512, 1024, 2048];
  const MAX_TILED_BACKING = 2048;

  let state: ProcessState = { type: 'notStarted' };
  let srcBmp: ImageBitmap | null = null;
  let cropRect: CropRect | null = null;
  let exemplarSize = 256;
  let maxKernels = 4;
  let rampStops = 8;
  let highpass = 0;
  let seed = 0;
  let grayscale = false;
  let synthSize = 512;
  let synthesizing = false;

  let exemplarCanvas: HTMLCanvasElement | null = null;
  let synthCanvas: HTMLCanvasElement | null = null;
  let tiledCanvas: HTMLCanvasElement | null = null;
  let panelWidth = 0;
  let copiedIx: number | null = null;
  let renderGen = 0;

  interface TextonFit {
    kernel_u8: number[];
    ksize: number;
    scale: number;
    offset: number;
    mean_h2: number;
  }

  const TEXTON_COVERAGE = 20;
  let texton: TextonFit | null = null;
  let textonUri = '';
  let textonKsize = 32;
  let textonFitting = false;
  let textonSynthesizing = false;
  let textonGen = 0;
  let textonRenderGen = 0;
  let textonKernelCanvas: HTMLCanvasElement | null = null;
  let textonSynthCanvas: HTMLCanvasElement | null = null;

  const workerP: Promise<Comlink.Remote<WorkerInterface>> = browser
    ? getWorkers().then(workers => workers.getWorker(0))
    : new Promise(() => {});

  const extractExemplar = (bmp: ImageBitmap, rect: CropRect, size: number): Uint8ClampedArray => {
    const canvas = document.createElement('canvas');
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext('2d')!;
    ctx.imageSmoothingQuality = 'high';
    ctx.drawImage(bmp, rect.x, rect.y, rect.side, rect.side, 0, 0, size, size);
    return ctx.getImageData(0, 0, size, size).data;
  };

  const analyze = async (
    raw: Uint8ClampedArray,
    size: number,
    flattenSigma: number,
    maxKernels: number,
    rampStops: number
  ) => {
    // re-fits keep the ready UI mounted; unmounting it thrashes layout + drops input focus
    if (state.type !== 'ready') {
      state = { type: 'analyzing', raw, exemplar: raw, size };
    }
    try {
      const worker = await workerP;
      const processed = await worker.nsPreprocess(
        new Uint8Array(raw.buffer.slice(0)),
        size,
        flattenSigma
      );
      const exemplar = new Uint8ClampedArray(processed.buffer.slice(0));
      const resJson = await worker.nsAnalyze(processed, size, maxKernels, rampStops);
      const res: { params: SpectralParams; ramp: RampStop[] } = JSON.parse(resJson);
      state = { type: 'ready', raw, exemplar, size, params: res.params, ramp: res.ramp };
      if (texton) {
        void fitTexton();
      }
    } catch (err) {
      state = { type: 'error', message: `${err}` };
    }
  };

  const kernelToDataUri = (t: TextonFit): string => {
    const c = document.createElement('canvas');
    c.width = t.ksize;
    c.height = t.ksize;
    const ctx = c.getContext('2d')!;
    const img = ctx.createImageData(t.ksize, t.ksize);
    t.kernel_u8.forEach((v, i) => {
      img.data[i * 4] = v;
      img.data[i * 4 + 1] = v;
      img.data[i * 4 + 2] = v;
      img.data[i * 4 + 3] = 255;
    });
    ctx.putImageData(img, 0, 0);
    return c.toDataURL('image/png');
  };

  const fitTexton = async () => {
    if (state.type !== 'ready') {
      return;
    }
    const gen = ++textonGen;
    textonFitting = true;
    try {
      const worker = await workerP;
      const json = await worker.nsFitTexton(
        new Uint8Array(state.exemplar.buffer.slice(0)),
        state.size,
        textonKsize,
        0
      );
      if (gen !== textonGen) {
        return;
      }
      texton = JSON.parse(json);
      textonUri = kernelToDataUri(texton!);
    } finally {
      if (gen === textonGen) {
        textonFitting = false;
      }
    }
  };

  const setTextonKsize = (ks: number) => {
    textonKsize = ks;
    if (texton) {
      void fitTexton();
    }
  };

  const renderTextonPreview = async (
    t: TextonFit,
    ramp: RampStop[],
    seed: number,
    grayscale: boolean,
    size: number,
    canvas: HTMLCanvasElement
  ) => {
    const gen = ++textonRenderGen;
    textonSynthesizing = true;
    try {
      const worker = await workerP;
      const pixels = await worker.nsTextonPreview(
        new Uint8Array(t.kernel_u8),
        t.ksize,
        t.scale,
        t.offset,
        size,
        size,
        seed,
        TEXTON_COVERAGE,
        JSON.stringify(ramp),
        grayscale
      );
      if (gen !== textonRenderGen) {
        return;
      }
      canvas.width = size;
      canvas.height = size;
      setImageDataToCanvas(canvas, {
        data: new Uint8ClampedArray(pixels),
        width: size,
        height: size,
      });
    } finally {
      if (gen === textonRenderGen) {
        textonSynthesizing = false;
      }
    }
  };

  const handleFilesSelect = async (e: CustomEvent<{ acceptedFiles: File[] }>) => {
    const { acceptedFiles } = e.detail;
    if (!acceptedFiles.length) {
      return;
    }
    srcBmp?.close();
    srcBmp = await createImageBitmap(acceptedFiles[0]);
    cropRect = null;
    texton = null;
    state = { type: 'cropping' };
  };

  const onCropConfirm = (e: CustomEvent<{ rect: CropRect }>) => {
    if (!srcBmp) {
      return;
    }
    cropRect = e.detail.rect;
    const raw = extractExemplar(srcBmp, cropRect, exemplarSize);
    void analyze(raw, exemplarSize, hpSigma, maxKernels, rampStops);
  };

  const startOver = () => {
    srcBmp?.close();
    srcBmp = null;
    cropRect = null;
    texton = null;
    state = { type: 'notStarted' };
  };

  const reAnalyze = () => {
    if (state.type === 'ready') {
      void analyze(state.raw, state.size, hpSigma, maxKernels, rampStops);
    }
  };

  const openInPlayground = () => {
    if (state.type !== 'ready') {
      return;
    }
    sessionStorage.setItem(
      'nsPlaygroundImport',
      JSON.stringify({ params: state.params, ramp: state.ramp, seed })
    );
    void goto('/spectral-noise-playground');
  };

  let hpTimer: ReturnType<typeof setTimeout> | null = null;
  const reAnalyzeDebounced = () => {
    if (hpTimer) {
      clearTimeout(hpTimer);
    }
    hpTimer = setTimeout(reAnalyze, 200);
  };

  $: hpLambda =
    highpass === 0
      ? 0
      : Math.round(HP_LAMBDA_MAX * Math.pow(HP_LAMBDA_MIN / HP_LAMBDA_MAX, (highpass - 1) / 99));
  $: hpSigma = hpLambda * HP_SIGMA_PER_LAMBDA;

  $: tiledBacking = Math.min(synthSize * 2, MAX_TILED_BACKING);

  const renderPreview = async (
    params: SpectralParams,
    ramp: RampStop[],
    seed: number,
    grayscale: boolean,
    size: number,
    synthCanvas: HTMLCanvasElement,
    tiledCanvas: HTMLCanvasElement
  ) => {
    const gen = ++renderGen;
    synthesizing = true;
    try {
      const worker = await workerP;
      const pixels = await worker.nsPreview(
        JSON.stringify(params),
        JSON.stringify(ramp),
        size,
        size,
        seed,
        grayscale
      );
      if (gen !== renderGen) {
        return;
      }
      synthCanvas.width = size;
      synthCanvas.height = size;
      setImageDataToCanvas(synthCanvas, {
        data: new Uint8ClampedArray(pixels),
        width: size,
        height: size,
      });
      const backing = Math.min(size * 2, MAX_TILED_BACKING);
      const half = backing / 2;
      tiledCanvas.width = backing;
      tiledCanvas.height = backing;
      const ctx = tiledCanvas.getContext('2d')!;
      ctx.imageSmoothingEnabled = half < size;
      for (const [dx, dy] of [
        [0, 0],
        [half, 0],
        [0, half],
        [half, half],
      ]) {
        ctx.drawImage(synthCanvas, dx, dy, half, half);
      }
    } finally {
      if (gen === renderGen) {
        synthesizing = false;
      }
    }
  };

  $: if (state.type === 'ready' && synthCanvas && tiledCanvas) {
    void renderPreview(
      state.params,
      state.ramp,
      seed,
      grayscale,
      synthSize,
      synthCanvas,
      tiledCanvas
    );
  }

  $: if ((state.type === 'analyzing' || state.type === 'ready') && exemplarCanvas) {
    exemplarCanvas.width = state.size;
    exemplarCanvas.height = state.size;
    setImageDataToCanvas(exemplarCanvas, {
      data: state.exemplar,
      width: state.size,
      height: state.size,
    });
  }

  $: fingerprintSnippet = state.type === 'ready' ? buildFingerprintSnippet(state.params, seed) : '';
  $: rampSnippet = state.type === 'ready' ? buildRampSnippet(state.ramp) : '';
  $: textonSnippet =
    texton && textonUri
      ? buildTextonSnippet(textonUri, texton.ksize, texton.scale, texton.offset, texton.mean_h2, seed)
      : '';

  $: if (texton && textonKernelCanvas) {
    textonKernelCanvas.width = texton.ksize;
    textonKernelCanvas.height = texton.ksize;
    setImageDataToCanvas(textonKernelCanvas, {
      data: new Uint8ClampedArray(texton.kernel_u8.flatMap(v => [v, v, v, 255])),
      width: texton.ksize,
      height: texton.ksize,
    });
  }

  $: if (texton && state.type === 'ready' && textonSynthCanvas) {
    void renderTextonPreview(texton, state.ramp, seed, grayscale, synthSize, textonSynthCanvas);
  }

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
  title="Noise Signature Extractor"
  {description}
  openGraph={{ title: 'Noise Signature Extractor', description, type: 'website', images: [] }}
/>

<div class="root">
  {#if state.type === 'notStarted'}
    <div class="dropzone-container">
      <h2>Noise Signature Extractor</h2>
      <p class="blurb">
        Fits a compact spectral fingerprint (~32 bytes) + color ramp to a texture photo and
        re-synthesizes a seamless texture from just those parameters. Both outputs are
        copy-pastable Geoscript code for use with the <code>spectral_noise</code> builtin. Works
        best on stochastic materials: concrete, stucco, sand, soil, marble, fabric, wood grain.
        Structured layouts (bricks, cracks, distinct chips) won't survive — by design.
      </p>
      <Dropzone on:drop={handleFilesSelect} accept={['image/*']} containerClasses="custom-dropzone">
        <p>Drag + drop a texture photo here to fingerprint it.</p>
        <p class="hint">You'll pick a square crop region + analysis resolution next.</p>
      </Dropzone>
    </div>
  {:else if state.type === 'cropping'}
    {#if srcBmp}
      <CropStep
        bmp={srcBmp}
        bind:exemplarSize
        initialRect={cropRect}
        on:confirm={onCropConfirm}
        on:cancel={startOver}
      />
    {/if}
  {:else if state.type === 'error'}
    <div class="error">
      {state.message}
      <button on:click={startOver}>Start over</button>
    </div>
  {:else}
    <div class="content">
      <div class="previews">
        <div class="preview" bind:clientWidth={panelWidth}>
          <canvas bind:this={exemplarCanvas} class:pixelated={panelWidth > state.size} />
          <span>exemplar (cropped, {state.size}²)</span>
        </div>
        <div class="preview">
          <canvas bind:this={synthCanvas} class:pixelated={panelWidth > synthSize} />
          <span>synthesized {synthSize}²{synthesizing ? ' — synthesizing…' : ''}</span>
        </div>
        <div class="preview">
          <canvas bind:this={tiledCanvas} class:pixelated={panelWidth > tiledBacking} />
          <span>synthesized, tiled 2×2</span>
        </div>
      </div>

      {#if state.type === 'analyzing'}
        <p>Analyzing…</p>
      {:else}
        <div class="controls">
          <label>
            max spectral peaks:
            <input type="number" min="0" max="4" bind:value={maxKernels} on:change={reAnalyze} />
            <span class="found">(fit found {state.params.kernels.length})</span>
          </label>
          <label>
            ramp stops:
            <input
              type="number"
              min="2"
              max="16"
              bind:value={rampStops}
              on:change={reAnalyze}
            />
          </label>
          <label>
            seed:
            <input type="number" min="0" bind:value={seed} />
            <button on:click={() => (seed = Math.floor(Math.random() * 1000))}>shuffle</button>
          </label>
          <label class="hp" title="Removes low-frequency shading (lighting gradients, vignettes, partial shadows) before fitting by subtracting a gaussian blur of the luminance — a high-pass filter with the shown half-power cutoff wavelength">
            high-pass filter:
            <input type="range" min="0" max="100" bind:value={highpass} on:input={reAnalyzeDebounced} />
            <span class="hpval">{hpLambda === 0 ? 'off' : `cutoff ≈ ${hpLambda}px`}</span>
          </label>
          <span class="sizebtns">
            preview size:
            {#each SYNTH_SIZES as s}
              <button class:on={synthSize === s} on:click={() => (synthSize = s)}>
                {s >= 1024 ? `${s / 1024}k` : s}
              </button>
            {/each}
          </span>
          <label>
            <input type="checkbox" bind:checked={grayscale} />
            grayscale field
          </label>
          <button
            title="Go back to the crop step for this image (keeps the current crop as a starting point)"
            on:click={() => (state = { type: 'cropping' })}
          >
            re-crop
          </button>
          <button on:click={startOver}>new image</button>
          <button
            class="playground-btn"
            title="Load the fitted bands/peaks/ramp into the Spectral Noise Playground for hand-tweaking. One-way: this page's state is discarded."
            on:click={openInPlayground}
          >
            open in playground →
          </button>
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

        <div class="texton">
          <div class="texton-head">
            <h3
              title="Fits a small kernel whose dense ±1 scatter reproduces the exemplar's full spectrum — holds narrow spectral lines and oriented broadband the 32-float fingerprint can't. Exported as an embedded grayscale PNG consumed via load_image + scatter."
            >
              texton scatter (higher-fidelity tier) ⓘ
            </h3>
            <label>
              kernel size:
              <select
                value={textonKsize}
                on:change={e => setTextonKsize(parseInt(e.currentTarget.value))}
              >
                {#each [16, 32, 64, 128] as ks}
                  <option value={ks}>{ks}²</option>
                {/each}
              </select>
            </label>
            <button on:click={fitTexton} disabled={textonFitting}>
              {textonFitting ? 'fitting…' : texton ? 'refit' : 'fit texton'}
            </button>
          </div>
          {#if texton}
            <div class="texton-body">
              <div class="preview kernel-preview">
                <canvas bind:this={textonKernelCanvas} class="pixelated" />
                <span>kernel ({texton.ksize}², u8)</span>
              </div>
              <div class="preview">
                <canvas
                  bind:this={textonSynthCanvas}
                  class:pixelated={panelWidth > synthSize}
                />
                <span>texton synth {synthSize}²{textonSynthesizing ? ' — synthesizing…' : ''}</span>
              </div>
              <div class="snippet">
                <div class="snippet-head">
                  <h3>texton → <code>load_image</code> + <code>scatter</code></h3>
                  <button on:click={() => copySnippet(textonSnippet, 2)}>
                    {copiedIx === 2 ? 'copied!' : 'copy'}
                  </button>
                </div>
                <textarea readonly rows={16} value={textonSnippet} />
                <span class="hint">
                  color it with the same ramp: <code>ramp(field)</code>
                </span>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style lang="css">
  :global(html) {
    scrollbar-gutter: stable;
  }

  .found {
    color: #999;
  }

  .root {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 20px 12px 60px;
  }

  .dropzone-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    max-width: 720px;
    text-align: center;
  }

  .blurb {
    text-align: left;
  }

  .hint {
    font-size: 12px;
    color: #999;
  }

  .content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    width: 100%;
  }

  .previews {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 12px;
    width: 100%;
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

  .sizebtns button {
    margin-left: 2px;
  }

  .hp input[type='range'] {
    width: 130px;
    vertical-align: middle;
  }

  .hpval {
    display: inline-block;
    min-width: 92px;
    color: #999;
  }

  .sizebtns button.on {
    background: #4a6;
    color: #111;
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

  .snippets {
    display: flex;
    gap: 16px;
    width: 100%;
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

  .error {
    color: #e88;
  }

  .playground-btn {
    background: #46a;
    color: #eee;
  }

  .texton {
    width: 100%;
    border: 1px solid #2e2e2e;
    padding: 8px 12px 12px;
    box-sizing: border-box;
  }

  .texton-head {
    display: flex;
    align-items: center;
    gap: 16px;
    font-size: 13px;
  }

  .texton-head h3 {
    font-size: 14px;
    margin: 4px 0;
    cursor: help;
  }

  .texton-body {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    align-items: flex-start;
    margin-top: 8px;
  }

  .texton-body .preview {
    flex: 1;
    min-width: 220px;
    max-width: 420px;
  }

  .kernel-preview {
    max-width: 180px !important;
    min-width: 120px !important;
  }

  .texton-body .snippet {
    flex: 2;
  }

  .texton .hint {
    margin-top: 2px;
  }
</style>
