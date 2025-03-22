<script lang="ts" context="module">
  const description =
    'Online tool to create a color ramp that can be used to map a grayscale texture to color.';

  const buildDefaultColorRamp = (): ColorRamp => {
    // TODO: use a good ramp here
    // const steps: ColorRampStep[] = [
    //   { position: 0, color: [0, 0, 0] },
    //   { position: 1 / 3, color: [85, 45, 91] },
    //   { position: 2 / 3, color: [120, 170, 20] },
    //   { position: 1, color: [255, 255, 255] },
    // ];

    const rawSteps: [number, number, number][] = [
      [18.156, 23.205, 23.052],
      [23.256, 33.405, 33.251999999999995],
      [56.1, 53.55, 68.85000000000001],
      [132.6, 137.70000000000002, 186.15],
      [56.1, 61.199999999999996, 51],
    ];
    const steps: ColorRampStep[] = rawSteps.map((color, i) => ({
      position: i / (rawSteps.length - 1),
      color,
    }));

    return { steps };
  };
</script>

<script lang="ts">
  import type * as Comlink from 'comlink';
  import { browser } from '$app/environment';
  import SvelteSeo from 'svelte-seo';
  import Dropzone from 'svelte-file-dropzone';
  import download from 'downloadjs';

  import type { ColorRamp, ColorRampStep, WorkerInterface } from 'src/wasmWorker.worker';
  import { getWorkers, type WorkerPoolManager } from 'src/workerPool';
  import {
    parseImageToRGBA,
    setImageData,
    setImageDataToCanvas,
    setPixelsToCanvas,
  } from 'src/imageHelpers';
  import { onMount } from 'svelte';
  import ColorRampConfigurator from './ColorRampConfigurator.svelte';

  onMount(async () => {
    if (document.getElementById('img-comparison-slider')) {
      return;
    }

    const script = document.createElement('script');
    script.id = 'img-comparison-slider';
    script.defer = true;
    script.src = '/imgComparisonSlider.js';
    document.body.appendChild(script);
  });

  type ProcessState =
    | { type: 'notStarted' }
    | { type: 'uploaded'; data: Uint8ClampedArray; width: number; height: number }
    | { type: 'error'; message: string };

  let state: ProcessState = { type: 'notStarted' };
  let rxImg: HTMLCanvasElement | null = null;
  let ramp: ColorRamp = buildDefaultColorRamp();

  let workerP: Promise<Comlink.Remote<WorkerInterface>> = browser
    ? getWorkers().then(workers => workers.getWorker(0))
    : new Promise(() => {});

  const handleFilesSelect = async (e: CustomEvent<{ acceptedFiles: File[] }>) => {
    const { acceptedFiles } = e.detail;
    if (!acceptedFiles.length) {
      return;
    }

    const file = acceptedFiles[0];
    const { data: imgData, width, height } = await parseImageToRGBA(file);

    const worker = await workerP;
    await worker.setColorRampInputTexture(new Uint8Array(imgData.buffer));

    state = { type: 'uploaded', data: imgData, width, height };
  };

  const generate = async (
    rampDef: ColorRamp,
    width: number,
    height: number,
    rxImg: HTMLCanvasElement
  ) => {
    const worker = await workerP;
    const applied = await worker.applyColorRamp(rampDef);
    const imgData = new Uint8ClampedArray(applied);
    setImageDataToCanvas(rxImg, { data: imgData, width, height });
  };

  $: if (state.type === 'uploaded' && rxImg) {
    generate(ramp, state.width, state.height, rxImg);
  }

  const downloadGrayscaleTexture = async () => {
    if (state.type !== 'uploaded') {
      return;
    }

    const worker = await workerP;
    const grayscaleImageData: Uint8Array = await worker.getGrayscaleColorRampImageData();
    const pixels = new Uint8ClampedArray(grayscaleImageData);
    const canvas = document.createElement('canvas');
    canvas.width = state.width;
    canvas.height = state.height;
    const ctx = canvas.getContext('2d')!;
    const imageData = new ImageData(pixels, state.width, state.height);
    ctx.putImageData(imageData, 0, 0);
    canvas.toBlob(blob => void download(blob!, 'grayscale_texture.png', 'image/png'));
  };

  const downloadColorTexture = async () => {
    if (state.type !== 'uploaded') {
      return;
    }

    const worker = await workerP;
    const colorImageData: Uint8Array = await worker.applyColorRamp(ramp);
    const pixels = new Uint8ClampedArray(colorImageData);
    const canvas = setPixelsToCanvas(pixels, state.width, state.height);
    canvas.toBlob(blob => void download(blob!, 'color_texture.png', 'image/png'));
  };

  const copyLUTToClipboard = () => {
    const fixedSteps = 256;
    const sampledLUT: string[] = [];

    const sortedSteps = [...ramp.steps].sort((a, b) => a.position - b.position);

    for (let i = 0; i < fixedSteps; i++) {
      const t = i / (fixedSteps - 1);

      if (t <= sortedSteps[0].position) {
        const [r, g, b] = sortedSteps[0].color.map(c => c / 255);
        sampledLUT.push(`vec3(${r.toFixed(5)}, ${g.toFixed(5)}, ${b.toFixed(5)})`);
        continue;
      }
      if (t >= sortedSteps[sortedSteps.length - 1].position) {
        const [r, g, b] = sortedSteps[sortedSteps.length - 1].color.map(c => c / 255);
        sampledLUT.push(`vec3(${r.toFixed(5)}, ${g.toFixed(5)}, ${b.toFixed(5)})`);
        continue;
      }

      let leftStep = sortedSteps[0];
      let rightStep = sortedSteps[sortedSteps.length - 1];
      for (let j = 0; j < sortedSteps.length - 1; j++) {
        if (t >= sortedSteps[j].position && t <= sortedSteps[j + 1].position) {
          leftStep = sortedSteps[j];
          rightStep = sortedSteps[j + 1];
          break;
        }
      }
      let f = (t - leftStep.position) / (rightStep.position - leftStep.position);
      if (isNaN(f)) {
        f = 0;
      }

      const r = (leftStep.color[0] * (1 - f) + rightStep.color[0] * f) / 255;
      const g = (leftStep.color[1] * (1 - f) + rightStep.color[1] * f) / 255;
      const b = (leftStep.color[2] * (1 - f) + rightStep.color[2] * f) / 255;

      sampledLUT.push(`vec3(${r.toFixed(5)}, ${g.toFixed(5)}, ${b.toFixed(5)})`);
    }

    const lutString = `const vec3[${fixedSteps}] COLOR_RAMP = vec3[${fixedSteps}](${sampledLUT.join(
      ', '
    )});`;
    navigator.clipboard.writeText(lutString);
  };
