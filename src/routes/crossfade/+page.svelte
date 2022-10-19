<script lang="ts">
  import Dropzone from 'svelte-file-dropzone';
  import { parseImageToRGBA } from 'src/processUpload';
  import { getWorkers, type WorkerPoolManager } from 'src/workerPool';
  import { browser } from '$app/environment';

  interface CrossfadeParams {
    threshold: number;
    debug: boolean;
  }

  const buildDefaultCrossfadeParams = (): CrossfadeParams => ({
    threshold: 0.99,
    debug: false,
  });

  type ProcessState =
    | { type: 'notStarted' }
    | {
        type: 'uploaded';
        images: { data: Uint8ClampedArray; dataURL: string }[];
        width: number;
        height: number;
      }
    | { type: 'error'; message: string }
    | {
        type: 'generating';
        images: { data: Uint8ClampedArray; dataURL: string }[];
        width: number;
        height: number;
      }
    | {
        type: 'generated';
        images: { data: Uint8ClampedArray; dataURL: string }[];
        generated: { data: Uint8ClampedArray; dataURL: string };
        width: number;
        height: number;
        params: CrossfadeParams;
      };

  let processState: ProcessState = { type: 'notStarted' };
  let workerPoolP: Promise<WorkerPoolManager<any>> = browser ? getWorkers() : new Promise(() => {});

  $: uploadWarningMsg = (() => {
    if (processState.type === 'uploaded') {
      const uploadedImageCount = processState.images.length;
      if (uploadedImageCount < 4) {
        return 'A minimum of 4 textures is required';
      } else if (uploadedImageCount > 64) {
        return 'A maximum of 64 textures is allowed';
      }
      const uploadedCountIsPerfectSquare = Math.sqrt(uploadedImageCount) % 1 === 0;
      if (!uploadedCountIsPerfectSquare) {
        return `Uploaded ${uploadedImageCount} textures, but the number of images must be a perfect square.`;
      }
    } else {
      return null;
    }
  })();
  $: outputWidth =
    processState.type !== 'notStarted' && processState.type !== 'error'
      ? processState.width * processState.images.length
      : 256;
  $: outputHeight =
    processState.type !== 'notStarted' && processState.type !== 'error'
      ? processState.height * processState.images.length
      : 256;

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

  async function handleFilesSelect(e: any) {
    const { acceptedFiles } = e.detail;
    if (acceptedFiles.length) {
      const parsed: {
        data: Uint8ClampedArray;
        width: number;
        height: number;
      }[] = await Promise.all(acceptedFiles.map((file: File) => parseImageToRGBA(file)));

      // All images must be the same size
      const { width, height } = processState.type === 'uploaded' ? processState : parsed[0];
      if (parsed.some(({ width: w, height: h }) => w !== width || h !== height)) {
        processState = { type: 'error', message: 'All textures must be the same size' };
        return;
      }

      const newImages = parsed.map(({ data, width, height }) => {
        const dataURL = rgbaToDataURL(data, width, height);
        return { data, dataURL };
      });

      processState = {
        type: 'uploaded',
        images:
          processState.type === 'uploaded' ? [...processState.images, ...newImages] : newImages,
        width,
        height,
      };
    }
  }

  const renderToCanvas = (
    canvas: HTMLCanvasElement,
    { data, width, height }: { data: Uint8ClampedArray; width: number; height: number }
  ) => {
    const ctx = canvas.getContext('2d')!;
    const imageData = new ImageData(data, width, height);
    ctx.putImageData(imageData, 0, 0);
  };

  const generate = async () => {
    if (processState.type !== 'uploaded') {
      throw new Error('Unreachable');
    }

    processState = { ...processState, type: 'generating' };
    const workerPool = await workerPoolP;
    const { images, width, height } = processState;

    await workerPool.submitWork(worker => worker.setCrossfadeTextures(images.map(img => img.data)));

    const params = buildDefaultCrossfadeParams();
    const generated = new Uint8ClampedArray(
      (
        await workerPool.submitWork(worker =>
          worker.crossfadeGenerate(width, height, params.threshold, params.debug)
        )
      ).buffer
    );
    processState = {
      type: 'generated',
      images,
      generated: {
        data: generated,
        dataURL: rgbaToDataURL(generated, width * images.length, height * images.length),
      },
      width,
      height,
      params,
    };
  };
</script>

<div class="root">
  <div class="content">
    {#if processState.type === 'notStarted' || processState.type === 'uploaded'}
      <div
        style="width: 100%; height: 100%; display: flex; flex-direction: column; justify-content: center; align-items: center;"
      >
        <h2>Seamless Texture Crossfade Stitcher</h2>
        <Dropzone
          on:drop={handleFilesSelect}
          accept={['image/*']}
          containerClasses="custom-dropzone"
        >
          <p>Drag + drop textures here to use for generation.</p>
          <p>
            All textures must be the same size, should be square, and ideally power-of-2 sizes
            (256x256, 512x512, etc.)
          </p>
          <p>
            The number of uploaded textures must be a perfect square (4, 9, 16, etc.) and ideally
            power of 4 (4, 16, 64)
          </p>
        </Dropzone>
        <div class="images-preview">
          {#if processState.type === 'uploaded'}
            {#each processState.images as image}
              <img src={image.dataURL} alt="" />
            {/each}
          {/if}
        </div>
      </div>
    {:else if processState.type === 'error'}
      <div class="error">
        <h1>Error</h1>
        <p>{processState.message}</p>
        <br /><br />
        <button on:click={() => (processState = { type: 'notStarted' })}>Try again</button>
      </div>
    {:else if processState.type === 'generating'}
      <h2>Generating...</h2>
    {:else if processState.type === 'generated'}
      <canvas
        width={outputWidth}
        height={outputHeight}
        use:renderToCanvas={{
          data: processState.generated.data,
          width: outputWidth,
          height: outputHeight,
        }}
        style="max-width: min({outputWidth}px, 80vw); max-height: min({outputHeight}px, calc(100vh - 200px)); margin: auto;"
      />
    {/if}
  </div>
  <div class="controls">
    {#if processState.type === 'uploaded'}
      {#if uploadWarningMsg}
        <div class="warning">{uploadWarningMsg}</div>
      {:else}
        <div class="buttons-container">
          <button on:click={() => (processState = { type: 'notStarted' })}>Reset</button>
          <button on:click={generate}>Generate</button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style lang="css">
  :global(.custom-dropzone) {
    background-color: #121212 !important;
    color: #e8e8e8 !important;
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

  .root {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 40px);
    margin-top: 8px;
    margin-bottom: 8px;
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

  .images-preview {
    display: flex;
    flex-direction: row;
    gap: 8px;
    overflow-x: auto;
    height: 100px;
    margin-top: 8px;
  }

  .images-preview img {
    /* 100% height, maintain aspect ratio */
    max-height: 100%;
    max-width: 100%;
    object-fit: contain;
  }

  .content h2 {
    margin-top: 2px;
  }

  .error {
    color: red;
  }

  .controls {
    display: flex;
    flex-direction: row;
    justify-content: center;
    height: 140px;
    min-height: 140px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid #888;
  }

  .controls button {
    height: 24px;
  }

  .buttons-container {
    display: flex;
    flex-direction: row;
    gap: 8px;
  }
</style>
