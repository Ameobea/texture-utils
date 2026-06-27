<script lang="ts">
  import type * as Comlink from 'comlink';
  import { transfer } from 'comlink';
  import { browser } from '$app/environment';
  import SvelteSeo from 'svelte-seo';
  import Dropzone from 'svelte-file-dropzone';
  import download from 'downloadjs';

  import type { WorkerInterface } from 'src/wasmWorker.worker';
  import { getWorkers } from 'src/workerPool';
  import { parseImageToRGBA, setPixelsToCanvas } from 'src/imageHelpers';
  import { computeHomography, outputSize, type Pt } from './homography';

  interface ImageState {
    objectURL: string;
    width: number;
    height: number;
  }

  type ProcessState =
    | { type: 'initial' }
    | { type: 'loaded'; image: ImageState }
    | { type: 'error'; message: string };

  interface Guide {
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    vertical: boolean;
  }

  let state: ProcessState = { type: 'initial' };
  let outputCanvas: HTMLCanvasElement | null = null;
  let svgEl: SVGSVGElement | null = null;

  let mode: 'corners' | 'guides' = 'corners';
  let srcQuad: Pt[] = [];
  let guides: Guide[] = [];

  let aspectMode: 'auto' | 'locked' = 'auto';
  let lockedAspect = 1;
  let outputLongEdge = 2048;
  let showGrid = false;

  let cannyLow = 50;
  let cannyHigh = 100;
  let voteThreshold = 200;
  let angleTol = 25;

  let processing = false;
  let detecting = false;
  let statusMsg = '';

  let workerP: Promise<Comlink.Remote<WorkerInterface>> = browser
    ? getWorkers().then(workers => workers.getWorker(0))
    : new Promise(() => {});

  $: maxDim = state.type === 'loaded' ? Math.max(state.image.width, state.image.height) : 1000;
  $: handleR = maxDim * 0.013;
  $: strokeW = maxDim * 0.003;
  $: outSize =
    state.type === 'loaded' && srcQuad.length === 4
      ? outputSize(srcQuad, aspectMode, lockedAspect, outputLongEdge)
      : { w: 16, h: 16 };

  const identityQuad = (w: number, h: number): Pt[] => [
    { x: 0, y: 0 },
    { x: w, y: 0 },
    { x: w, y: h },
    { x: 0, y: h },
  ];

  const handleFile = async (file: File) => {
    try {
      const { data, width, height } = await parseImageToRGBA(file);
      const objectURL = URL.createObjectURL(file);
      const worker = await workerP;
      const src = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      await worker.warpSetSource(transfer(src, [src.buffer]), width, height);

      srcQuad = identityQuad(width, height);
      guides = [];
      voteThreshold = Math.max(40, Math.round(Math.min(width, height) * 0.12));
      statusMsg = '';
      state = { type: 'loaded', image: { objectURL, width, height } };
      schedulePreview();
    } catch (err) {
      state = { type: 'error', message: err instanceof Error ? err.message : String(err) };
    }
  };

  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  const schedulePreview = () => {
    clearTimeout(previewTimer);
    previewTimer = setTimeout(applyWarp, 120);
  };

  const applyWarp = async () => {
    if (state.type !== 'loaded' || !outputCanvas || srcQuad.length !== 4) {
      return;
    }
    const { w: outW, h: outH } = outSize;
    const dstRect: Pt[] = [
      { x: 0, y: 0 },
      { x: outW, y: 0 },
      { x: outW, y: outH },
      { x: 0, y: outH },
    ];
    let hom: number[];
    try {
      hom = computeHomography(dstRect, srcQuad);
    } catch {
      return;
    }

    processing = true;
    try {
      const worker = await workerP;
      const warped = await worker.warpPerspective(new Float64Array(hom), outW, outH);
      outputCanvas.width = outW;
      outputCanvas.height = outH;
      setPixelsToCanvas(new Uint8ClampedArray(warped.buffer), outW, outH, outputCanvas);
    } finally {
      processing = false;
    }
  };

  $: if (browser && state.type === 'loaded') {
    // re-run preview when any of these change
    srcQuad;
    outputLongEdge;
    aspectMode;
    lockedAspect;
    schedulePreview();
  }

  const autoDetect = async () => {
    if (state.type !== 'loaded') {
      return;
    }
    detecting = true;
    statusMsg = '';
    try {
      const worker = await workerP;
      const res = await worker.detectPerspective(cannyLow, cannyHigh, voteThreshold, angleTol);
      if (res[0] === 1) {
        srcQuad = [
          { x: res[1], y: res[2] },
          { x: res[3], y: res[4] },
          { x: res[5], y: res[6] },
          { x: res[7], y: res[8] },
        ];
        mode = 'corners';
        statusMsg = 'Detected a correction from image lines. Fine-tune the corners as needed.';
      } else {
        statusMsg =
          'No clear perspective lines found. Try lowering the vote threshold or widening angle tolerance, or correct manually.';
      }
    } finally {
      detecting = false;
    }
  };

  const solveFromGuides = async () => {
    if (state.type !== 'loaded' || guides.length === 0) {
      return;
    }
    const vCount = guides.filter(g => g.vertical).length;
    if (vCount < 2 && guides.length - vCount < 2) {
      statusMsg = 'Add at least 2 guides of the same orientation (vertical or horizontal).';
      return;
    }
    const endpoints = new Float32Array(guides.length * 4);
    const isVertical = new Uint8Array(guides.length);
    guides.forEach((g, i) => {
      endpoints.set([g.x1, g.y1, g.x2, g.y2], i * 4);
      isVertical[i] = g.vertical ? 1 : 0;
    });
    const worker = await workerP;
    const res = await worker.solveGuides(endpoints, isVertical, state.image.width, state.image.height);
    if (res[0] === 1) {
      srcQuad = [
        { x: res[1], y: res[2] },
        { x: res[3], y: res[4] },
        { x: res[5], y: res[6] },
        { x: res[7], y: res[8] },
      ];
      statusMsg = 'Applied correction from guides.';
    } else {
      statusMsg = 'Could not solve from those guides — try repositioning them along clear features.';
    }
  };

  const addGuide = (vertical: boolean) => {
    if (state.type !== 'loaded') {
      return;
    }
    const { width: w, height: h } = state.image;
    const n = guides.filter(g => g.vertical === vertical).length;
    const off = (n - 1) * w * 0.08;
    const guide: Guide = vertical
      ? { x1: w / 2 + off, y1: h * 0.15, x2: w / 2 + off, y2: h * 0.85, vertical }
      : { x1: w * 0.15, y1: h / 2 + (n - 1) * h * 0.08, x2: w * 0.85, y2: h / 2 + (n - 1) * h * 0.08, vertical };
    guides = [...guides, guide];
  };

  const removeGuide = (i: number) => {
    guides = guides.filter((_, ix) => ix !== i);
    solveFromGuides();
  };

  type DragTarget =
    | { kind: 'corner'; i: number }
    | { kind: 'guide'; i: number; end: 1 | 2 }
    | null;
  let drag: DragTarget = null;

  const toSrc = (ev: PointerEvent): Pt => {
    const pt = svgEl!.createSVGPoint();
    pt.x = ev.clientX;
    pt.y = ev.clientY;
    const loc = pt.matrixTransform(svgEl!.getScreenCTM()!.inverse());
    return { x: loc.x, y: loc.y };
  };

  const startDrag = (target: DragTarget, ev: PointerEvent) => {
    drag = target;
    (ev.target as Element).setPointerCapture(ev.pointerId);
    ev.preventDefault();
  };

  const onPointerMove = (ev: PointerEvent) => {
    if (!drag) {
      return;
    }
    const p = toSrc(ev);
    if (drag.kind === 'corner') {
      const i = drag.i;
      srcQuad = srcQuad.map((c, ix) => (ix === i ? p : c));
    } else {
      const { i, end } = drag;
      guides = guides.map((g, ix) =>
        ix === i ? { ...g, [`x${end}`]: p.x, [`y${end}`]: p.y } : g
      );
    }
  };

  const endDrag = () => {
    if (drag && drag.kind === 'guide') {
      solveFromGuides();
    }
    drag = null;
  };

  const resetCorrection = () => {
    if (state.type !== 'loaded') {
      return;
    }
    srcQuad = identityQuad(state.image.width, state.image.height);
    guides = [];
    statusMsg = '';
  };

  const downloadOutput = () => {
    if (!outputCanvas) {
      return;
    }
    outputCanvas.toBlob(blob => void download(blob!, 'corrected.png', 'image/png'));
  };

  const startOver = () => {
    if (state.type === 'loaded') {
      URL.revokeObjectURL(state.image.objectURL);
    }
    state = { type: 'initial' };
    srcQuad = [];
    guides = [];
    mode = 'corners';
    aspectMode = 'auto';
    lockedAspect = 1;
    outputLongEdge = 2048;
    showGrid = false;
    statusMsg = '';
  };

  const gridLines = Array.from({ length: 7 }, (_, i) => ((i + 1) / 8) * 100);
