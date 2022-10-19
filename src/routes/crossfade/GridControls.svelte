<script context="module" lang="ts">
  export interface CellParams {
    texIx: number;
    rotation: number;
    offsetX: number;
    offsetY: number;
  }
  export type GridParams = CellParams[][];

  export const buildDefaultGridParams = (tileCount: number): GridParams =>
    new Array(tileCount).fill(null).map((_, y) =>
      new Array(tileCount).fill(null).map((_, x) => ({
        texIx: (y + x) % tileCount,
        rotation: 0,
        offsetX: 0,
        offsetY: 0,
      }))
    );

  const getDebugColor = (ix: number) => {
    const [r, g, b] = (() => {
      switch (ix % 8) {
        case 0:
          return [255, 0, 0];
        case 1:
          return [0, 255, 0];
        case 2:
          return [0, 0, 255];
        case 3:
          return [255, 255, 0];
        case 4:
          return [255, 0, 255];
        case 5:
          return [0, 255, 255];
        case 6:
          return [255, 255, 255];
        case 7:
          return [0, 0, 0];
        default:
          throw new Error('unreachable');
      }
    })();
    return `rgba(${r}, ${g}, ${b}, 0.2)`;
  };
</script>

<script lang="ts">
  export let state: GridParams;
  export let texWidth: number;
  export let texHeight: number;
  export let textureCount: number;

  $: textureOptions = new Array(textureCount).fill(null).map((_, ix) => ({
    value: ix,
    label: `Texture ${ix}`,
  }));
</script>

<div class="root">
  {#each state as row, rowIx}
    <div class="row">
      {#each row as cell, colIx}
        <div class="cell" style="background-color: {getDebugColor(cell.texIx)};">
          <div class="source-tex">
            <label for={`tex-${rowIx}-${colIx}`}>Source Texture</label>
            <select id={`tex-${rowIx}-${colIx}`} bind:value={cell.texIx}>
              {#each textureOptions as { value, label }}
                <option {value}>{label}</option>
              {/each}
            </select>
          </div>
          <div class="rotation">
            <label for={`rotation-${rowIx}-${colIx}`}>Rotation</label>
            <div class="rot-controls">
              <button
                class="rot-change"
                on:click={() => {
                  cell.rotation = cell.rotation === 0 ? 3 : cell.rotation - 1;
                }}>-</button
              >
              <div class="rot-display" id={`rotation-${rowIx}-${colIx}`}>{cell.rotation * 90}°</div>
              <button
                class="rot-change"
                on:click={() => {
                  cell.rotation = cell.rotation === 3 ? 0 : cell.rotation + 1;
                }}>+</button
              >
            </div>
          </div>
          <div class="offset-x">
            <label for={`offset-x-${rowIx}-${colIx}`}>Offset X</label>
            <input
              id={`offset-x-${rowIx}-${colIx}`}
              type="range"
              min={-texWidth + 1}
              max={texWidth - 1}
              step="1"
              bind:value={cell.offsetX}
            />
          </div>
          <div class="offset-y">
            <input
              id={`offset-y-${rowIx}-${colIx}`}
              type="range"
              min={-texHeight + 1}
              max={texHeight - 1}
              step="1"
              bind:value={cell.offsetY}
            />
            <label for={`offset-y-${rowIx}-${colIx}`}>Offset Y</label>
          </div>
        </div>
      {/each}
    </div>
  {/each}
</div>
<div class="buttons-container">
  <button
    on:click={() => {
      state = buildDefaultGridParams(state.length);
    }}
  >
    Reset
  </button>
  <button
    on:click={() => {
      state = state.map(row =>
        row.map(cell => ({ ...cell, rotation: Math.floor(Math.random() * 4) }))
      );
    }}
  >
    Randomize Rotations
  </button>
  <button
    on:click={() => {
      state = state.map(row =>
        row.map(cell => ({
          ...cell,
          offsetX: Math.round(Math.random() * (texWidth - 1)),
          offsetY: Math.round(Math.random() * (texHeight - 1)),
        }))
      );
    }}
  >
    Randomize Offsets
  </button>
</div>

<style lang="css">
  .root {
    display: block;
  }

  .row {
    display: flex;
    flex-direction: row;
  }

  .cell {
    display: grid;
    box-sizing: border-box;
    grid-template-areas:
      'source-tex offset-y'
      'rotation offset-y'
      'offset-x spacer';
    grid-template-rows: 2fr 2fr 1fr;
    grid-template-columns: 4fr 1fr;
    height: 140px;
    width: 140px;
    border: 1px solid #888;
  }

  .cell > div {
    position: relative;
    padding: 4px;
    font-size: 13px;
  }

  .cell .source-tex,
  .cell .rotation {
    padding-left: 14px;
  }

  .cell select {
    margin-top: 4px;
  }

  .cell .source-tex {
    grid-area: source-tex;
  }

  .cell .rot-controls {
    display: flex;
    flex-direction: row;
    margin-top: 4px;
  }

  .cell .rot-display {
    width: 25px;
    text-align: center;
  }

  .cell button.rot-change {
    width: 10px;
    height: 10px;
    padding: 6px;
    line-height: 0;
    margin-left: 4px;
    margin-right: 4px;
  }

  .cell .offset-x {
    grid-area: offset-x;
  }

  .cell .offset-x label {
    width: 40px;
    position: absolute;
    transform: translate(-48px, 2px);
  }

  .cell .offset-y label {
    position: absolute;
    transform: translate(-6px, -8px);
  }

  .cell .offset-y {
    grid-area: offset-y;
    text-align: center;
    writing-mode: vertical-rl;
  }

  .cell .offset-x,
  .cell .offset-y {
    font-size: 10px;
    text-align: center;
  }

  .cell .offset-x input[type='range'],
  .cell .offset-y input[type='range'] {
    width: 260px;
    position: absolute;
    transform-origin: 0 0;
  }

  .cell .offset-x input[type='range'] {
    transform: scale(0.495, 0.495);
    bottom: -9px;
    left: 0px;
  }

  .cell .offset-y input[type='range'] {
    transform: scale(0.495, 0.495) rotate(90deg) translate(-110px, -536px);
  }

  .cell .rotation {
    grid-area: rotation;
  }

  .buttons-container {
    display: flex;
    flex-direction: column;
    gap: 10px;
    justify-content: space-between;
    margin-top: 20px;
    margin-left: 10px;
  }

  .buttons-container button {
    padding-top: 10px;
    padding-bottom: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
