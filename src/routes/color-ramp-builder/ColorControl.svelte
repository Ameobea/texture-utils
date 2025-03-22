<script lang="ts">
  import { toCssColor } from 'src/colorUtils';
  import type { ColorRampStep } from 'src/wasmWorker.worker';

  export let step: ColorRampStep;
  export let onInput: (newColorHex: string) => void;
  export let onDelete: () => void;

  const handleInput = (evt: InputEvent) => onInput((evt.target! as HTMLInputElement).value);

  const reset = () => {
    const color = [
      Math.round(step.position * 255),
      Math.round(step.position * 255),
      Math.round(step.position * 255),
    ] as [number, number, number];
    onInput(toCssColor(color));
  };
</script>

<div class="control">
  <input type="color" value={toCssColor(step.color)} on:input={handleInput} />
  <button on:click={reset} type="button">Reset</button>
  <button on:click={onDelete} type="button">Delete</button>
</div>

<style lang="css">
  .control {
    transform: translate(0px, 46px);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }

  .control input[type='color'] {
    border: none;
    padding: 0;
    min-width: 30px;
    min-height: 30px;
  }
</style>
