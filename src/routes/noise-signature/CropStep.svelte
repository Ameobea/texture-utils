<script lang="ts" context="module">
  export interface CropRect {
    x: number;
    y: number;
    side: number;
  }
</script>

<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let bmp: ImageBitmap;
  export let exemplarSize: number;
  export let initialRect: CropRect | null = null;

  const dispatch = createEventDispatcher<{ confirm: { rect: CropRect }; cancel: void }>();

  const EXEMPLAR_SIZES = [256, 512, 1024];
  const MAX_DISPLAY_W = 900;
  const MAX_DISPLAY_H = 620;

  const maxSide = Math.min(bmp.width, bmp.height);
  $: minSide = Math.min(exemplarSize, maxSide);

  const centeredMax = (): CropRect => ({
    x: Math.floor((bmp.width - maxSide) / 2),
    y: Math.floor((bmp.height - maxSide) / 2),
    side: maxSide,
  });

  let rect: CropRect = initialRect ?? centeredMax();

  const k = Math.min(MAX_DISPLAY_W / bmp.width, MAX_DISPLAY_H / bmp.height, 1);
  const dispW = Math.round(bmp.width * k);
  const dispH = Math.round(bmp.height * k);

  const clampRect = (r: CropRect): CropRect => {
    const side = Math.min(Math.max(r.side, minSide), maxSide);
    return {
      side,
      x: Math.min(Math.max(r.x, 0), bmp.width - side),
      y: Math.min(Math.max(r.y, 0), bmp.height - side),
    };
  };

  // growing minSide (exemplar size bumped) can invalidate the current rect
  $: if (rect.side < minSide) {
    rect = clampRect(rect);
  }

  const drawImage = (canvas: HTMLCanvasElement) => {
    canvas.width = dispW;
    canvas.height = dispH;
    canvas.getContext('2d')!.drawImage(bmp, 0, 0, dispW, dispH);
  };

  const onMovePointerDown = (e: PointerEvent) => {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const start = { ...rect };
    const [px0, py0] = [e.clientX, e.clientY];
    const onMove = (ev: PointerEvent) => {
      rect = clampRect({
        ...start,
        x: start.x + (ev.clientX - px0) / k,
        y: start.y + (ev.clientY - py0) / k,
      });
    };
    const onUp = () => {
      el.removeEventListener('pointermove', onMove);
      el.removeEventListener('pointerup', onUp);
      el.removeEventListener('pointercancel', onUp);
      rect = { x: Math.round(rect.x), y: Math.round(rect.y), side: Math.round(rect.side) };
    };
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerup', onUp);
    el.addEventListener('pointercancel', onUp);
  };

  // corners resize about the opposite (anchored) corner, square-locked
  const onHandlePointerDown = (corner: string, e: PointerEvent) => {
    e.stopPropagation();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const start = { ...rect };
    const anchor = {
      x: corner.includes('w') ? start.x + start.side : start.x,
      y: corner.includes('n') ? start.y + start.side : start.y,
    };
    const [px0, py0] = [e.clientX, e.clientY];
    const onMove = (ev: PointerEvent) => {
      const dx = (ev.clientX - px0) / k;
      const dy = (ev.clientY - py0) / k;
      const wantX = corner.includes('w') ? start.x + dx : start.x + start.side + dx;
      const wantY = corner.includes('n') ? start.y + dy : start.y + start.side + dy;
      let side = Math.max(
        corner.includes('w') ? anchor.x - wantX : wantX - anchor.x,
        corner.includes('n') ? anchor.y - wantY : wantY - anchor.y
      );
      side = Math.min(
        Math.max(side, minSide),
        corner.includes('w') ? anchor.x : bmp.width - anchor.x,
        corner.includes('n') ? anchor.y : bmp.height - anchor.y
      );
      rect = {
        side,
        x: corner.includes('w') ? anchor.x - side : anchor.x,
        y: corner.includes('n') ? anchor.y - side : anchor.y,
      };
    };
    const onUp = () => {
      el.removeEventListener('pointermove', onMove);
      el.removeEventListener('pointerup', onUp);
      el.removeEventListener('pointercancel', onUp);
      rect = { x: Math.round(rect.x), y: Math.round(rect.y), side: Math.round(rect.side) };
    };
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerup', onUp);
    el.addEventListener('pointercancel', onUp);
  };

  $: ratio = rect.side / exemplarSize;
</script>

<div class="crop-step">
  <div class="crop-controls">
    <span class="szsel" title="Resolution the crop is resampled to for analysis. Larger exemplars keep more of the source's fine detail and give the fit more statistical support, but every re-analysis (and especially the texton fit) gets ~4× slower per doubling.">
      exemplar size:
      {#each EXEMPLAR_SIZES as s}
        <button class:on={exemplarSize === s} on:click={() => (exemplarSize = s)}>
          {s >= 1024 ? `${s / 1024}k` : s}
        </button>
      {/each}
    </span>
    <span class="readout" class:warn={ratio < 1}>
      crop {Math.round(rect.side)}×{Math.round(rect.side)}px → {ratio >= 1
        ? `downsample ×${ratio.toFixed(1)}`
        : `UPSCALE ×${(1 / ratio).toFixed(1)}`} → {exemplarSize}²
    </span>
    <button on:click={() => (rect = centeredMax())}>reset</button>
    <button class="primary" on:click={() => dispatch('confirm', { rect })}>
      extract &amp; analyze
    </button>
    <button on:click={() => dispatch('cancel')}>new image</button>
  </div>
  <p class="hint">
    Drag the box to move it, corners to resize (square-locked). A tighter crop preserves
    high-frequency detail that full-frame downsampling would destroy.
  </p>
  <div class="stage" style="width: {dispW}px; height: {dispH}px">
    <canvas use:drawImage />
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="crop-box"
      style="left: {rect.x * k}px; top: {rect.y * k}px; width: {rect.side * k}px; height: {rect.side * k}px"
      on:pointerdown={onMovePointerDown}
    >
      {#each ['nw', 'ne', 'sw', 'se'] as c}
        <div class="handle {c}" on:pointerdown={e => onHandlePointerDown(c, e)} />
      {/each}
    </div>
  </div>
</div>

<style>
  .crop-step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .crop-controls {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    font-size: 13px;
  }

  .szsel button {
    margin-left: 2px;
  }

  .szsel button.on {
    background: #4a6;
    color: #111;
  }

  .readout {
    color: #aaa;
    font-variant-numeric: tabular-nums;
  }

  .readout.warn {
    color: #e9a;
  }

  .primary {
    background: #4a6;
    color: #111;
    font-weight: bold;
  }

  .hint {
    font-size: 12px;
    color: #999;
    margin: 0;
  }

  .stage {
    position: relative;
    overflow: hidden;
    border: 1px solid #333;
  }

  .stage canvas {
    display: block;
  }

  .crop-box {
    position: absolute;
    border: 1px solid #4fc;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.55);
    cursor: move;
    touch-action: none;
    box-sizing: border-box;
  }

  .handle {
    position: absolute;
    width: 12px;
    height: 12px;
    background: #4fc;
    touch-action: none;
  }

  .handle.nw {
    left: -6px;
    top: -6px;
    cursor: nwse-resize;
  }

  .handle.ne {
    right: -6px;
    top: -6px;
    cursor: nesw-resize;
  }

  .handle.sw {
    left: -6px;
    bottom: -6px;
    cursor: nesw-resize;
  }

  .handle.se {
    right: -6px;
    bottom: -6px;
    cursor: nwse-resize;
  }
</style>
