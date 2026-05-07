<script lang="ts">
  import type * as Comlink from 'comlink';
  import { browser } from '$app/environment';
  import { tick } from 'svelte';
  import SvelteSeo from 'svelte-seo';
  import Dropzone from 'svelte-file-dropzone';
  import download from 'downloadjs';

  import type { WorkerInterface } from 'src/wasmWorker.worker';
  import { getWorkers } from 'src/workerPool';
  import { parseImageToRGBA, setPixelsToCanvas } from 'src/imageHelpers';

  interface ImageState {
    data: Uint8ClampedArray;
    dataURL: string;
    width: number;
    height: number;
  }

  type ProcessState =
    | { type: 'initial' }
    | { type: 'loaded'; image: ImageState }
    | { type: 'error'; message: string };

  let state: ProcessState = { type: 'initial' };
  let outputCanvas: HTMLCanvasElement | null = null;

  let marginX = 0.2;
  let marginY = 0.2;
  let linkMargins = false;
  let contrastCorrectionFactor = 0.7;
  let processing = false;

  const clampMargin = (v: number) => {
    if (Number.isNaN(v)) return 0;
    return Math.min(0.5, Math.max(0, v));
  };

  // Setters used by both the slider and the number input.  When the link
  // checkbox is on, mutating one axis snaps the other to match.
  const setMarginX = (v: number) => {
    const c = clampMargin(v);
    marginX = c;
    if (linkMargins) marginY = c;
  };
  const setMarginY = (v: number) => {
    const c = clampMargin(v);
    marginY = c;
    if (linkMargins) marginX = c;
  };

  // When the user *enables* linking, snap Y to X so the two values match
  // immediately rather than only after the next slider drag.
  $: if (linkMargins && marginX !== marginY) {
    marginY = marginX;
  }

  const scratchCanvas = browser ? document.createElement('canvas') : null;
  const rgbaToDataURL = (rgba: Uint8ClampedArray, width: number, height: number) => {
    if (!scratchCanvas) {
      throw new Error('Cannot convert RGBA to data URL without a browser');
    }
    scratchCanvas.width = width;
    scratchCanvas.height = height;
    const ctx = scratchCanvas.getContext('2d')!;
    const imageData = ctx.createImageData(width, height);
    imageData.data.set(rgba);
    ctx.putImageData(imageData, 0, 0);
    return scratchCanvas.toDataURL();
  };

  let workerP: Promise<Comlink.Remote<WorkerInterface>> = browser
    ? getWorkers().then(workers => workers.getWorker(0))
    : new Promise(() => {});

  const handleFile = async (file: File) => {
    const { data, width, height } = await parseImageToRGBA(file);
    const dataURL = rgbaToDataURL(data, width, height);
    state = { type: 'loaded', image: { data, width, height, dataURL } };
    // Wait for the canvas to be inserted into the DOM, then run the first
    // pass so the user sees a result immediately.
    await tick();
    applyFilter();
  };

  let outputDataURL: string | null = null;
  let genState: {
    isGenerating: boolean;
    nextParams: { marginX: number; marginY: number; contrast: number } | null;
    lastParams: { marginX: number; marginY: number; contrast: number } | null;
  } = { isGenerating: false, nextParams: null, lastParams: null };

  const applyFilter = async () => {
    if (state.type !== 'loaded' || !outputCanvas) {
      return;
    }

    const myParams = { marginX, marginY, contrast: contrastCorrectionFactor };
    if (
      genState.lastParams &&
      genState.lastParams.marginX === myParams.marginX &&
      genState.lastParams.marginY === myParams.marginY &&
      genState.lastParams.contrast === myParams.contrast
    ) {
      return;
    }
    if (genState.isGenerating) {
      genState.nextParams = myParams;
      return;
    }
    genState.isGenerating = true;
    processing = true;

    try {
      const { image } = state;
      const worker = await workerP;
      const filtered = await worker.seamlessTileMaker(
        new Uint8Array(image.data.buffer),
        image.width,
        image.height,
        myParams.marginX,
        myParams.marginY,
        myParams.contrast
      );

      const imgData = new Uint8ClampedArray(filtered);
      setPixelsToCanvas(imgData, image.width, image.height, outputCanvas);
      outputDataURL = outputCanvas.toDataURL();
      genState.lastParams = myParams;
    } finally {
      genState.isGenerating = false;
      processing = false;
    }

    if (genState.nextParams) {
      genState.nextParams = null;
      await applyFilter();
    }
  };

  $: if (
    browser &&
    state.type === 'loaded' &&
    (marginX !== undefined || marginY !== undefined || contrastCorrectionFactor !== undefined)
  ) {
    applyFilter();
  }

  const downloadOutput = () => {
    if (!outputCanvas) {
      return;
    }
    outputCanvas.toBlob(blob => void download(blob!, 'seamless.png', 'image/png'));
  };

  const startOver = () => {
    state = { type: 'initial' };
    marginX = 0.2;
    marginY = 0.2;
    contrastCorrectionFactor = 0.7;
  };

  // Toggle between rendering the output once or as a 2x2 tile grid so the
  // seam quality is obvious at a glance.
  let tilePreview = false;
