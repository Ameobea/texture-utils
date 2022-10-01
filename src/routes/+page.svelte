<script lang="ts">
  import Dropzone from 'svelte-file-dropzone';

  import { parseImageToRGBA } from 'src/processUpload';
  import { onMount } from 'svelte';
  import type { Tensor } from '@tensorflow/tfjs';
  import { browser } from '$app/environment';

  const clamp = (value: number, min: number, max: number) => {
    return Math.min(Math.max(value, min), max);
  };

  /**
   * Expects RGB from [0, 255]
   *
   * From https://stackoverflow.com/a/17243070/3833068
   */
  function RGBtoHSV(r: number, g: number, b: number): [number, number, number] {
    r = clamp(r, 0, 255);
    g = clamp(g, 0, 255);
    b = clamp(b, 0, 255);

    var max = Math.max(r, g, b),
      min = Math.min(r, g, b),
      d = max - min,
      h,
      s = max === 0 ? 0 : d / max,
      v = max / 255;

    switch (max) {
      case min:
        h = 0;
        break;
      case r:
        h = g - b + d * (g < b ? 6 : 0);
        h /= 6 * d;
        break;
      case g:
        h = b - r + d * 2;
        h /= 6 * d;
        break;
      case b:
        h = r - g + d * 4;
        h /= 6 * d;
        break;
      default: {
        throw new Error('Invalid color');
      }
    }

    return [h, s, v];
  }

  /**
   * Expects HSV from [0, 1], returns RGB from [0, 255]
   *
   * From https://stackoverflow.com/a/17243070/3833068
   */
  function HSVtoRGB(h: number, s: number, v: number): [number, number, number] {
    h = clamp(h, 0, 1);
    s = clamp(s, 0, 1);
    v = clamp(v, 0, 1);

    var r, g, b, i, f, p, q, t;
    i = Math.floor(h * 6);
    f = h * 6 - i;
    p = v * (1 - s);
    q = v * (1 - f * s);
    t = v * (1 - (1 - f) * s);
    switch (i % 6) {
      case 0:
        (r = v), (g = t), (b = p);
        break;
      case 1:
        (r = q), (g = v), (b = p);
        break;
      case 2:
        (r = p), (g = v), (b = t);
        break;
      case 3:
        (r = p), (g = q), (b = v);
        break;
      case 4:
        (r = t), (g = p), (b = v);
        break;
      case 5:
        (r = v), (g = p), (b = q);
        break;
      default:
        throw new Error('unreachable' + i);
    }
    return [r * 255, g * 255, b * 255];
  }

  type ProcessState =
    | { type: 'notStarted' }
    | { type: 'converting' }
    | { type: 'converted'; data: Uint8ClampedArray; width: number; height: number }
    | { type: 'error'; message: string }
    | {
        type: 'training';
        data: Uint8ClampedArray;
        width: number;
        height: number;
      }
    | { type: 'trained'; width: number; height: number };

  let processState: ProcessState = { type: 'notStarted' };
  let modelMod: typeof import('../model') | null = null;

  onMount(async () => {
    const script = document.createElement('script');
    script.defer = true;
    script.src = 'https://unpkg.com/img-comparison-slider@7/dist/index.js';
    document.body.appendChild(script);

    modelMod = await import('../model');
  });

  async function handleFilesSelect(e: any) {
    const { acceptedFiles } = e.detail;
    if (acceptedFiles.length) {
      const { data, width, height } = await parseImageToRGBA(acceptedFiles[0]);
      processState = { type: 'converted', data, width, height };
    }
  }

  const renderToCanvas = (
    canvas: HTMLCanvasElement,
    { data, width, height }: Extract<ProcessState, { type: 'converted' }>
  ) => {
    const ctx = canvas.getContext('2d')!;
    const imageData = new ImageData(data, width, height);
    ctx.putImageData(imageData, 0, 0);
  };

  const setImageData = (
    img: HTMLImageElement,
    { data: ysDataUint8, width, height }: { data: Uint8ClampedArray; width: number; height: number }
  ) => {
    if (!browser) {
      return;
    }

    const scratchCanvas = document.createElement('canvas');
    const scratchCtx = scratchCanvas.getContext('2d')!;
    scratchCanvas.width = width;
    scratchCanvas.height = height;
    const ysDataUint8Clamped = new Uint8ClampedArray(ysDataUint8);
    const imageData = new ImageData(ysDataUint8Clamped, width, height);
    scratchCtx.putImageData(imageData, 0, 0);
    img.src = scratchCanvas.toDataURL();
  };

  let rxImg: HTMLImageElement | null = null;
  const startTraining = async () => {
    if (!modelMod) {
      alert('Model not loaded yet');
      return;
    }

    if (processState.type !== 'converted') {
      throw new Error('Invalid state: ' + processState.type);
    }

    const { data, width, height } = processState;
    processState = { type: 'training', data, width, height };

    const uniqPixels = new Set<number>();
    const uniqPixelIndices = [];

    // Map [0,255] to [-1,1] and strip off alpha channel
    const dataFloat = new Float32Array(width * height * 3);
    for (let pxIx = 0; pxIx < width * height; pxIx += 1) {
      // dataFloat[pxIx * 3 + 0] = data[pxIx * 4 + 0] / 127.5 - 1;
      // dataFloat[pxIx * 3 + 1] = data[pxIx * 4 + 1] / 127.5 - 1;
      // dataFloat[pxIx * 3 + 2] = data[pxIx * 4 + 2] / 127.5 - 1;

      const r = data[pxIx * 4 + 0];
      const g = data[pxIx * 4 + 1];
      const b = data[pxIx * 4 + 2];

      // Convert RGB values to a single number
      const px = r * 65536 + g * 256 + b;
      if (!uniqPixels.has(px)) {
        uniqPixels.add(px);
        uniqPixelIndices.push(pxIx);
      }
      // else if (Math.random() > 0.8) {
      //   uniqPixelIndices.push(pxIx);
      // }

      const [h, s, v] = RGBtoHSV(r, g, b);
      dataFloat[pxIx * 3 + 0] = h * 2 - 1;
      dataFloat[pxIx * 3 + 1] = s * 2 - 1;
      dataFloat[pxIx * 3 + 2] = v * 2 - 1;
    }

    // modelMod.tf.setBackend('cpu');

    const model = modelMod.buildModel(4, false);
    model.summary();

    const optimizer = modelMod.tf.train.sgd(0.1);

    model.compile({
      optimizer,
      // loss: 'meanSquaredError',
      loss: 'meanAbsoluteError',
    });

    const uniquePixelsData = new Float32Array(uniqPixelIndices.length * 3);
    let i = 0;
    for (const pxIx of uniqPixelIndices) {
      const [h, s, v] = dataFloat.slice(pxIx * 3, pxIx * 3 + 3);
      uniquePixelsData[i * 3 + 0] = h;
      uniquePixelsData[i * 3 + 1] = s;
      uniquePixelsData[i * 3 + 2] = v;
      i += 1;
    }

    const trainingTensor = modelMod.tf.tensor2d(uniquePixelsData, [uniqPixelIndices.length, 3]);
    const fullInputsTensor = modelMod.tf.tensor2d(dataFloat, [width * height, 3]);

    await model.fit(trainingTensor, trainingTensor, {
      epochs: 50,
      callbacks: {
        onEpochEnd: async (epoch, logs) => {
          console.log(`Epoch ${epoch}: loss = ${logs?.loss}`);
          // TODO: Record + plot
          const ys = model.predict(fullInputsTensor) as Tensor;
          const ysData = await ys.data();

          if (!rxImg) {
            return;
          }

          const ysDataUint8 = new Uint8ClampedArray(width * height * 4);
          for (let i = 0; i < ysData.length / 3; i += 1) {
            const h = ysData[i * 3 + 0] * 0.5 + 0.5;
            const s = ysData[i * 3 + 1] * 0.5 + 0.5;
            const v = ysData[i * 3 + 2] * 0.5 + 0.5;

            const [r, g, b] = HSVtoRGB(h, s, v);

            ysDataUint8[i * 4 + 0] = r;
            ysDataUint8[i * 4 + 1] = g;
            ysDataUint8[i * 4 + 2] = b;
            ysDataUint8[i * 4 + 3] = 255;
          }

          setImageData(rxImg, { data: ysDataUint8, width, height });
        },
      },
      // batchSize: width * height,
      // batchSize: Math.round((width * height) / 16),
      batchSize: 256,
      // shuffle: true,
      // stepsPerEpoch: 32,
    });
  };
