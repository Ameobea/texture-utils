<script lang="ts">
  import Dropzone from 'svelte-file-dropzone';
  import { browser } from '$app/environment';
  import type * as Comlink from 'comlink';
  import type { SvelteComponent } from 'svelte';

  import { getWorkers } from 'src/workerPool';
  import type { ReverseColorRampParams } from '../../wasmWorker.worker';
  import type { WorkerInterface } from '../../wasmWorker.worker';
  import { buildReverseColorRampGenerator, ReverseColorRampCommonFunctions } from './glslGenerator';
  import ColorPicker from './ColorPicker.svelte';

  let workerP: Promise<Comlink.Remote<WorkerInterface>> = browser
    ? getWorkers().then(workers => workers.getWorker(0))
    : new Promise(() => {});

  let inputCanvas: HTMLCanvasElement;
  let outputCanvas: HTMLCanvasElement;

  let inputImage: HTMLImageElement | null = null;
  let inputImageData: ImageData | null = null;

  type RampTarget = 'roughness' | 'metalness';

  const targetLabels: Record<RampTarget, string> = {
    roughness: 'Roughness',
    metalness: 'Metalness',
  };

  const buildDefaultParams = (): ReverseColorRampParams => ({
    colorA_srgb: [0.8, 0.8, 0.8],
    colorB_srgb: [0.2, 0.2, 0.2],
    vMin: 0.1,
    vMax: 1,
    curveSteepness: 1.5,
    curveOffset: 0.5,
    perpSigma: 0.1,
    baseFallback: 0.1,
  });

  let roughnessParams: ReverseColorRampParams = buildDefaultParams();
  let metalnessParams: ReverseColorRampParams = buildDefaultParams();
  let activeTarget: RampTarget = 'roughness';
  let activeParams: ReverseColorRampParams = roughnessParams;
  let jsonImportText = '';

  let previewOpen = false;
  let previewLoading = false;
  let previewError = '';
  let previewUseRoughness = true;
  let previewUseMetalness = false;
  let PreviewComponent: typeof SvelteComponent | null = null;

  const togglePreview = async () => {
    if (previewOpen) {
      previewOpen = false;
      return;
    }

    previewOpen = true;
    previewError = '';

    if (PreviewComponent) {
      return;
    }

    previewLoading = true;
    try {
      const mod = await import('./ThreePreview.svelte');
      PreviewComponent = mod.default;
    } catch (err) {
      console.error('Failed to load 3D preview', err);
      previewError = 'Failed to load 3D preview.';
      previewOpen = false;
    } finally {
      previewLoading = false;
    }
  };

  const setActiveTarget = (target: RampTarget) => {
    activeTarget = target;
    if (inputImageData) {
      generate();
    }
  };

  const getParamsForTarget = (target: RampTarget) =>
    target === 'roughness' ? roughnessParams : metalnessParams;

  const setParamsForTarget = (target: RampTarget, next: ReverseColorRampParams) => {
    if (target === 'roughness') {
      roughnessParams = next;
    } else {
      metalnessParams = next;
    }
  };

  $: activeParams = getParamsForTarget(activeTarget);

  function swapColors() {
    const tmp = activeParams.colorA_srgb;
    activeParams.colorA_srgb = activeParams.colorB_srgb;
    activeParams.colorB_srgb = tmp;
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
      const paramsToUse = getParamsForTarget(activeTarget);
      setParamsForTarget(activeTarget, paramsToUse);
      const result = await worker.reverseColorRamp(
        inputImageData.width,
        inputImageData.height,
        paramsToUse
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

  const buildGLSLForTarget = (target: RampTarget) => {
    const params = getParamsForTarget(target);
    const fnName = target === 'roughness' ? 'roughness_from_color' : 'metalness_from_color';
    return `${ReverseColorRampCommonFunctions}\n${buildReverseColorRampGenerator(params, fnName)}`;
  };

  const copyGLSL = async (target: RampTarget) => {
    try {
      await navigator.clipboard.writeText(buildGLSLForTarget(target));
      showMessage(`${targetLabels[target]} GLSL copied to clipboard`, 'success');
    } catch (err) {
      showMessage('Failed to copy GLSL to clipboard', 'error');
      console.error('Failed to copy GLSL to clipboard', err);
    }
  };

  const downloadJSON = (target: RampTarget) => {
    const json = JSON.stringify(getParamsForTarget(target), null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${target}_reverse_color_ramp.json`;
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

  const copyJSON = async (target: RampTarget) => {
    const json = JSON.stringify(getParamsForTarget(target), null, 2);
    try {
      await navigator.clipboard.writeText(json);
      showMessage(`${targetLabels[target]} JSON copied to clipboard`, 'success');
    } catch (err) {
      showMessage('Failed to copy JSON to clipboard', 'error');
      console.error('Failed to copy JSON to clipboard', err);
    }
  };

  const isNumber = (value: unknown): value is number =>
    typeof value === 'number' && Number.isFinite(value);

  const isVec3 = (value: unknown): value is [number, number, number] =>
    Array.isArray(value) && value.length === 3 && value.every(isNumber);

  const isReverseColorRampParams = (value: unknown): value is ReverseColorRampParams => {
    if (!value || typeof value !== 'object') {
      return false;
    }

    const candidate = value as ReverseColorRampParams;
    return (
      isVec3(candidate.colorA_srgb) &&
      isVec3(candidate.colorB_srgb) &&
      isNumber(candidate.vMin) &&
      isNumber(candidate.vMax) &&
      isNumber(candidate.curveSteepness) &&
      isNumber(candidate.curveOffset) &&
      isNumber(candidate.perpSigma) &&
      isNumber(candidate.baseFallback)
    );
  };

  const importJSON = (target: RampTarget) => {
    const trimmed = jsonImportText.trim();
    if (!trimmed) {
      showMessage('Paste JSON before importing', 'error');
      return;
    }

    try {
      const parsed = JSON.parse(trimmed);
      if (!isReverseColorRampParams(parsed)) {
        showMessage('JSON does not match reverse color ramp format', 'error');
        return;
      }
      setParamsForTarget(target, parsed);
      showMessage(`${targetLabels[target]} params loaded`, 'success');
      if (target === activeTarget) {
        generate();
      }
    } catch (err) {
      showMessage('Invalid JSON', 'error');
      console.error('Failed to parse JSON', err);
    }
  };
</script>

<div class="container">
  {#if !inputImage}
    <Dropzone on:drop={handleDrop} accept="image/*" containerClasses="custom-dropzone" />
  {:else}
    <div class="image-grid">
      <div class="input-header" aria-hidden="true" />
      <div class="output-header">
        <div class="preview-controls">
          <button on:click={togglePreview} disabled={previewLoading}>
            {previewOpen ? 'Show 2D Output' : 'Show 3D Preview'}
          </button>
          <label class="preview-toggle">
            <input type="checkbox" bind:checked={previewUseRoughness} disabled={!previewOpen} />
            Apply to roughness
          </label>
          <label class="preview-toggle">
            <input type="checkbox" bind:checked={previewUseMetalness} disabled={!previewOpen} />
            Apply to metalness
          </label>
        </div>
        {#if previewOpen && previewLoading}
          <div class="preview-status">Loading 3D preview…</div>
        {/if}
      </div>

      <div class="canvas-wrapper input-slot">
        <canvas bind:this={inputCanvas} class="input-canvas" />
      </div>
      <div class="output-panel">
        <div class="canvas-wrapper output-slot">
          <canvas bind:this={outputCanvas} class="output-canvas {previewOpen ? 'hidden' : ''}" />
          {#if PreviewComponent}
            <div class="preview-host {previewOpen ? '' : 'hidden'}">
              <svelte:component
                this={PreviewComponent}
                {inputImage}
                {roughnessParams}
                {metalnessParams}
                useRoughness={previewUseRoughness}
                useMetalness={previewUseMetalness}
                active={previewOpen}
              />
            </div>
          {/if}
        </div>
        {#if previewError}
          <div class="message error">{previewError}</div>
        {/if}
      </div>
    </div>
  {/if}

  {#if inputImage}
    <div class="target-tabs">
      <button
        class:active={activeTarget === 'roughness'}
        on:click={() => setActiveTarget('roughness')}
      >
        Roughness
      </button>
      <button
        class:active={activeTarget === 'metalness'}
        on:click={() => setActiveTarget('metalness')}
      >
        Metalness
      </button>
    </div>

    <div class="color-picker-center-wrapper">
      <div class="color-picker-grid swap-row">
        <ColorPicker
          label="Color A"
          value={activeParams.colorA_srgb}
          onInput={newColor => {
            activeParams.colorA_srgb = newColor;
            generate();
          }}
        />
        <button class="swap-btn" title="Swap colors" on:click={swapColors}>&#8646;</button>
        <ColorPicker
          label="Color B"
          value={activeParams.colorB_srgb}
          onInput={newColor => {
            activeParams.colorB_srgb = newColor;
            generate();
          }}
        />
      </div>
    </div>

    <div class="slider-grid">
      <div>
        <label for="vMin">vMin: {activeParams.vMin}</label>
        <input
          id="vMin"
          type="range"
          min="0"
          max="1"
          step="0.01"
          bind:value={activeParams.vMin}
          on:input={generate}
        />
      </div>
      <div>
        <label for="vMax">vMax: {activeParams.vMax}</label>
        <input
          id="vMax"
          type="range"
          min="0"
          max="1"
          step="0.01"
          bind:value={activeParams.vMax}
          on:input={generate}
        />
      </div>
    </div>

    <div>
      <label for="curveSteepness">Curve Steepness: {activeParams.curveSteepness}</label>
      <input
        id="curveSteepness"
        type="range"
        min="1"
        max="5"
        step="0.01"
        bind:value={activeParams.curveSteepness}
        on:input={generate}
      />
    </div>

    <div>
      <label for="curveOffset">Curve Offset: {activeParams.curveOffset}</label>
      <input
        id="curveOffset"
        type="range"
        min="0.001"
        max="0.999"
        step="0.001"
        bind:value={activeParams.curveOffset}
        on:input={generate}
      />
    </div>

    <div>
      <label for="perpSigma">Perpendicular Sigma: {activeParams.perpSigma}</label>
      <input
        id="perpSigma"
        type="range"
        min="0"
        max="1"
        step="0.01"
        bind:value={activeParams.perpSigma}
        on:input={generate}
      />
    </div>

    <div>
      <label for="baseFallback">Base Fallback: {activeParams.baseFallback}</label>
      <input
        id="baseFallback"
        type="range"
        min="0"
        max="1"
        step="0.01"
        bind:value={activeParams.baseFallback}
        on:input={generate}
      />
    </div>

    <div class="button-row">
      <button on:click={() => copyGLSL(activeTarget)}>
        Copy {targetLabels[activeTarget]} GLSL
      </button>
      <button on:click={() => downloadJSON(activeTarget)}>
        Download {targetLabels[activeTarget]} JSON
      </button>
      <button on:click={() => copyJSON(activeTarget)}>
        Copy {targetLabels[activeTarget]} JSON
      </button>
    </div>

    <div class="import-row">
      <textarea
        class="import-textarea"
        rows="4"
        bind:value={jsonImportText}
        placeholder="Paste reverse color ramp JSON here"
        style="width: calc(100% - 2rem);"
      />
      <div class="import-buttons">
        <button on:click={() => importJSON('roughness')}>Load Roughness JSON</button>
        <button on:click={() => importJSON('metalness')}>Load Metalness JSON</button>
      </div>
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
    grid-template-rows: auto 1fr;
    grid-template-areas:
      'input-header output-header'
      'input-slot output-slot';
    gap: 1rem;
    max-height: calc(100vh - 300px);
    align-items: start;
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

  .input-header {
    grid-area: input-header;
    min-height: 2.6rem;
    width: 100%;
  }

  .input-slot {
    grid-area: input-slot;
  }

  .output-panel {
    grid-area: output-slot;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    width: 100%;
  }

  .output-header {
    grid-area: output-header;
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 0.75rem;
    align-items: center;
  }

  .output-slot {
    width: 100%;
  }

  .output-canvas.hidden {
    display: none;
  }

  .preview-host {
    position: absolute;
    inset: 0;
  }

  .preview-host.hidden {
    opacity: 0;
    pointer-events: none;
  }

  .color-picker-center-wrapper {
    display: flex;
    justify-content: center;
    width: 100%;
    margin: 0.5rem 0 1.5rem 0;
  }

  .target-tabs {
    display: inline-flex;
    gap: 0.5rem;
    align-self: center;
    background: #1d1d1d;
    border: 1px solid #333;
    border-radius: 999px;
    padding: 0.25rem;
  }

  .target-tabs button {
    border-radius: 999px;
    padding: 0.35rem 1rem;
    border: 1px solid transparent;
    background: transparent;
  }

  .target-tabs button.active {
    background: #3a3a3a;
    border-color: #5a5a5a;
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
    flex-wrap: wrap;
    gap: 1rem;
  }

  .import-row {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .import-textarea {
    width: 100%;
    min-height: 110px;
    background: #1a1a1a;
    color: #e8e8e8;
    border: 1px solid #444;
    padding: 0.6rem;
    resize: vertical;
    font-family: 'SFMono-Regular', Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New',
      monospace;
    font-size: 0.9rem;
  }

  .import-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .preview-controls {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: center;
  }

  .preview-toggle {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    font-size: 0.95rem;
  }

  .preview-status {
    font-size: 0.95rem;
    color: #c7c7c7;
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
