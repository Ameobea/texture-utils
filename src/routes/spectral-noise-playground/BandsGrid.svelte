<script lang="ts" context="module">
  export const KR = 8;
  export const KA = 4;

  // Radial band centers are log-spaced over cycles/px [1/256, 0.5] — exactly one octave
  // apart, so each column is a wavelength halving: 256px … 2px.
  const WAVELENGTHS = [256, 128, 64, 32, 16, 8, 4, 2];
  const SECTORS = [0, 45, 90, 135];

  const PRESETS: { name: string; tip: string; colGain: (k: number) => number }[] = [
    { name: 'white', tip: 'Equal power at every frequency — harsh, fine-grained static', colGain: () => 0 },
    {
      name: 'pink (1/f)',
      tip: 'Power falls 3dB/octave (−0.69 nats per column) — the balanced, natural-looking default',
      colGain: k => -0.693 * k,
    },
    {
      name: 'brown (1/f²)',
      tip: 'Power falls 6dB/octave — smooth large blobs, little fine detail',
      colGain: k => -1.386 * k,
    },
    {
      name: 'blue (f)',
      tip: 'Power rises with frequency — almost pure fine grain, no large-scale structure',
      colGain: k => -0.693 * (KR - 1 - k),
    },
  ];
</script>

<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  /** bands[radialBand][angularSector], log-gain in [-14, 0] nats */
  export let bands: number[][];
  export let isotropic: boolean;

  const dispatch = createEventDispatcher<{ change: void }>();

  const onCell = (i: number, j: number, raw: string) => {
    const v = parseFloat(raw);
    if (!Number.isFinite(v)) return;
    if (isotropic) {
      for (let jj = 0; jj < KA; jj += 1) bands[i][jj] = v;
    } else {
      bands[i][j] = v;
    }
    bands = bands;
    dispatch('change');
  };

  const applyPreset = (colGain: (k: number) => number) => {
    bands = Array.from({ length: KR }, (_, i) => Array.from({ length: KA }, () => colGain(i)));
    dispatch('change');
  };
</script>

<div class="bands">
  <div class="bands-head">
    <span
      class="section-label"
      title="The base spectrum: how much power the noise has at each frequency band × direction. Each cell is a log-power gain in nats relative to the loudest cell (0 = loudest, −14 ≈ silent). The synthesized texture is random noise shaped to exactly this spectrum."
    >
      spectrum bands
    </span>
    <label
      class="iso"
      title="Links the 4 direction sectors of each column so the spectrum stays isotropic (direction-free). Uncheck to boost/cut specific orientations."
    >
      <input type="checkbox" bind:checked={isotropic} />
      isotropic
    </label>
    <span class="presets">
      {#each PRESETS as p (p.name)}
        <button title={p.tip} on:click={() => applyPreset(p.colGain)}>{p.name}</button>
      {/each}
    </span>
  </div>

  <div class="grid">
    <span class="corner" />
    {#each WAVELENGTHS as wl, i (i)}
      <span
        class="col-head"
        title="Radial band {i}: features ~{wl}px across ({(1 / wl).toFixed(4)} cycles/px). Bands are log-spaced one octave apart; frequencies between band centers interpolate."
      >
        {wl}px
      </span>
    {/each}
    {#each SECTORS as deg, j (j)}
      <span
        class="row-head"
        title="Angular sector centered at {deg}°: the direction the noise varies in. 0° varies along x (vertical streaks), 90° varies along y (horizontal streaks). Frequencies between sector centers interpolate, and 180° wraps to 0°."
      >
        {deg}°
      </span>
      {#each WAVELENGTHS as _wl, i (i)}
        <span class="cell">
          <input
            type="range"
            min="-14"
            max="0"
            step="0.1"
            value={bands[i][j]}
            title="band {i} ({WAVELENGTHS[i]}px) × sector {deg}°: {bands[i][j].toFixed(1)} nats"
            on:input={e => onCell(i, j, e.currentTarget.value)}
          />
          <span class="val">{bands[i][j].toFixed(1)}</span>
        </span>
      {/each}
    {/each}
  </div>
</div>

<style>
  .bands {
    background: #141414;
    border: 1px solid #2e2e2e;
    padding: 6px 10px 8px 10px;
    box-sizing: border-box;
    font-size: 12px;
    color: #ddd;
  }
  .bands-head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 3px 0 6px 0;
    flex-wrap: wrap;
  }
  .section-label {
    color: #aaa;
  }
  .iso {
    display: flex;
    align-items: center;
    gap: 4px;
    color: #bbb;
  }
  .presets {
    display: flex;
    gap: 4px;
  }
  .presets button {
    background: #222;
    border: 1px solid #444;
    color: #ccc;
    font-size: 11px;
    cursor: pointer;
    padding: 1px 6px;
  }
  .presets button:hover {
    background: #2c2c2c;
  }
  .grid {
    display: grid;
    grid-template-columns: 34px repeat(8, 44px);
    gap: 2px 4px;
    align-items: end;
    justify-items: center;
  }
  .col-head,
  .row-head {
    font-size: 11px;
    color: #999;
    cursor: help;
  }
  .row-head {
    justify-self: end;
    align-self: center;
  }
  .cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
  }
  .cell input[type='range'] {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 20px;
    height: 76px;
    margin: 0;
  }
  .val {
    font-size: 9px;
    color: #888;
    font-variant-numeric: tabular-nums;
  }
</style>