</script>

<svelte:head>
  <style lang="css">
    img-comparison-slider {
      visibility: hidden;
    }

    img-comparison-slider [slot='second'] {
      display: none;
    }

    img-comparison-slider.rendered {
      visibility: inherit;
    }

    img-comparison-slider.rendered [slot='second'] {
      display: unset;
    }
  </style>
</svelte:head>

<div class="root">
  <div class="image">
    {#if processState.type === 'notStarted'}
      <Dropzone on:drop={handleFilesSelect} accept={['image/*']} containerClasses="custom-dropzone">
        <button>Choose an image to process</button>

        <p>or</p>
        <p>Drag and drop one here</p>
      </Dropzone>
    {:else if processState.type === 'converting'}
      <p>Converting...</p>
    {:else if processState.type === 'converted'}
      <canvas
        width={processState.width}
        height={processState.height}
        use:renderToCanvas={processState}
        style="max-width: min({processState.width}px, 80vw); margin: auto;"
      />
    {:else if processState.type === 'training'}
      <img-comparison-slider
        style="width: 100%; max-width: min({processState.width}px, 80vw); margin: auto;"
      >
        <img style="width: 100%" slot="first" use:setImageData={processState} alt="before" />
        <img style="width: 100%" slot="second" alt="after" bind:this={rxImg} />
      </img-comparison-slider>
    {:else if processState.type === 'error'}
      <p>{processState.message}</p>
    {/if}
  </div>
  <div class="controls">
    {#if processState.type === 'converted'}
      <button on:click={startTraining}>Train</button>
    {/if}
  </div>
</div>

<style lang="css">
  .root {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 40px);
    margin-top: 20px;
    margin-bottom: 20px;
  }

  .image {
    display: flex;
    flex: 1;
    margin-left: 20px;
    margin-right: 20px;
    height: calc(100vh - 40px - 40px - 140px);
    overflow: auto;
  }

  .controls {
    display: flex;
    flex-direction: row;
    justify-content: center;
    height: 140px;
    min-height: 140px;
  }

  .controls button {
    height: 24px;
  }

  :global(.custom-dropzone) {
    background-color: #121212 !important;
    color: #e8e8e8 !important;
    /* margin-top: 20px; */
    width: max(80%, 300px);
    margin-left: auto;
    margin-right: auto;
  }

  :global(.custom-dropzone button) {
    font-size: 18px;
    background-color: #333;
    outline: none;
    border: 1px solid #888;
    padding: 4px 8px;
  }

  :global(.custom-dropzone p) {
    margin-bottom: 8px;
    margin-top: 16px;
  }
</style>