</script>

<SvelteSeo
  title="Seamless Tile Maker"
  description="Make a near-seamless texture into a fully seamless tileable one with a single-image overlap-and-blend pass."
  openGraph={{
    title: 'Seamless Tile Maker',
    description:
      'Make a near-seamless texture into a fully seamless tileable one with a single-image overlap-and-blend pass.',
    type: 'website',
  }}
/>

<div class="root">
  <div class="title-bar">
    <h2>Seamless Tile Maker</h2>
    {#if state.type !== 'initial'}
      <button on:click={startOver}>Reset</button>
    {/if}
  </div>

  {#if state.type === 'error'}
    <div class="error-container">
      <p>{state.message}</p>
      <button on:click={startOver}>Start Over</button>
    </div>
  {:else}
    <div class="main-content">
      {#if state.type === 'initial'}
        <div class="intro">
          <p>
            Takes a single texture that's already close to seamless (or mostly homogeneous with
            little large-scale structure) and blends it with a half-period-shifted copy of itself
            so that the original seams are hidden inside a smooth interpolation margin. The output
            is a new texture of the same size that tiles cleanly.
          </p>
          <p>
            This is a fixup pass, not a fully-generic seamless texture generator. Heavy structure
            crossing the original boundaries will still show artifacts.
          </p>
        </div>
        <div class="input-group">
          <Dropzone
            on:drop={e => handleFile(e.detail.acceptedFiles[0])}
            accept={['image/*']}
            containerClasses="custom-dropzone"
          >
            <p>Drop texture here</p>
          </Dropzone>
        </div>
      {/if}

      {#if state.type === 'loaded'}
        <div class="work-area">
          <div class="previews">
            <div class="preview-group">
              <h3>Input</h3>
              {#if tilePreview}
                <div
                  class="preview-frame tile"
                  style="background-image: url({state.image.dataURL});"
                />
              {:else}
                <div class="preview-frame">
                  <img src={state.image.dataURL} alt="Input texture" />
                </div>
              {/if}
            </div>
            <div class="preview-group">
              <h3>Output</h3>
              <div
                class="preview-frame"
                class:tile={tilePreview && outputDataURL}
                style={tilePreview && outputDataURL
                  ? `background-image: url(${outputDataURL});`
                  : ''}
              >
                <canvas
                  bind:this={outputCanvas}
                  width={state.image.width}
                  height={state.image.height}
                  class:hidden={tilePreview && outputDataURL}
                />
              </div>
            </div>
          </div>

          <div class="controls">
            <div class="margin-controls">
              <label class="link-row">
                <input type="checkbox" bind:checked={linkMargins} />
                Link H&nbsp;/&nbsp;V
              </label>

              <div class="control-row margin-row">
                <label for="margin-x">Horizontal Margin</label>
                <div class="slider-with-number">
                  <input
                    type="range"
                    id="margin-x"
                    min="0"
                    max="0.5"
                    step="0.005"
                    value={marginX}
                    on:input={e => setMarginX(parseFloat(e.currentTarget.value))}
                  />
                  <input
                    type="number"
                    class="percent-input"
                    min="0"
                    max="50"
                    step="0.5"
                    value={(marginX * 100).toFixed(1)}
                    on:input={e => setMarginX(parseFloat(e.currentTarget.value) / 100)}
                  />
                  <span class="percent-suffix">%</span>
                </div>
              </div>

              <div class="control-row margin-row">
                <label for="margin-y">Vertical Margin</label>
                <div class="slider-with-number">
                  <input
                    type="range"
                    id="margin-y"
                    min="0"
                    max="0.5"
                    step="0.005"
                    value={marginY}
                    on:input={e => setMarginY(parseFloat(e.currentTarget.value))}
                  />
                  <input
                    type="number"
                    class="percent-input"
                    min="0"
                    max="50"
                    step="0.5"
                    value={(marginY * 100).toFixed(1)}
                    on:input={e => setMarginY(parseFloat(e.currentTarget.value) / 100)}
                  />
                  <span class="percent-suffix">%</span>
                </div>
              </div>
            </div>

            <div class="control-row">
              <label for="contrast">
                <!-- svelte-ignore security-anchor-rel-noreferrer -->
                <a href="https://hal.inria.fr/inria-00536064v2" target="_blank">
                  Contrast-Corrected Blending
                </a>: {contrastCorrectionFactor.toFixed(2)}
              </label>
              <input
                type="range"
                id="contrast"
                min="0"
                max="1"
                step="0.01"
                bind:value={contrastCorrectionFactor}
              />
              <span class="hint">
                Variance-preserving blend. Reduces wash-out in the seam region.
              </span>
            </div>

            <div class="control-row checkbox-row">
              <label>
                <input type="checkbox" bind:checked={tilePreview} />
                2&times;2 tile preview
              </label>
            </div>

            <div class="button-row">
              <button class="apply-button" on:click={applyFilter} disabled={processing}>
                {processing ? 'Processing…' : 'Apply'}
              </button>
              <button on:click={downloadOutput}>Download</button>
            </div>
          </div>

          <div class="citation">
            Variance-preserving blend trick from Heitz &amp; Neyret,
            <a
              href="https://hal.inria.fr/inria-00536064v2"
              target="_blank"
              rel="noopener noreferrer"
            >Example-Based Repetitive Structure Synthesis</a>;
            histogram-preserving extension in
            <a
              href="https://hal.inria.fr/hal-01824773"
              target="_blank"
              rel="noopener noreferrer"
            >High-Performance By-Example Noise using a Histogram-Preserving Blending Operator</a>.
            Shadertoy demo:
            <a
              href="https://www.shadertoy.com/view/MdyfDV"
              target="_blank"
              rel="noopener noreferrer"
            >MdyfDV</a>.
          </div>
        </div>
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

  .main-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    width: 100%;
    flex: 1;
    min-height: 0;
  }

  .intro {
    max-width: 720px;
    text-align: center;
    color: #ccc;
  }

  .intro p {
    margin: 0.25rem 0;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    width: 100%;
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

  .preview-frame {
    width: 100%;
    height: 100%;
    min-height: 0;
    border: 1px solid #ccc;
    display: flex;
    justify-content: center;
    align-items: center;
    overflow: hidden;
  }

  .preview-frame img,
  .preview-frame canvas {
    width: 100%;
    height: 100%;
    min-height: 0;
    object-fit: contain;
  }

  .preview-frame.tile {
    background-repeat: repeat;
    background-size: 50% 50%;
    background-position: center;
    image-rendering: pixelated;
  }

  .hidden {
    display: none;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 1rem;
    flex-shrink: 0;
  }

  .control-row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 180px;
  }

  .margin-controls {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .link-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    cursor: pointer;
    color: #ccc;
  }

  .margin-row {
    min-width: 360px;
  }

  .slider-with-number {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .slider-with-number input[type='range'] {
    flex: 1;
    min-width: 0;
  }

  .percent-input {
    width: 4.5rem;
    box-sizing: border-box;
    padding: 2px 4px;
  }

  .percent-suffix {
    font-size: 0.85rem;
    color: #888;
    margin-left: -0.25rem;
  }

  .control-row label {
    font-size: 0.9rem;
  }

  .checkbox-row label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
  }

  .hint {
    font-size: 0.8rem;
    color: #888;
  }

  .button-row {
    display: flex;
    gap: 0.5rem;
    align-items: end;
  }

  /* Reserve enough width for the longer "Processing…" label so the row's
     overall layout (and any sliders flexing alongside it) doesn't twitch
     when the button text swaps mid-drag. */
  .apply-button {
    min-width: 9.5ch;
  }

  .citation {
    font-size: 0.78rem;
    color: #888;
    max-width: 900px;
    text-align: center;
    line-height: 1.4;
    flex-shrink: 0;
  }

  .citation a {
    color: #aaa;
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
