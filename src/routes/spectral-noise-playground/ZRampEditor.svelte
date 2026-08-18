<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  import { drawRampBar, sampleRamp, linearToSrgb, type Ease, type RampSpace, type RampStop } from './rampSample';

  export let stops: RampStop[];
  export let space: RampSpace;

  const dispatch = createEventDispatcher<{ change: void }>();

  let selectedIx: number | null = null;
  $: selStop = selectedIx === null ? null : (stops[selectedIx] ?? null);

  const canvasFor = (canvas: HTMLCanvasElement, params: { stops: RampStop[]; space: RampSpace }) => {
    drawRampBar(canvas, params.stops, params.space);
    return {
      update(next: { stops: RampStop[]; space: RampSpace }) {
        drawRampBar(canvas, next.stops, next.space);
      },
    };
  };

  /** Keeps stops sorted (stable) so the bar and snippet agree on order. */
  const sortStops = (keepIx?: number): number => {
    const tagged = stops.map((s, i) => ({ s, i }));
    tagged.sort((a, b) => a.s.pos - b.s.pos || a.i - b.i);
    stops = tagged.map(t => t.s);
    return keepIx === undefined ? -1 : tagged.findIndex(t => t.i === keepIx);
  };

  const commit = (keepIx?: number): number => {
    const ix = sortStops(keepIx);
    dispatch('change');
    return ix;
  };

  const extent = (): [number, number] => [
    stops[0]?.pos ?? 0,
    stops[stops.length - 1]?.pos ?? 1,
  ];

  const frac = (pos: number): number => {
    const [lo, hi] = extent();
    return hi > lo ? (pos - lo) / (hi - lo) : 0;
  };

  const linToHex = (lin: [number, number, number]): string => {
    const enc = linearToSrgb(lin);
    const h = (c: number) =>
      Math.round(Math.min(1, Math.max(0, c)) * 255)
        .toString(16)
        .padStart(2, '0');
    return `#${h(enc[0])}${h(enc[1])}${h(enc[2])}`;
  };

  const onStopColor = (ix: number, hex: string) => {
    if (hex === stops[ix].hex) return;
    stops[ix].hex = hex;
    stops = stops;
    dispatch('change');
  };

  const onStopPos = (ix: number, raw: string) => {
    const v = parseFloat(raw);
    if (!Number.isFinite(v)) return;
    stops[ix].pos = v;
    const newIx = commit(ix);
    if (newIx >= 0) selectedIx = newIx;
  };

  const onStopEase = (ix: number, e: Event) => {
    stops[ix].ease = (e.currentTarget as HTMLSelectElement).value as Ease;
    stops = stops;
    dispatch('change');
  };

  const onSpaceChange = (e: Event) => {
    space = (e.currentTarget as HTMLSelectElement).value as RampSpace;
    dispatch('change');
  };

  const addStop = (e: MouseEvent) => {
    const bar = e.currentTarget as HTMLElement;
    const r = bar.getBoundingClientRect();
    const [lo, hi] = extent();
    const pos = lo + (hi - lo) * Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const hex = linToHex(sampleRamp(stops, space, pos));
    stops = [...stops, { pos, hex, ease: 'linear' }];
    selectedIx = commit(stops.length - 1);
  };

  const removeStop = (ix: number) => {
    if (stops.length <= 2) return;
    stops = stops.filter((_, i) => i !== ix);
    commit();
    selectedIx = null;
  };

  // Interior stops drag along the bar; the two extremes pin the extent (edit their pos
  // numerically instead) so the bar's mapping stays stable mid-drag.
  const onMarkerPointerDown = (ix: number, e: PointerEvent) => {
    selectedIx = ix;
    if (ix === 0 || ix === stops.length - 1) return;
    const marker = e.currentTarget as HTMLElement;
    const bar = marker.parentElement!;
    marker.setPointerCapture(e.pointerId);
    const [lo, hi] = extent();
    const onMove = (ev: PointerEvent) => {
      const r = bar.getBoundingClientRect();
      const pos = lo + (hi - lo) * Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width));
      stops[ix].pos = Math.min(hi, Math.max(lo, pos));
      const newIx = commit(ix);
      if (newIx >= 0) {
        selectedIx = newIx;
        ix = newIx;
      }
    };
    const onUp = () => {
      marker.removeEventListener('pointermove', onMove);
      marker.removeEventListener('pointerup', onUp);
      marker.removeEventListener('pointercancel', onUp);
    };
    marker.addEventListener('pointermove', onMove);
    marker.addEventListener('pointerup', onUp);
    marker.addEventListener('pointercancel', onUp);
  };
