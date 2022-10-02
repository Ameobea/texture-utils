<script lang="ts">
  import Dropzone from 'svelte-file-dropzone';
  import { parseImageToRGBA } from 'src/processUpload';
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import { EMBED_API_URL } from 'src/conf';
  import { getWorkers, WorkerPoolManager } from 'src/workerPool';

  type ProcessState =
    | { type: 'notStarted' }
    | { type: 'uploaded'; data: Uint8ClampedArray; width: number; height: number }
    | { type: 'error'; message: string }
    | {
        type: 'training';
        data: Uint8ClampedArray;
        width: number;
        height: number;
        palette: string[] | null;
        sortedPalette: string[] | null;
      }
    | {
        type: 'trained';
        data: Uint8ClampedArray;
        width: number;
        height: number;
        palette: string[];
        sortedPalette: string[];
        encoded: Uint8Array;
        decoded: Uint8ClampedArray;
        loss: number;
      };

  let processState: ProcessState = { type: 'notStarted' };
  let workerPoolP: Promise<WorkerPoolManager<any>> = browser ? getWorkers() : new Promise(() => {});

  onMount(async () => {
    const script = document.createElement('script');
    script.defer = true;
    script.src = '/imgComparisonSlider.js';
    document.body.appendChild(script);
  });

  async function handleFilesSelect(e: any) {
    const { acceptedFiles } = e.detail;
    if (acceptedFiles.length) {
      const { data, width, height } = await parseImageToRGBA(acceptedFiles[0]);
      processState = { type: 'uploaded', data, width, height };
    }
  }

  const renderToCanvas = (
    canvas: HTMLCanvasElement,
    { data, width, height }: Extract<ProcessState, { type: 'uploaded' | 'training' | 'trained' }>
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

  let txImg: HTMLImageElement | null = null;
  let rxImg: HTMLImageElement | null = null;
  let displayMode: 'original' | 'encoded' = 'original';

  $: if (
    txImg &&
    (processState.type === 'uploaded' ||
      processState.type === 'training' ||
      processState.type === 'trained')
  ) {
    if (displayMode === 'original' || processState.type !== 'trained') {
      setImageData(txImg, processState);
    } else {
      // Create grayscale image from the single channel
      const { encoded, width, height } = processState;
      const grayscaleImageData = new Uint8ClampedArray(encoded.length * 4);
      for (let i = 0; i < encoded.length; i++) {
        grayscaleImageData[i * 4 + 0] = encoded[i];
        grayscaleImageData[i * 4 + 1] = encoded[i];
        grayscaleImageData[i * 4 + 2] = encoded[i];
        grayscaleImageData[i * 4 + 3] = 255;
      }
      setImageData(txImg, { data: grayscaleImageData, width, height });
    }
  }

  const generate = async () => {
    if (processState.type !== 'uploaded') {
      throw new Error('Invalid state: ' + processState.type);
    }

    const workerPool = await workerPoolP;

    const { data, width, height } = processState;
    processState = { type: 'training', data, width, height, palette: null, sortedPalette: null };

    // TODO: configurable
    const numColors = 256;

    const palettes: { palette: Uint8Array; score: number }[] = await Promise.all(
      new Array(8)
        .fill(null)
        .map(() =>
          workerPool.submitWork(remote => remote.genPalette(new Uint8Array(data.buffer), numColors))
        )
    );
    // use palette with lowest score
    const { palette } = palettes.reduce((a, b) => (a.score < b.score ? a : b));

    const allColors: [number, number, number][] = [];

    processState = {
      type: 'training',
      data,
      width,
      height,
      palette: new Array(palette.length / 4).fill(null).map((_, i) => {
        const r = palette[i * 4 + 0];
        const g = palette[i * 4 + 1];
        const b = palette[i * 4 + 2];
        allColors.push([r, g, b]);
        return `rgb(${r},${g},${b})`;
      }),
      sortedPalette: null,
    };

    // let colorPositions: number[];
    // try {
    //   colorPositions = await fetch(EMBED_API_URL, {
    //     method: 'POST',
    //     body: JSON.stringify(allColors),
    //     headers: { 'Content-Type': 'application/json' },
    //   }).then(async res => {
    //     if (!res.ok) {
    //       throw await res.text();
    //     }
    //     return (await res.json()) as number[];
    //   });
    // } catch (err) {
    //   console.error(err);
    //   alert(`Error embedding colors: ${err}`);
    //   processState = { type: 'error', message: `${err}` };
    //   return;
    // }

    const sortedColors = [...allColors];
    // .sort((a, b) => {
    //   const aIx = colorPositions[allColors.indexOf(a)];
    //   const bIx = colorPositions[allColors.indexOf(b)];
    //   return aIx - bIx;
    // });
    const sortedPalette = sortedColors.map(([r, g, b]) => `rgb(${r},${g},${b})`);
    processState = {
      type: 'training',
      data,
      width,
      height,
      palette: processState.palette,
      sortedPalette,
    };

    const sortedPaletteData = new Uint8Array(sortedColors.length * 4);
    for (const [i, [r, g, b]] of sortedColors.entries()) {
      sortedPaletteData[i * 4 + 0] = r;
      sortedPaletteData[i * 4 + 1] = g;
      sortedPaletteData[i * 4 + 2] = b;
      sortedPaletteData[i * 4 + 3] = 255;
    }

    // Split `data` into `navigator.hardwareConcurrency - 2` chunks so that it can be encoded in parallel
    const numChunks = Math.max(1, (navigator.hardwareConcurrency || 0) - 2);
    let chunkSize = Math.ceil(data.length / numChunks);
    // ensure chunk size is divisible by 4
    chunkSize = Math.floor(chunkSize / 4) * 4;
    const chunks = new Array(numChunks).fill(null).map((_, i) => {
      const start = i * chunkSize;
      const end = Math.min(start + chunkSize, data.length);
      return data.subarray(start, end);
    });

    const encodedChunks: Uint8Array[] = await Promise.all(
      chunks.map(chunk =>
        workerPool.submitWork(remote =>
          remote.encodeImage(sortedPaletteData, new Uint8Array(chunk))
        )
      )
    );
    const encodedImage = new Uint8Array(width * height);
    // stitch chunks back together
    for (const [i, chunk] of encodedChunks.entries()) {
      encodedImage.set(chunk, i * (chunkSize / 4));
    }

    const decodedImage = await workerPool.submitWork(remote =>
      remote.decodeImage(sortedPaletteData, encodedImage)
    );
    const decodedImageData = new Uint8ClampedArray(decodedImage.buffer);
    if (!rxImg) {
      throw new Error('rxImg not set');
    }
    setImageData(rxImg, { data: decodedImageData, width, height });

    const loss: number = await workerPool.submitWork(remote =>
      remote.computeLoss(new Uint8Array(data.buffer), decodedImage)
    );

    processState = {
      type: 'trained',
      data,
      width,
      height,
      palette: processState.palette!,
      sortedPalette,
      encoded: encodedImage,
      decoded: decodedImageData,
      loss,
    };
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
    {:else if processState.type === 'uploaded'}
      <canvas
        width={processState.width}
        height={processState.height}
        use:renderToCanvas={processState}
        style="max-width: min({processState.width}px, 80vw); margin: auto;"
      />
    {:else if processState.type === 'training' || processState.type === 'trained'}
      <img-comparison-slider
        style="width: 100%; max-width: min({processState.width}px, 80vw); margin: auto;"
      >
        <img
          style="width: 100%"
          slot="first"
          use:setImageData={processState}
          alt="before"
          bind:this={txImg}
        />
        <img style="width: 100%" slot="second" alt="after" bind:this={rxImg} />
      </img-comparison-slider>
    {:else if processState.type === 'error'}
      <p>{processState.message}</p>
    {/if}
  </div>
  <div class="controls">
    {#if processState.type === 'uploaded'}
      <button on:click={generate}>Generate</button>
    {/if}
    <div class="palettes-container">
      {#if processState.type === 'training' || (processState.type === 'trained' && processState.palette)}
        <div class="palette">
          {#each processState.palette ?? [] as color}
            <div class="palette-swatch" style="background-color: {color}" />
          {/each}
        </div>
      {/if}
      {#if processState.type === 'training' || (processState.type === 'trained' && processState.sortedPalette)}
        <div class="palette">
          {#each processState.sortedPalette ?? [] as color}
            <div class="palette-swatch" style="background-color: {color}" />
          {/each}
        </div>
      {/if}
    </div>
    {#if processState.type === 'trained'}
      <div class="display-mode-wrapper">
        <h3>Compare To</h3>
        <div class="display-mode">
          <label>
            <input type="radio" name="display-mode" value="original" bind:group={displayMode} />
            Original
          </label>
          <label>
            <input type="radio" name="display-mode" value="encoded" bind:group={displayMode} />
            Encoded
          </label>
        </div>
      </div>
      <div class="loss">
        <h3>Loss</h3>
        <p>{processState.loss}</p>
      </div>
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

  .palettes-container {
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .palette {
    display: flex;
    flex-direction: row;
    width: 100%;
    max-width: 800px;
  }

  .palette-swatch {
    width: 24px;
    height: 24px;
    border-right: 1px solid black;
  }

  .display-mode-wrapper {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
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
