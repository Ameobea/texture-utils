<script lang="ts">
  import type { Sequential, Tensor, Variable } from '@tensorflow/tfjs';
  import { onMount } from 'svelte';

  let modelMod: typeof import('../../model') | null = null;

  onMount(async () => {
    modelMod = await import('../../model');
    const tf = modelMod.tf;

    tf.setBackend('cpu');

    const inputs = [
      [0.8, 0.2, 0.4],
      [0.0, 0.5, 0.5],
      [0.1, 0.3, 0.6],
      [0.8, 0.2, 0.5],
    ];
    const inputsTensor = tf.tensor2d(inputs);

    const model: Sequential = modelMod.buildModel(4, false);
    model.summary();

    const optimizer = tf.train.sgd(0.1);

    model.compile({
      optimizer,
      loss: 'meanSquaredError',
    });

    await model.fit(inputsTensor, inputsTensor, {
      epochs: 100,
      callbacks: {
        onEpochEnd: (epoch, logs) => {
          console.log(`Epoch ${epoch}: loss = ${logs?.loss}`);
        },
      },
    });

    const pred = model.predict(inputsTensor) as Tensor;
    console.log('pred:');
    pred.print();

    console.log('Weights:');
    model.getWeights().forEach(w => {
      console.log((w as Variable).name);
      w.print();
    });
  });
</script>

<div class="root">TODO</div>

<style lang="css">
  .root {
    display: flex;
    flex-direction: column;
  }
</style>