</script>

<div class="ramp">
  <div class="ramp-head">
    <span
      class="ramp-label"
      title="Maps the Gaussian field's z-scores to color. Stop positions are z-scores (standard deviations from the mean): ±2.5 covers ~99% of pixels. End stops pin the range — edit their positions numerically."
    >
      color ramp (z-score positions)
    </span>
    <select
      class="space-select"
      title={'Color space stops are interpolated in. oklab: perceptually even (default). oklch: hue as shorter-arc angle, stays saturated. linear: raw RGB / light mixing. srgb: legacy gamma-space.'}
      value={space}
      on:change={onSpaceChange}
    >
      {#each ['oklab', 'oklch', 'linear', 'srgb'] as s (s)}
        <option value={s}>{s}</option>
      {/each}
    </select>
  </div>
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="bar-wrap" on:dblclick={addStop} title="double-click to add a stop">
    <canvas class="bar" width={340} height={20} use:canvasFor={{ stops, space }} />
    {#each stops as s, ix (ix)}
      <div
        class="marker"
        class:selected={selectedIx === ix}
        class:pinned={ix === 0 || ix === stops.length - 1}
        style="left: {frac(s.pos) * 100}%; background: {s.hex}"
        on:pointerdown={e => onMarkerPointerDown(ix, e)}
      />
    {/each}
  </div>
  {#if selStop}
    <div class="stop-editor">
      <input
        class="num"
        type="number"
        step="any"
        value={selStop.pos}
        title="stop position (z-score)"
        on:change={e => onStopPos(selectedIx ?? 0, e.currentTarget.value)}
      />
      <input
        class="swatch"
        type="color"
        value={selStop.hex}
        title="stop color (sRGB)"
        on:input={e => onStopColor(selectedIx ?? 0, e.currentTarget.value)}
        on:change={e => onStopColor(selectedIx ?? 0, e.currentTarget.value)}
      />
      <select
        class="ease-select"
        value={selStop.ease}
        title="easing toward the next stop"
        on:change={e => onStopEase(selectedIx ?? 0, e)}
      >
        {#each ['linear', 'smooth', 'smoother', 'step'] as ez (ez)}
          <option value={ez}>{ez}</option>
        {/each}
      </select>
      <button
        class="del"
        disabled={stops.length <= 2}
        title="remove stop"
        on:click={() => removeStop(selectedIx ?? 0)}
      >
        ×
      </button>
    </div>
  {/if}
</div>

<style>
  .ramp {
    background: #141414;
    border: 1px solid #2e2e2e;
    padding: 6px 10px 8px 10px;
    width: 372px;
    box-sizing: border-box;
    font-size: 12px;
    color: #ddd;
  }
  .ramp-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 0;
  }
  .ramp-label {
    color: #aaa;
  }
  .space-select,
  .ease-select {
    background: #1a1a1a;
    color: #ddd;
    border: 1px solid #444;
    font-size: 11px;
  }
  .bar-wrap {
    position: relative;
    height: 30px;
    margin: 2px 4px 6px 4px;
  }
  .bar {
    width: 100%;
    height: 20px;
    display: block;
    border: 1px solid #444;
    box-sizing: border-box;
  }
  .marker {
    position: absolute;
    top: 16px;
    width: 9px;
    height: 12px;
    transform: translateX(-50%);
    border: 1px solid #ccc;
    cursor: grab;
    touch-action: none;
  }
  .marker.pinned {
    cursor: pointer;
  }
  .marker.selected {
    border-color: #0ff;
    outline: 1px solid #0ff;
  }
  .stop-editor {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 4px 4px 4px;
  }
  .num {
    width: 62px;
    background: #1a1a1a;
    color: #ddd;
    border: 1px solid #444;
    font-size: 11px;
    padding: 1px 3px;
  }
  .swatch {
    width: 30px;
    height: 20px;
    padding: 0;
    border: 1px solid #444;
    background: none;
  }
  .del {
    background: #333;
    border: 1px solid #555;
    color: #f0f0f0;
    cursor: pointer;
    font-size: 12px;
    padding: 0 6px;
  }
  .del:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
