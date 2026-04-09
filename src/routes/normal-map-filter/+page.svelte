<script lang="ts">
  import type * as Comlink from 'comlink';
  import { browser } from '$app/environment';
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

  let filterMode: number = 0; // 0=lowpass, 1=highpass, 2=bandpass, 3=bandreject
  let sigma = 5;
  let sigmaHigh = 20;
  let useBilateral = false;
  let rangeSigma = 0.3;
  let processing = false;

  const filterModeLabels = [
    { value: 0, label: 'Low-pass (blur)' },
    { value: 1, label: 'High-pass (detail)' },
    { value: 2, label: 'Band-pass' },
    { value: 3, label: 'Band-reject (notch)' },
  ];

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
  };

  const applyFilter = async () => {
    if (state.type !== 'loaded' || !outputCanvas) {
      return;
    }

    processing = true;
    try {
      const { image } = state;
      const worker = await workerP;
      const filtered = await worker.normalMapFilter(
        new Uint8Array(image.data.buffer),
        image.width,
        image.height,
        filterMode,
        sigma,
        filterMode >= 2 ? sigmaHigh : 0,
        useBilateral,
        rangeSigma
      );

      const imgData = new Uint8ClampedArray(filtered);
      setPixelsToCanvas(imgData, image.width, image.height, outputCanvas);
    } finally {
      processing = false;
    }
  };

  const downloadOutput = () => {
    if (!outputCanvas) {
      return;
    }
    outputCanvas.toBlob(blob => void download(blob!, 'filtered_normal_map.png', 'image/png'));
  };

  const startOver = () => {
    state = { type: 'initial' };
    filterMode = 0;
    sigma = 5;
    sigmaHigh = 20;
    useBilateral = false;
    rangeSigma = 0.3;
  };

  const needsDualSigma = (mode: number) => mode >= 2;
</script>

<SvelteSeo
  title="Normal Map Filter"
  description="Filter normal maps to extract or remove detail at specific frequency scales using Gaussian filtering."
  openGraph={{
    title: 'Normal Map Filter',
    description: 'Filter normal maps to extract or remove detail at specific frequency scales using Gaussian filtering.',
    type: 'website',
  }}
/>

<div class="root">
  <div class="title-bar">
    <h2>Normal Map Filter</h2>
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
        <div class="input-group">
          <h3>Normal Map</h3>
          <Dropzone
            on:drop={e => handleFile(e.detail.acceptedFiles[0])}
            accept={['image/*']}
            containerClasses="custom-dropzone"
          >
            <p>Drop normal map here</p>
          </Dropzone>
        </div>
      {/if}

      {#if state.type === 'loaded'}
        <div class="work-area">
          <div class="previews">
            <div class="preview-group">
              <h3>Input</h3>
              <img src={state.image.dataURL} alt="Input normal map" />
            </div>
            <div class="preview-group">
              <h3>Output</h3>
              <canvas
                bind:this={outputCanvas}
                width={state.image.width}
                height={state.image.height}
              />
            </div>
          </div>

          <div class="controls">
            <div class="control-row">
              <label for="filter-mode">Filter Mode</label>
              <select id="filter-mode" bind:value={filterMode}>
                {#each filterModeLabels as { value, label }}
                  <option {value}>{label}</option>
                {/each}
              </select>
            </div>

            <div class="control-row">
              <label for="sigma">
                {needsDualSigma(filterMode) ? 'Sigma Low' : 'Sigma'} (px): {sigma}
              </label>
              <input
                type="range"
                id="sigma"
                min="0.5"
                max="100"
                step="0.5"
                bind:value={sigma}
              />
            </div>

            {#if needsDualSigma(filterMode)}
              <div class="control-row">
                <label for="sigma-high">Sigma High (px): {sigmaHigh}</label>
                <input
                  type="range"
                  id="sigma-high"
                  min="0.5"
                  max="100"
                  step="0.5"
                  bind:value={sigmaHigh}
                />
              </div>
            {/if}

            <div class="control-row checkbox-row">
              <label>
                <input type="checkbox" bind:checked={useBilateral} />
                Edge-preserving (bilateral)
              </label>
            </div>

            {#if useBilateral}
              <div class="control-row">
                <label for="range-sigma">Range Sigma: {rangeSigma}</label>
                <input
                  type="range"
                  id="range-sigma"
                  min="0.05"
                  max="1.5"
                  step="0.05"
                  bind:value={rangeSigma}
                />
                <span class="hint">Lower = stronger edge preservation</span>
              </div>
            {/if}

            <div class="button-row">
              <button on:click={applyFilter} disabled={processing}>
                {processing ? 'Processing...' : 'Apply Filter'}
              </button>
              <button on:click={downloadOutput}>Download</button>
            </div>
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

  .preview-group img,
  .preview-group canvas {
    width: 100%;
    height: 100%;
    min-height: 0;
    object-fit: contain;
    border: 1px solid #ccc;
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
  }

  .control-row label {
    font-size: 0.9rem;
  }

  .control-row select {
    padding: 0.3rem;
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
