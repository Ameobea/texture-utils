<script lang="ts">
  import { toCssColor, fromCssColor } from '../../colorUtils';

  export let label: string;
  export let value: [number, number, number];
  export let onInput: (newColor: [number, number, number]) => void;

  let hexValue: string;
  $: hexValue = toCssColor(value.map(c => c * 255) as [number, number, number]);

  const handleInput = (evt: Event) => {
    const target = evt.target as HTMLInputElement;
    const rgb = fromCssColor(target.value);
    onInput(rgb.map(c => c / 255) as [number, number, number]);
  };
</script>

<div>
  <label for="rev-color-ramp-color-picker">{label}</label>
  <input id="rev-color-ramp-color-picker" type="color" value={hexValue} on:input={handleInput} />
</div>
