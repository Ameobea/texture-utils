<script lang="ts">
  import Dropzone from 'svelte-file-dropzone';
  import { browser } from '$app/environment';
  import type * as Comlink from 'comlink';

  import { getWorkers } from 'src/workerPool';
  import type { ReverseColorRampParams } from '../../wasmWorker.worker';
  import type { WorkerInterface } from '../../wasmWorker.worker';
  import { generateRoughnessGLSL } from './glslGenerator';
  import ColorPicker from './ColorPicker.svelte';

  let workerP: Promise<Comlink.Remote<WorkerInterface>> = browser
    ? getWorkers().then(workers => workers.getWorker(0))
    : new Promise(() => {});

  let inputCanvas: HTMLCanvasElement;
  let outputCanvas: HTMLCanvasElement;

  let inputImage: HTMLImageElement | null = null;
  let inputImageData: ImageData | null = null;

  let params: ReverseColorRampParams = {
    colorA_srgb: [0.8, 0.8, 0.8],
    colorB_srgb: [0.2, 0.2, 0.2],
    vMin: 0.1,
    vMax: 1,
    curveSteepness: 1.5,
    curveOffset: 0.5,
    perpSigma: 0.1,
    baseFallback: 0.1,
  };

  function swapColors() {
    const tmp = params.colorA_srgb;
    params.colorA_srgb = params.colorB_srgb;
    params.colorB_srgb = tmp;
    generate();
  }

  const handleDrop = async (e: CustomEvent) => {
    const file = e.detail.acceptedFiles[0];
    if (!file) {
      return;
    }

    const reader = new FileReader();
    reader.onload = () => {
      inputImage = new Image();
      inputImage.src = reader.result as string;
      inputImage.onload = async () => {
        inputCanvas.width = inputImage!.width;
        inputCanvas.height = inputImage!.height;
        outputCanvas.width = inputImage!.width;
        outputCanvas.height = inputImage!.height;

        const ctx = inputCanvas.getContext('2d');
        ctx?.drawImage(inputImage!, 0, 0);
        const imgData = ctx?.getImageData(0, 0, inputImage!.width, inputImage!.height) ?? null;
        if (!imgData) {
          return;
        }
        const worker = await workerP;
        await worker.reverseColorRampSetInputTexture(new Uint8Array(imgData.data.buffer));
        inputImageData = imgData;
        generate();
      };
    };
    reader.readAsDataURL(file);
  };

  let isGenerating = false;
  let generationQueued = false;
  const generate = async () => {
    if (!inputImageData) {
      return;
    }

    if (isGenerating) {
      generationQueued = true;
      return;
    }
    isGenerating = true;

    try {
      const worker = await workerP;
      const result = await worker.reverseColorRamp(
        inputImageData.width,
        inputImageData.height,
        params
      );

      const outImageData = new ImageData(
        new Uint8ClampedArray(result.buffer),
        inputImageData.width,
        inputImageData.height
      );
      outputCanvas.getContext('2d')?.putImageData(outImageData, 0, 0);
    } finally {
      isGenerating = false;
      if (generationQueued) {
        generationQueued = false;
        generate();
      }
    }
  };

  const downloadGLSL = () => {
    const glsl = generateRoughnessGLSL(params);
    const blob = new Blob([glsl], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'roughness_from_color.glsl';
    a.click();
    URL.revokeObjectURL(url);
  };

  const downloadJSON = () => {
    const json = JSON.stringify(params, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'reverse_color_ramp.json';
    a.click();
    URL.revokeObjectURL(url);
  };

  // Message state for success/error display
  let message: string = '';
  let messageType: 'success' | 'error' | '' = '';
  let messageTimeout: ReturnType<typeof setTimeout> | null = null;

  function showMessage(msg: string, type: 'success' | 'error') {
    message = msg;
    messageType = type;
    if (messageTimeout) clearTimeout(messageTimeout);
    messageTimeout = setTimeout(() => {
      message = '';
      messageType = '';
    }, 2000);
  }

  const copyJSON = async () => {
    const json = JSON.stringify(params, null, 2);
    try {
      await navigator.clipboard.writeText(json);
      showMessage('JSON copied to clipboard', 'success');
    } catch (err) {
      showMessage('Failed to copy JSON to clipboard', 'error');
      console.error('Failed to copy JSON to clipboard', err);
    }
  };
</script>

<div class="container">
  {#if !inputImage}
    <Dropzone on:drop={handleDrop} accept="image/*" containerClasses="custom-dropzone" />
  {:else}
    <div class="image-grid">
      <div class="canvas-wrapper">
        <canvas bind:this={inputCanvas} class="input-canvas" />
      </div>
      <div class="canvas-wrapper">
        <canvas bind:this={outputCanvas} class="output-canvas" />
      </div>
    </div>
  {/if}

  {#if inputImage}
    <div class="color-picker-center-wrapper">
      <div class="color-picker-grid swap-row">
        <ColorPicker
          label="Color A"
          value={params.colorA_srgb}
          onInput={newColor => {
            params.colorA_srgb = newColor;
            generate();
          }}
        />
        <button class="swap-btn" title="Swap colors" on:click={swapColors}>&#8646;</button>
        <ColorPicker
          label="Color B"
          value={params.colorB_srgb}
          onInput={newColor => {
            params.colorB_srgb = newColor;
            generate();
          }}
        />
      </div>
    </div>

    <div class="slider-grid">
      <div>
        <label for="vMin">vMin: {params.vMin}</label>
        <input
          id="vMin"
          type="range"
          min="0"
          max="1"
          step="0.01"
          bind:value={params.vMin}
          on:input={generate}
        />
      </div>
      <div>
        <label for="vMax">vMax: {params.vMax}</label>
        <input
          id="vMax"
          type="range"
          min="0"
          max="1"
          step="0.01"
          bind:value={params.vMax}
          on:input={generate}
        />
      </div>
    </div>

    <div>
      <label for="curveSteepness">Curve Steepness: {params.curveSteepness}</label>
      <input
        id="curveSteepness"
        type="range"
        min="1"
        max="5"
        step="0.01"
        bind:value={params.curveSteepness}
        on:input={generate}
      />
    </div>

    <div>
      <label for="curveOffset">Curve Offset: {params.curveOffset}</label>
      <input
        id="curveOffset"
        type="range"
        min="0.001"
        max="0.999"
        step="0.001"
        bind:value={params.curveOffset}
        on:input={generate}
      />
    </div>

    <div>
      <label for="perpSigma">Perpendicular Sigma: {params.perpSigma}</label>
      <input
        id="perpSigma"
        type="range"
        min="0"
        max="1"
        step="0.01"
        bind:value={params.perpSigma}
        on:input={generate}
      />
    </div>

    <div>
      <label for="baseFallback">Base Fallback: {params.baseFallback}</label>
      <input
        id="baseFallback"
        type="range"
        min="0"
        max="1"
        step="0.01"
        bind:value={params.baseFallback}
        on:input={generate}
      />
    </div>

    <div class="button-row">
      <button on:click={downloadGLSL}>Download GLSL</button>
      <button on:click={downloadJSON}>Download JSON</button>
      <button on:click={copyJSON}>Copy JSON to Clipboard</button>
    </div>
    {#if message}
      <div class="message {messageType}">{message}</div>
    {/if}
  {/if}
</div>

<style lang="css">
  .container {
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .image-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    max-height: calc(100vh - 300px);
    align-items: center;
    justify-items: center;
    overflow: hidden;
  }

  .canvas-wrapper {
    position: relative;
    width: 100%;
    aspect-ratio: 1 / 1;
    max-width: 100%;
    max-height: 60vh;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: #222;
  }
  .input-canvas,
  .output-canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    border: 1px solid #888;
    background: transparent;
    display: block;
    box-sizing: border-box;
  }

  .color-picker-center-wrapper {
    display: flex;
    justify-content: center;
    width: 100%;
    margin: 0.5rem 0 1.5rem 0;
  }
  .color-picker-grid {
    display: grid;
    grid-template-columns: auto 1.5em auto;
    gap: 1.5rem;
    align-items: center;
    justify-items: center;
    min-width: 350px;
    max-width: 500px;
    margin: 0 auto;
  }

  .swap-btn {
    background: #222;
    color: #e8e8e8;
    border: 1px solid #888;
    border-radius: 50%;
    width: 1.5em;
    height: 1.5em;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.2em;
    cursor: pointer;
    margin: 0 auto;
    transition: background 0.2s;
  }
  .swap-btn:hover {
    background: #444;
  }

  .slider-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .button-row {
    display: flex;
    gap: 1rem;
  }

  input[type='range'] {
    width: 100%;
  }

  button {
    background-color: #333;
    color: #e8e8e8;
    border: 1px solid #888;
    padding: 4px 8px;
    cursor: pointer;
  }

  button:hover {
    background-color: #444;
  }

  .message {
    margin-top: 1rem;
    font-weight: bold;
    font-size: 1.1em;
    transition: opacity 0.2s;
  }
  .message.success {
    color: #1fa31f;
  }
  .message.error {
    color: #d32f2f;
  }
</style>
