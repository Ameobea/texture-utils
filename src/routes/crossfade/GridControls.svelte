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
</script>

<div class="root">
  {#each state as row, rowIx}
    <div class="row">
      {#each row as cell, colIx}
        <div class="cell" style="background-color: {getDebugColor(cell.texIx)};">
          <div class="cell-content">
            <div class="cell-text">
              <span class="cell-text-content">Source Index: {cell.texIx}</span>
            </div>
            <div class="cell-rotation">
              <span class="cell-rotation-content">Rotation: {cell.rotation}</span>
            </div>
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
        row.map(cell => ({ ...cell, rotation: Math.round(Math.random() * 3) }))
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
          offsetX: Math.round(Math.random() * 1000),
          offsetY: Math.round(Math.random() * 1000),
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
    display: flex;
    justify-content: center;
    align-items: center;
    height: 200px;
    width: 200px;
    border: 1px solid #888;
  }

  .buttons-container {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    margin-top: 20px;
  }
</style>
