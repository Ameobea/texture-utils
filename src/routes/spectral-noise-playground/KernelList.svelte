<script lang="ts" context="module">
  export interface KernelUI {
    /** carrier frequency, cycles/px */
    freq: number;
    /** wave-vector direction, degrees [0, 180) */
    orientDeg: number;
    /** log10 spectral widths along the lobe's two axes */
    sig1: number;
    sig2: number;
    /** lobe-ellipse orientation, degrees [0, 180] */
    angleDeg: number;
    /** log10 energy relative to the whole band spectrum */
    energy: number;
  }

  export const MAX_UI_KERNELS = 16;

  const FREQ_MIN = 1 / 256;
  const FREQ_MAX = 0.5;

  export const kernelF0 = (k: KernelUI): [number, number] => {
    const th = (k.orientDeg * Math.PI) / 180;
    return [k.freq * Math.sin(th), k.freq * Math.cos(th)];
  };
</script>

<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let kernels: KernelUI[];

  const dispatch = createEventDispatcher<{ change: void }>();

  // freq sliders work in log space: u ∈ [0,1] → FREQ_MIN · (FREQ_MAX/FREQ_MIN)^u
  const freqToU = (f: number) => Math.log(f / FREQ_MIN) / Math.log(FREQ_MAX / FREQ_MIN);
  const uToFreq = (u: number) => FREQ_MIN * Math.pow(FREQ_MAX / FREQ_MIN, u);

  const set = (ix: number, key: keyof KernelUI, raw: string, map?: (v: number) => number) => {
    const v = parseFloat(raw);
    if (!Number.isFinite(v)) return;
    kernels[ix][key] = map ? map(v) : v;
    kernels = kernels;
    dispatch('change');
  };

  const addKernel = () => {
    kernels = [
      ...kernels,
      { freq: 0.1, orientDeg: 45, sig1: -1.6, sig2: -2.2, angleDeg: 45, energy: -0.5 },
    ];
    dispatch('change');
  };

  const removeKernel = (ix: number) => {
    kernels = kernels.filter((_, i) => i !== ix);
    dispatch('change');
  };
</script>

<div class="kernels">
  <div class="kernels-head">
    <span
      class="section-label"
      title="Spectral peaks: concentrated Gaussian lobes added on top of the band spectrum. Each one puts a narrow oriented wave into the texture — ribs, grain, weave. The band grid can't represent these (its cells are too coarse); peaks are what carry regular, oriented structure."
    >
      spectral peaks
    </span>
    <button
      class="add"
      disabled={kernels.length >= MAX_UI_KERNELS}
      on:click={addKernel}
    >
      + add peak
    </button>
  </div>

  {#each kernels as k, ix (ix)}
    <div class="kernel">
      <div class="kernel-title">
        <span>peak {ix}</span>
        <button class="del" title="remove peak" on:click={() => removeKernel(ix)}>×</button>
      </div>
      <label
        title="Carrier frequency of the wave, in cycles/px (log slider). Sets the rib/grain spacing: wavelength = 1/frequency."
      >
        <span class="k">frequency</span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.002"
          value={freqToU(k.freq)}
          on:input={e => set(ix, 'freq', e.currentTarget.value, uToFreq)}
        />
        <span class="v">{k.freq.toFixed(4)} c/px · λ {(1 / k.freq).toFixed(1)}px</span>
      </label>
      <label
        title="Direction of the wave vector, degrees. The wave varies along this direction: 0° = varies along x (vertical ribs), 90° = varies along y (horizontal ribs)."
      >
        <span class="k">orientation</span>
        <input
          type="range"
          min="0"
          max="180"
          step="0.5"
          value={k.orientDeg}
          on:input={e => set(ix, 'orientDeg', e.currentTarget.value)}
        />
        <span class="v">{k.orientDeg.toFixed(1)}°</span>
      </label>
      <label
        title="Spectral width along the lobe's first axis (log10 cycles/px). Narrow (−3) = a nearly pure sine: long-range coherent waves. Broad (−0.5) = wide frequency spread: short, localized wiggles."
      >
        <span class="k">width σ1</span>
        <input
          type="range"
          min="-3"
          max="-0.5"
          step="0.01"
          value={k.sig1}
          on:input={e => set(ix, 'sig1', e.currentTarget.value)}
        />
        <span class="v">{k.sig1.toFixed(2)} → {Math.pow(10, k.sig1).toFixed(4)} c/px</span>
      </label>
      <label
        title="Spectral width along the lobe's second axis (log10 cycles/px). Making σ1 ≠ σ2 elongates the lobe: tight in one direction (coherent) and loose in the other (wandering)."
      >
        <span class="k">width σ2</span>
        <input
          type="range"
          min="-3"
          max="-0.5"
          step="0.01"
          value={k.sig2}
          on:input={e => set(ix, 'sig2', e.currentTarget.value)}
        />
        <span class="v">{k.sig2.toFixed(2)} → {Math.pow(10, k.sig2).toFixed(4)} c/px</span>
      </label>
      <label
        title="Orientation of the lobe ellipse's σ1 axis in the frequency plane, degrees. Only matters when σ1 ≠ σ2. σ1 along the carrier direction controls wavelength jitter; σ1 across it controls how much the wave's direction wanders."
      >
        <span class="k">lobe angle</span>
        <input
          type="range"
          min="0"
          max="180"
          step="0.5"
          value={k.angleDeg}
          on:input={e => set(ix, 'angleDeg', e.currentTarget.value)}
        />
        <span class="v">{k.angleDeg.toFixed(1)}°</span>
      </label>
      <label
        title="How strong the peak is: log10 of its energy relative to the ENTIRE band spectrum's energy. 0 = as much energy as all the bands combined; −2 = 1% of it; positive values dominate the texture."
      >
        <span class="k">energy</span>
        <input
          type="range"
          min="-4"
          max="2"
          step="0.02"
          value={k.energy}
          on:input={e => set(ix, 'energy', e.currentTarget.value)}
        />
        <span class="v">{k.energy.toFixed(2)} → ×{Math.pow(10, k.energy).toPrecision(2)}</span>
      </label>
    </div>
  {/each}
  {#if kernels.length === 0}
    <p class="empty">
      No peaks. The band grid alone makes fully stochastic textures; add a peak to introduce an
      oriented wave (fabric ribs, wood grain).
    </p>
  {/if}
</div>

<style>
  .kernels {
    background: #141414;
    border: 1px solid #2e2e2e;
    padding: 6px 10px 8px 10px;
    box-sizing: border-box;
    font-size: 12px;
    color: #ddd;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .kernels-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 0;
  }
  .section-label {
    color: #aaa;
  }
  .add {
    background: #222;
    border: 1px solid #444;
    color: #ccc;
    font-size: 11px;
    cursor: pointer;
    padding: 1px 8px;
  }
  .add:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .kernel {
    border: 1px solid #2a2a2a;
    padding: 4px 8px 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .kernel-title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: #9ac;
    font-size: 11px;
  }
  .kernel label {
    display: grid;
    grid-template-columns: 72px 150px 1fr;
    align-items: center;
    gap: 8px;
  }
  .kernel .k {
    color: #999;
    font-size: 11px;
    cursor: help;
  }
  .kernel .v {
    font-size: 11px;
    color: #bbb;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .kernel input[type='range'] {
    width: 150px;
    margin: 0;
  }
  .del {
    background: #333;
    border: 1px solid #555;
    color: #f0f0f0;
    cursor: pointer;
    font-size: 12px;
    padding: 0 6px;
  }
  .empty {
    color: #777;
    font-size: 11px;
    margin: 2px 0;
  }
</style>