</script>

<SvelteSeo
  title="Distortion Correction"
  description="Correct perspective (keystone) distortion in photos of flat surfaces to produce clean diffuse maps, with auto-detection and manual corner/guide controls."
  openGraph={{
    title: 'Distortion Correction',
    description:
      'Correct perspective distortion in photos of flat surfaces to produce clean diffuse maps for 3D rendering.',
    type: 'website',
  }}
/>

<div class="root">
  <div class="title-bar">
    <h2>Distortion Correction</h2>
    {#if state.type !== 'initial'}
      <button on:click={startOver}>Reset</button>
    {/if}
  </div>

  {#if state.type === 'error'}
    <div class="error-container">
      <p>{state.message}</p>
      <button on:click={startOver}>Start Over</button>
    </div>
  {:else if state.type === 'initial'}
    <div class="input-group">
      <h3>Source Photo</h3>
      <Dropzone
        on:drop={e => handleFile(e.detail.acceptedFiles[0])}
        accept={['image/*']}
        containerClasses="custom-dropzone"
      >
        <p>Drop a photo of a flat surface here</p>
      </Dropzone>
    </div>
  {:else}
    <div class="work-area">
      <div class="previews">
        <div class="preview-group">
          <h3>Input — {mode === 'corners' ? 'drag corners' : 'draw guides'}</h3>
          <div class="overlay-wrap">
            <img src={state.image.objectURL} alt="Source" />
            <svg
              bind:this={svgEl}
              class="overlay"
              viewBox={`0 0 ${state.image.width} ${state.image.height}`}
              preserveAspectRatio="xMidYMid meet"
              on:pointermove={onPointerMove}
              on:pointerup={endDrag}
              on:pointercancel={endDrag}
            >
              {#if mode === 'corners'}
                <polygon
                  points={srcQuad.map(p => `${p.x},${p.y}`).join(' ')}
                  fill="rgba(100, 206, 170, 0.12)"
                  stroke="#64CEAA"
                  stroke-width={strokeW}
                />
                {#each srcQuad as p, i}
                  <circle
                    cx={p.x}
                    cy={p.y}
                    r={handleR}
                    fill="#fff"
                    stroke="#64CEAA"
                    stroke-width={strokeW}
                    class="handle"
                    on:pointerdown={ev => startDrag({ kind: 'corner', i }, ev)}
                  />
                {/each}
              {:else}
                {#each guides as g, i}
                  <line
                    x1={g.x1}
                    y1={g.y1}
                    x2={g.x2}
                    y2={g.y2}
                    stroke={g.vertical ? '#ff5a5a' : '#4d9bff'}
                    stroke-width={strokeW}
                  />
                  <circle
                    cx={g.x1}
                    cy={g.y1}
                    r={handleR}
                    fill="#fff"
                    stroke={g.vertical ? '#ff5a5a' : '#4d9bff'}
                    stroke-width={strokeW}
                    class="handle"
                    on:pointerdown={ev => startDrag({ kind: 'guide', i, end: 1 }, ev)}
                  />
                  <circle
                    cx={g.x2}
                    cy={g.y2}
                    r={handleR}
                    fill="#fff"
                    stroke={g.vertical ? '#ff5a5a' : '#4d9bff'}
                    stroke-width={strokeW}
                    class="handle"
                    on:pointerdown={ev => startDrag({ kind: 'guide', i, end: 2 }, ev)}
                  />
                {/each}
              {/if}
            </svg>
          </div>
        </div>

        <div class="preview-group">
          <h3>Output {processing ? '(updating…)' : ''}</h3>
          <div class="overlay-wrap">
            <canvas bind:this={outputCanvas} width={outSize.w} height={outSize.h} />
            {#if showGrid}
              <svg class="overlay grid" viewBox="0 0 100 100" preserveAspectRatio="none">
                {#each gridLines as g}
                  <line x1={g} y1="0" x2={g} y2="100" stroke="rgba(255,90,90,0.5)" stroke-width="0.2" />
                  <line x1="0" y1={g} x2="100" y2={g} stroke="rgba(77,155,255,0.5)" stroke-width="0.2" />
                {/each}
              </svg>
            {/if}
          </div>
        </div>
      </div>

      <div class="controls">
        <div class="control-group">
          <div class="mode-toggle">
            <button class:active={mode === 'corners'} on:click={() => (mode = 'corners')}>
              Corners
            </button>
            <button class:active={mode === 'guides'} on:click={() => (mode = 'guides')}>
              Guides
            </button>
          </div>
          {#if mode === 'corners'}
            <span class="hint">Drag the 4 corners onto a region that should be a rectangle.</span>
          {:else}
            <div class="guide-buttons">
              <button on:click={() => addGuide(true)}>+ Vertical guide</button>
              <button on:click={() => addGuide(false)}>+ Horizontal guide</button>
            </div>
            <span class="hint">
              Draw lines along features that should be vertical (red) or horizontal (blue). 2+ of one
              orientation needed.
            </span>
            {#if guides.length > 0}
              <div class="guide-list">
                {#each guides as g, i}
                  <span class="guide-chip" style:border-color={g.vertical ? '#ff5a5a' : '#4d9bff'}>
                    {g.vertical ? 'V' : 'H'}{i + 1}
                    <button class="chip-x" on:click={() => removeGuide(i)}>×</button>
                  </span>
                {/each}
              </div>
            {/if}
          {/if}
        </div>

        <div class="control-group">
          <strong>Auto-detect</strong>
          <button on:click={autoDetect} disabled={detecting}>
            {detecting ? 'Detecting…' : 'Detect from image'}
          </button>
          <div class="control-row">
            <label>Vote threshold: {voteThreshold}</label>
            <input type="range" min="20" max="800" step="10" bind:value={voteThreshold} />
          </div>
          <div class="control-row">
            <label>Angle tolerance: {angleTol}°</label>
            <input type="range" min="5" max="45" step="1" bind:value={angleTol} />
          </div>
          <div class="control-row">
            <label>Edge low / high: {cannyLow} / {cannyHigh}</label>
            <input type="range" min="5" max="150" step="5" bind:value={cannyLow} />
            <input type="range" min="20" max="300" step="5" bind:value={cannyHigh} />
          </div>
        </div>

        <div class="control-group">
          <strong>Output</strong>
          <div class="control-row">
            <label>Long edge: {outputLongEdge}px</label>
            <input type="range" min="256" max="8192" step="128" bind:value={outputLongEdge} />
          </div>
          <div class="control-row">
            <label for="aspect-mode">Aspect</label>
            <select id="aspect-mode" bind:value={aspectMode}>
              <option value="auto">Auto</option>
              <option value="locked">Locked</option>
            </select>
          </div>
          {#if aspectMode === 'locked'}
            <div class="control-row">
              <label>Ratio (w/h): {lockedAspect.toFixed(2)}</label>
              <input type="range" min="0.25" max="4" step="0.05" bind:value={lockedAspect} />
            </div>
          {/if}
          <label class="checkbox-row">
            <input type="checkbox" bind:checked={showGrid} />
            Show grid on output
          </label>
        </div>

        <div class="control-group actions">
          <button on:click={resetCorrection}>Reset correction</button>
          <button on:click={downloadOutput}>Download</button>
        </div>
      </div>

      {#if statusMsg}
        <p class="status">{statusMsg}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .root {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 1rem 2rem;
    gap: 1rem;
    width: 100%;
    height: 100vh;
    box-sizing: border-box;
    overflow: hidden;
  }

  .title-bar {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 2rem;
    width: 100%;
    position: relative;
  }

  .title-bar h2 {
    margin: 0;
  }

  .title-bar button {
    position: absolute;
    right: 2rem;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  .work-area {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    width: 100%;
    flex: 1;
    min-height: 0;
  }

  .previews {
    display: flex;
    justify-content: center;
    gap: 2rem;
    flex: 1;
    min-height: 0;
    width: 100%;
  }

  .preview-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  .preview-group h3 {
    margin: 0;
    flex-shrink: 0;
  }

  .overlay-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    width: 100%;
    display: flex;
    justify-content: center;
  }

  .overlay-wrap img,
  .overlay-wrap canvas {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border: 1px solid #ccc;
  }

  .overlay {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    touch-action: none;
  }

  .overlay.grid {
    pointer-events: none;
  }

  .handle {
    cursor: grab;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: center;
    gap: 1.5rem;
    flex-shrink: 0;
  }

  .control-group {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-width: 180px;
  }

  .control-group.actions {
    justify-content: center;
  }

  .control-row {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .control-row label,
  .control-group label {
    font-size: 0.85rem;
  }

  .mode-toggle {
    display: flex;
    gap: 0;
  }

  .mode-toggle button {
    border: 1px solid #888;
    background: #2a2a2a;
    padding: 0.3rem 0.8rem;
    cursor: pointer;
  }

  .mode-toggle button.active {
    background: #64ceaa;
    color: #000;
  }

  .guide-buttons {
    display: flex;
    gap: 0.4rem;
  }

  .guide-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .guide-chip {
    border: 1px solid;
    border-radius: 0.3rem;
    padding: 0 0.2rem 0 0.4rem;
    font-size: 0.8rem;
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
  }

  .chip-x {
    border: none;
    background: none;
    cursor: pointer;
    font-size: 1rem;
    line-height: 1;
    padding: 0;
  }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    cursor: pointer;
  }

  .hint {
    font-size: 0.8rem;
    color: #888;
    max-width: 240px;
  }

  .status {
    margin: 0;
    font-size: 0.9rem;
    color: #64ceaa;
    flex-shrink: 0;
  }

  .error-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    color: red;
  }

  :global(.custom-dropzone) {
    width: 300px;
    height: 300px;
    border: 2px dashed #ccc;
    display: flex;
    justify-content: center;
    align-items: center;
    text-align: center;
  }
</style>