</script>

<SvelteSeo
  title="Color Ramp Builder"
  {description}
  openGraph={{
    title: 'Color Ramp Builder',
    description,
    type: 'website',
    images: [], // TODO?
  }}
/>

<div class="root">
  {#if state.type === 'notStarted'}
    <div
      style="width: 100%; height: 100%; display: flex; flex-direction: column; justify-content: center; align-items: center;"
    >
      <h2>Seamless Texture Crossfade Stitcher</h2>
      <Dropzone on:drop={handleFilesSelect} accept={['image/*']} containerClasses="custom-dropzone">
        <p>Drag + drop a texture here to use while building the ramp.</p>
      </Dropzone>
    </div>
  {:else if state.type === 'uploaded'}
    <div class="content-container">
      <div class="top-left-button-stack">
        <button type="button" on:click={downloadGrayscaleTexture}>
          Download Grayscale Texture
        </button>
        <button type="button" on:click={downloadColorTexture}>Download Color Texture</button>
        <button type="button" on:click={copyLUTToClipboard}>Copy LUT to Clipboard</button>
      </div>

      <ColorRampConfigurator bind:steps={ramp.steps} />
      <div class="image">
        <img-comparison-slider
          style="width: calc(min(100%, {state.width}px)); height: auto; aspect-ratio: {state.width} / {state.height};"
        >
          <img
            height={state.height}
            width={state.width}
            style="width: 100%; height: auto"
            slot="first"
            use:setImageData={state}
            alt="before"
          />
          <canvas
            height={state.height}
            width={state.width}
            style="width: 100%; height: auto"
            slot="second"
            alt="after"
            bind:this={rxImg}
          />
        </img-comparison-slider>
      </div>
    </div>
  {:else if state.type === 'error'}
    <div>{state.message}</div>
  {/if}
</div>

<style lang="css">
  .root {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 40px);
    margin-top: 20px;
    margin-bottom: 20px;
  }

  .content-container {
    display: flex;
    flex-direction: column;
    width: 100%;
    justify-content: center;
    align-items: center;
  }

  .image {
    overflow: auto;
    display: flex;
    flex: 1;
    width: 100%;
    justify-content: center;
    align-items: center;
  }

  .top-left-button-stack {
    position: absolute;
    top: 4px;
    left: 4px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
</style>
