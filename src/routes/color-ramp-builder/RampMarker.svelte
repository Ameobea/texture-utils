<script lang="ts">
  import type { ColorRampStep } from 'src/wasmWorker.worker';
  import ColorControl from './ColorControl.svelte';
  import { toCssColor } from 'src/colorUtils';

  export let steps: ColorRampStep[];
  export let index: number;
  export let activeStepIx: number;
  export let handleMouseDown: (index: number, event: MouseEvent) => void;
  export let onDelete: () => void;

  $: step = steps[index];

  const handleMouseDownInner = (evt: MouseEvent) => handleMouseDown(index, evt);

  const updateColor = (index: number, hex: string) => {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    steps[index].color = [r, g, b];
    steps = [...steps];
  };
</script>

<div
  class="marker{activeStepIx === index ? ' active' : ''}"
  style="left: {step.position * 100}%; background: {toCssColor(step.color)}"
  on:mousedown={handleMouseDownInner}
>
  {#if activeStepIx === index}
    <ColorControl {step} onInput={newColorHex => updateColor(index, newColorHex)} {onDelete} />
  {/if}
</div>

<style lang="css">
  .marker {
    position: absolute;
    top: -5px;
    width: 10px;
    height: 40px;
    margin-left: -5px;
    border: 1px solid #333;
    border-radius: 4px;
    cursor: pointer;
  }

  .marker.active {
    border: 2px solid #fdd;
  }
</style>
