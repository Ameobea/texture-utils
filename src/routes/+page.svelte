<script lang="ts">
  import Dropzone from 'svelte-file-dropzone';
  import { parseImageToRGBA } from 'src/processUpload';
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
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
        sortedPaletteData: Uint8Array;
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

  const buildGrayscaleImageFromEncoded = (encoded: Uint8Array) => {
    const grayscaleImageData = new Uint8ClampedArray(encoded.length * 4);
    for (let i = 0; i < encoded.length; i++) {
      grayscaleImageData[i * 4 + 0] = encoded[i];
      grayscaleImageData[i * 4 + 1] = encoded[i];
      grayscaleImageData[i * 4 + 2] = encoded[i];
      grayscaleImageData[i * 4 + 3] = 255;
    }
    return grayscaleImageData;
  };

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
      const grayscaleImageData = buildGrayscaleImageFromEncoded(encoded);
      setImageData(txImg, { data: grayscaleImageData, width, height });
    }
  }

  const maybeResizeImage = (
    data: Uint8ClampedArray,
    width: number,
    height: number
  ): Uint8ClampedArray => {
    if (width < 412 && height < 412) {
      return data;
    }

    const scale = Math.min(412 / width, 412 / height);
    const newWidth = Math.floor(width * scale);
    const newHeight = Math.floor(height * scale);

    const largeCanvas = document.createElement('canvas');
    largeCanvas.width = width;
    largeCanvas.height = height;
    const largeCtx = largeCanvas.getContext('2d')!;
    const largeImageData = new ImageData(data, width, height);
    largeCtx.putImageData(largeImageData, 0, 0);

    const smallCanvas = document.createElement('canvas');
    smallCanvas.width = newWidth;
    smallCanvas.height = newHeight;
    const smallCtx = smallCanvas.getContext('2d')!;
    smallCtx.drawImage(largeCanvas, 0, 0, newWidth, newHeight);

    const smallImageData = smallCtx.getImageData(0, 0, newWidth, newHeight);
    return smallImageData.data;
  };

  const generate = async () => {
    if (processState.type !== 'uploaded') {
      throw new Error('Invalid state: ' + processState.type);
    }

    const workerPool = await workerPoolP;

    const { data, width, height } = processState;
    processState = { type: 'training', data, width, height, palette: null, sortedPalette: null };

    // TODO: configurable?
    const numColors = 256;

    // Downsample the image before generating palettes so things run reasonably fast
    const resizedImageData = maybeResizeImage(data, width, height);

    const palettes: { palette: Uint8Array; score: number }[] = await Promise.all(
      new Array(Math.max((navigator.hardwareConcurrency || 4) - 2, 2))
        .fill(null)
        .map(() =>
          workerPool.submitWork(remote =>
            remote.genPalette(new Uint8Array(resizedImageData.buffer), numColors)
          )
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
      sortedPaletteData,
      encoded: encodedImage,
      decoded: decodedImageData,
      loss,
    };
  };

  const download = async () => {
    if (processState.type !== 'trained') {
      throw new Error('Invalid state: ' + processState.type);
    }

    const { encoded, width, height, sortedPaletteData } = processState;
    const grayscaleImageData = buildGrayscaleImageFromEncoded(encoded);
    const grayscaleImage = new ImageData(grayscaleImageData, width, height);

    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d')!;
    ctx.putImageData(grayscaleImage, 0, 0);

    const a = document.createElement('a');
    a.href = canvas.toDataURL('image/png');
    a.download = 'encoded.png';
    a.click();

    const fullLUT: Uint8Array = await (
      await workerPoolP
    ).submitWork(remote => remote.buildFullLUT(sortedPaletteData));

    const formatted = JSON.stringify(Array.from(fullLUT));
    console.log(formatted);

    if (navigator.clipboard) {
      await navigator.clipboard.writeText(formatted);
    }
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
  <div class="content">
    {#if processState.type === 'notStarted'}
      <div
        style="width: 100%; height: 100%; display: flex; flex-direction: column; justify-content: center; align-items: center;"
      >
        <h2>Convert Texture to Single Channel</h2>
        <Dropzone
          on:drop={handleFilesSelect}
          accept={['image/*']}
          containerClasses="custom-dropzone"
        >
          <button>Choose an image to process</button>

          <p>or</p>
          <p>Drag and drop one here</p>
        </Dropzone>
      </div>
    {:else if processState.type === 'uploaded'}
      <canvas
        width={processState.width}
        height={processState.height}
        use:renderToCanvas={processState}
        style="max-width: min({processState.width}px, 80vw); margin: auto;"
      />
    {:else if processState.type === 'training' || processState.type === 'trained'}
      <div class="content-container">
        <div class="image">
          <img-comparison-slider
            style="width: calc(min(100%, {processState.width}px)); height: auto; aspect-ratio: {processState.width} / {processState.height};"
          >
            <img
              height={processState.height}
              width={processState.width}
              style="width: 100%; height: auto"
              slot="first"
              use:setImageData={processState}
              alt="before"
              bind:this={txImg}
            />
            <img
              height={processState.height}
              width={processState.width}
              style="width: 100%; height: auto"
              slot="second"
              alt="after"
              bind:this={rxImg}
            />
          </img-comparison-slider>
        </div>
        {#if processState.type === 'training'}
          <div class="loading" style="display: flex; flex: 0">
            <h3>Generating Palettes with K-Means</h3>
            <p>This can take up to a minute</p>
          </div>
        {/if}
      </div>
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
        <br /><i>Anything 1 or less is very good</i>
      </div>
      <button on:click={download}>Download</button>
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

  .content-container {
    display: flex;
    flex-direction: column;
    width: 100%;
    justify-content: center;
    align-items: center;
  }

  .content {
    display: flex;
    flex: 1;
    margin-left: 20px;
    margin-right: 20px;
    height: calc(100vh - 40px - 40px - 140px);
  }

  .image {
    overflow: auto;
    display: flex;
    flex: 1;
    width: 100%;
    justify-content: center;
    align-items: center;
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

  .display-mode-wrapper,
  .loss {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    margin-left: 20px;
  }

  .loss h3 {
    margin-bottom: 0;
  }

  .loading {
    /* center horizontally and vertically */
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    height: 100%;
    width: 100%;
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
