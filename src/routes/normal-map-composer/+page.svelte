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
    | {
        type: 'processing';
        base: ImageState | null;
        detail: ImageState | null;
      }
    | { type: 'error'; message: string };

  let state: ProcessState = { type: 'initial' };
  let outputCanvas: HTMLCanvasElement | null = null;
  let weight = 0.5;

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

  const handleFile = async (file: File, type: 'base' | 'detail') => {
    const { data, width, height } = await parseImageToRGBA(file);
    const dataURL = rgbaToDataURL(data, width, height);

    if (state.type === 'initial') {
      state = { type: 'processing', base: null, detail: null };
    }

    if (state.type === 'processing') {
      state = { ...state, [type]: { data, width, height, dataURL } };

      if (state.base && state.detail) {
        if (state.base.width !== state.detail.width || state.base.height !== state.detail.height) {
          state = { type: 'error', message: 'Base and detail maps must have the same dimensions.' };
        }
      }
    }
  };

  const compose = async (composeWeight: number) => {
    if (state.type !== 'processing' || !state.base || !state.detail || !outputCanvas) {
      return;
    }

    const worker = await workerP;
    const composed = await worker.normalMapCompose(
      new Uint8Array(state.base.data.buffer),
      new Uint8Array(state.detail.data.buffer),
      composeWeight
    );

    const imgData = new Uint8ClampedArray(composed);
    setPixelsToCanvas(imgData, state.base.width, state.base.height, outputCanvas);
  };

  $: {
    if (state.type === 'processing' && state.base && state.detail && outputCanvas) {
      compose(weight);
    }
  }

  const downloadOutput = () => {
    if (state.type !== 'processing' || !state.base || !state.detail || !outputCanvas) {
      return;
    }

    outputCanvas.toBlob(blob => void download(blob!, 'composed_normal_map.png', 'image/png'));
  };

  const startOver = () => {
    state = { type: 'initial' };
    weight = 0.5;
  };
</script>

<SvelteSeo
  title="Normal Map Composer"
  description="A tool to compose two normal maps together using Reoriented Normal Mapping."
  openGraph={{
    title: 'Normal Map Composer',
    description: 'A tool to compose two normal maps together using Reoriented Normal Mapping.',
    type: 'website',
  }}
/>

<div class="root">
  <div class="title-bar">
    <h2>Normal Map Composer</h2>
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
      <div class="inputs">
        <div class="input-group">
          <h3>Base Normal Map</h3>
          {#if state.type === 'processing' && state.base}
            <img src={state.base.dataURL} alt="Base normal map preview" />
          {:else}
            <Dropzone
              on:drop={e => handleFile(e.detail.acceptedFiles[0], 'base')}
              accept={['image/*']}
              containerClasses="custom-dropzone"
            >
              <p>Drop base normal map here</p>
            </Dropzone>
          {/if}
        </div>
        <div class="input-group">
          <h3>Detail Normal Map</h3>
          {#if state.type === 'processing' && state.detail}
            <img src={state.detail.dataURL} alt="Detail normal map preview" />
          {:else}
            <Dropzone
              on:drop={e => handleFile(e.detail.acceptedFiles[0], 'detail')}
              accept={['image/*']}
              containerClasses="custom-dropzone"
            >
              <p>Drop detail normal map here</p>
            </Dropzone>
          {/if}
        </div>
      </div>

      {#if state.type === 'processing' && state.base && state.detail}
        <div class="output-container">
          <h3>Output</h3>
          <canvas bind:this={outputCanvas} width={state.base.width} height={state.base.height} />
          <div class="controls">
            <label for="weight">Weight</label>
            <input type="range" id="weight" min="0" max="1" step="0.01" bind:value={weight} />
            <button on:click={downloadOutput}>Download</button>
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
    padding: 2rem;
    gap: 2rem;
    width: 100%;
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
    gap: 2rem;
    width: 100%;
  }

  .inputs {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 2rem;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  .input-group img {
    width: 300px;
    height: 300px;
    object-fit: contain;
    border: 1px solid #ccc;
  }

  .output-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  canvas {
    max-width: 100%;
    height: auto;
    border: 1px solid #ccc;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 1rem;
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
