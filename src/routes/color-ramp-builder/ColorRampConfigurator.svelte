<script lang="ts">
  import { toCssColor } from 'src/colorUtils';
  import type { ColorRampStep } from 'src/wasmWorker.worker';

  import { onMount } from 'svelte';
  import RampMarker from './RampMarker.svelte';

  export let steps: ColorRampStep[];

  let viewportWidth = 0;
  $: trackWidth = Math.floor(viewportWidth * 0.75);

  let trackEl: HTMLDivElement | null = null;
  let draggingStepIndex: number | null = null;
  let activeStepIx = 0;

  $: sortedSteps = [...steps].sort((a, b) => a.position - b.position);

  $: gradientStyle = `linear-gradient(to right, ${sortedSteps
    .map(s => `${toCssColor(s.color)} ${s.position * 100}%`)
    .join(', ')})`;

  const handleMouseDown = (index: number, event: MouseEvent) => {
    if (index !== 0 && index !== steps.length - 1) {
      draggingStepIndex = index;
    }
    activeStepIx = index;
    event.stopPropagation();
    event.preventDefault();
  };

  const handleMouseMove = (event: MouseEvent) => {
    if (draggingStepIndex === null) {
      return;
    }
    // TODO: this is bad for perf and there should be an easier way
    const rect = trackEl!.getBoundingClientRect();
    let newPos = (event.clientX - rect.left) / trackWidth;
    newPos = Math.max(0, Math.min(1, newPos));
    const draggingStep = steps[draggingStepIndex];
    draggingStep.position = newPos;
    steps = steps.sort((a, b) => a.position - b.position);
    draggingStepIndex = steps.indexOf(draggingStep);
    activeStepIx = draggingStepIndex;
  };

  const handleMouseUp = () => {
    if (draggingStepIndex !== null) {
      draggingStepIndex = null;
      steps = [...sortedSteps];
    }
  };

  const handleDoubleClick = (event: MouseEvent) => {
    if (!trackEl) {
      return;
    }

    const rect = trackEl.getBoundingClientRect();
    let newPos = (event.clientX - rect.left) / trackWidth;
    newPos = Math.max(0, Math.min(1, newPos));

    const first = sortedSteps[0].position;
    const last = sortedSteps[sortedSteps.length - 1].position;
    if (newPos <= first || newPos >= last) {
      return;
    }

    let leftStep = sortedSteps[0];
    let rightStep = sortedSteps[sortedSteps.length - 1];
    for (let i = 0; i < sortedSteps.length - 1; i++) {
      if (sortedSteps[i].position <= newPos && sortedSteps[i + 1].position >= newPos) {
        leftStep = sortedSteps[i];
        rightStep = sortedSteps[i + 1];
        break;
      }
    }

    const f = (newPos - leftStep.position) / (rightStep.position - leftStep.position);
    const newColor: [number, number, number] = [
      Math.round(leftStep.color[0] * (1 - f) + rightStep.color[0] * f),
      Math.round(leftStep.color[1] * (1 - f) + rightStep.color[1] * f),
      Math.round(leftStep.color[2] * (1 - f) + rightStep.color[2] * f),
    ];

    const newStep: ColorRampStep = {
      position: newPos,
      color: newColor,
    };
    steps = [...steps, newStep].sort((a, b) => a.position - b.position);
    activeStepIx = steps.indexOf(newStep);
  };

  const deleteMarker = (index: number) => {
    steps = steps.filter((_, i) => i !== index);
    activeStepIx = Math.max(0, Math.min(steps.length - 1, activeStepIx));
  };

  onMount(() => {
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  });
</script>

<svelte:window bind:innerWidth={viewportWidth} />

<div class="ramp-editor">
  <div
    class="track"
    style="width: {trackWidth}px; background: {gradientStyle};"
    bind:this={trackEl}
    on:dblclick={handleDoubleClick}
  >
    <!-- start and end steps are added first so they don't block other steps and prevent them from being dragged -->
    <RampMarker
      bind:steps
      index={0}
      {activeStepIx}
      {handleMouseDown}
      onDelete={() => deleteMarker(0)}
    />
    <RampMarker
      bind:steps
      index={steps.length - 1}
      {activeStepIx}
      {handleMouseDown}
      onDelete={() => deleteMarker(steps.length - 1)}
    />
    {#each steps as step, index}
      {#if index !== 0 && index !== steps.length - 1}
        <RampMarker
          bind:steps
          {index}
          {activeStepIx}
          {handleMouseDown}
          onDelete={() => deleteMarker(index)}
        />
      {/if}
    {/each}
  </div>
</div>

<style>
  .ramp-editor {
    width: 80vw;
    margin: auto;
    padding-bottom: 80px;
  }

  .track {
    position: relative;
    height: 30px;
    background: #eee;
    border: 1px solid #ccc;
    margin-bottom: 20px;
    cursor: pointer;
  }
</style>
