export * as tf from '@tensorflow/tfjs';
import * as tf from '@tensorflow/tfjs';
import type { Kwargs } from '@tensorflow/tfjs-layers/dist/types';

// const lut = tf.variable(tf.zeros([size, enableAlpha ? 4 : 3]));
// const lut = tf.variable(tf.initializers.glorotNormal({}).apply([size, enableAlpha ? 4 : 3]));
// const lut = tf.variable(
//   tf.tensor2d([
//     [0, 0, 1],
//     [0, 1, 0],
//   ])
// );

// const forward = (x: tf.Tensor) => {
//   // [0,1] -> [0,size-1]
//   const scaled = tf.mul(x, tf.scalar(size - 1)).clipByValue(0, size - 1);
//   // console.log('scaled:');
//   // scaled.print();

//   const floors = tf.floor(scaled);
//   const ceils = tf.ceil(scaled);

//   const eqMask = tf.equal(floors, ceils);
//   console.log('eqMask:');
//   eqMask.print();
//   const rawWeights = tf.sub(scaled, floors);
//   const weights = tf.where(eqMask, tf.onesLike(rawWeights), rawWeights);
//   const invWeights = tf.where(eqMask, tf.zerosLike(weights), tf.sub(tf.scalar(1), weights));

//   // console.log('xs: ');
//   // x.print();
//   // console.log('scaled: ');
//   // scaled.print();
//   // console.log('floors: ');
//   // floors.print();
//   // console.log('ceils: ');
//   // ceils.print();
//   // console.log('weights: ');
//   // weights.print();
//   // console.log('invWeights: ');
//   // invWeights.print();

//   const floorsIndex = tf.cast(floors, 'int32');
//   // const ceilsIndex = tf.add(floorsIndex, tf.scalar(1, 'int32')).clipByValue(0, size - 1);
//   const ceilsIndex = tf.cast(ceils, 'int32');
//   console.log('floorsIndex: ');
//   floorsIndex.print();
//   // console.log('ceilsIndex: ');
//   // ceilsIndex.print();
//   const lutFloors = tf.gather(lut, floorsIndex);
//   const lutCeils = tf.gather(lut, ceilsIndex);
//   const floorWeights = tf.mul(lutFloors, invWeights);
//   // console.log('lutFloors: ');
//   // lutFloors.print();
//   // console.log('lutCeils: ');
//   // lutCeils.print();
//   console.log('floorWeights: ');
//   floorWeights.print();
//   const ceilWeights = tf.mul(lutCeils, weights);
//   return tf.add(floorWeights, ceilWeights);
// };

// const forward = (x: tf.Tensor) => {
//   // [0,1] -> [0,size-1]
//   const rawScaled = tf.mul(x, tf.scalar(size - 1)).clipByValue(0, size - 1);

//   const floors0 = tf.floor(rawScaled);
//   const ceils0 = tf.ceil(rawScaled);
//   const eqMask = tf.equal(floors0, ceils0);

//   const scaled = tf.add(
//     rawScaled,
//     tf.where(eqMask, tf.fill(rawScaled.shape, 0.0001), tf.zerosLike(rawScaled))
//   );
//   const floors = tf.floor(scaled);
//   const ceils = tf.ceil(scaled);

//   const weights = tf.sub(scaled, floors);
//   const invWeights = tf.sub(tf.scalar(1), weights);

//   const floorsIndex = tf.cast(floors, 'int32');
//   const ceilsIndex = tf.cast(ceils, 'int32');
//   const lutFloors = tf.gather(lut, floorsIndex);
//   const lutCeils = tf.gather(lut, ceilsIndex);
//   const floorWeights = tf.mul(lutFloors, invWeights);
//   const ceilWeights = tf.mul(lutCeils, weights);
//   return tf.add(floorWeights, ceilWeights);
// };

export const buildPixelPerceptron = (layer0Size: number, enableAlpha?: boolean) => {
  const dense0 = tf.layers.dense({
    units: layer0Size,
    activation: 'linear',
    useBias: true,
    kernelInitializer: 'glorotNormal',
    biasInitializer: 'zeros',
    trainable: true,
  });
  const dense1 = tf.layers.dense({
    units: enableAlpha ? 4 : 3,
    activation: 'linear',
    useBias: true,
    kernelInitializer: 'glorotNormal',
    biasInitializer: 'zeros',
    trainable: true,
  });

  const forward = (x: tf.Tensor) => {
    const y0 = dense0.apply(x) as tf.Tensor;
    const y1 = dense1.apply(y0) as tf.Tensor;
    return y1;
  };

  return { dense0, dense1, forward };

  // const initializer = tf.initializers.glorotNormal({});
  // // input size: [1]
  // const layer0Weights = tf.variable(initializer.apply([1, layer0Size]));
  // const layer0Biases = tf.variable(initializer.apply([layer0Size]));
  // // input size: [layer0Size]
  // const layer1Weights = tf.variable(initializer.apply([layer0Size, enableAlpha ? 4 : 3]));
  // const layer1Biases = tf.variable(initializer.apply([enableAlpha ? 4 : 3]));

  // const forward = (x: tf.Tensor) => {
  //   const layer0 = tf.add(tf.matMul(x, layer0Weights), layer0Biases);
  //   const layer1 = tf.add(tf.matMul(layer0, layer1Weights), layer1Biases);
  //   return layer1;
  // };

  // return { layer0Weights, layer0Biases, layer1Weights, layer1Biases, forward };
};

export const buildModel = (decodeLayer0Size: number, enableAlpha = false) => {
  const input = tf.input({ shape: [enableAlpha ? 4 : 3] });

  const encode0 = tf.layers
    .dense({
      units: 4,
      // activation: 'tanh',
      useBias: true,
      kernelInitializer: 'glorotNormal',
      biasInitializer: 'zeros',
      trainable: true,
    })
    .apply(input) as tf.SymbolicTensor;
  // const encode1 = tf.layers
  //   .dense({
  //     inputShape: [enableAlpha ? 4 : 3],
  //     units: 16,
  //     // activation: 'tanh',
  //     useBias: true,
  //     kernelInitializer: 'glorotNormal',
  //     biasInitializer: 'zeros',
  //     trainable: true,
  //   })
  //   .apply(encode0) as tf.SymbolicTensor;
  const encode2 = tf.layers
    .dense({
      units: 1,
      activation: 'sigmoid',
      useBias: true,
      kernelInitializer: 'glorotNormal',
      biasInitializer: 'zeros',
      trainable: true,
    })
    .apply(encode0) as tf.SymbolicTensor;

  // const decodeStart = tf.layers.concatenate().apply([encode2, memory]);
  // const decodeStart = tf.sin(
  //   tf.mul(encode2, tf.tensor1d())
  // );

  class DecodeStartLayer extends tf.layers.Layer {
    private vals!: tf.Tensor;

    constructor() {
      super({});
    }

    build(inputShape: tf.Shape | tf.Shape[]): void {
      const multipliers = [1, Math.PI * 1, Math.PI * 2, Math.PI * 4]; //Math.PI * 16];
      this.vals = tf.tensor1d(multipliers);
    }

    call(
      inputs: tf.Tensor<tf.Rank> | tf.Tensor<tf.Rank>[],
      _kwargs: Kwargs
    ): tf.Tensor<tf.Rank> | tf.Tensor<tf.Rank>[] {
      return tf.tidy(() => {
        const x = Array.isArray(inputs) ? inputs[0] : inputs;
        const y = tf.sin(tf.mul(x, this.vals));
        return y;
      });
    }

    computeOutputShape(inputShape: tf.Shape | tf.Shape[]): tf.Shape | tf.Shape[] {
      const newShape = [...(inputShape as tf.Shape)];
      newShape[newShape.length - 1]! *= this.vals.shape[0];
      return newShape;
    }

    getConfig(): tf.serialization.ConfigDict {
      return {};
    }

    static get className() {
      return 'DecodeStartLayer';
    }
  }

  // const decodeStart = new DecodeStartLayer().apply(encode2) as tf.SymbolicTensor;
  const decodeStart = encode2;
  const decode0 = tf.layers
    .dense({
      units: decodeLayer0Size,
      // activation: 'tanh',
      useBias: true,
      kernelInitializer: 'glorotNormal',
      biasInitializer: 'zeros',
      trainable: true,
    })
    .apply(decodeStart) as tf.SymbolicTensor;
  // const decode1 = tf.layers
  //   .dense({
  //     units: decodeLayer0Size,
  //     activation: 'tanh',
  //     useBias: true,
  //     kernelInitializer: 'glorotNormal',
  //     biasInitializer: 'zeros',
  //     trainable: true,
  //   })
  //   .apply(decode0) as tf.Tensor;
  const decodeFinal = tf.layers
    .dense({
      units: enableAlpha ? 4 : 3,
      useBias: true,
      kernelInitializer: 'glorotNormal',
      biasInitializer: 'zeros',
      trainable: true,
    })
    .apply(decode0) as tf.SymbolicTensor;

  const model = tf.model({ inputs: input, outputs: decodeFinal });

  return model;
};

// const buildModel = (size: number, enableAlpha?: boolean) => {
//   const { lut, forward } = buildDifferentiableLUT(size, enableAlpha);
//   const model = tf.sequential({
//     layers: [
//       tf.layers.inputLayer({ inputShape: [enableAlpha ? 4 : 3] }),
//       tf.layers.dense({
//         units: enableAlpha ? 4 : 3,
//         activation: forward,
//         useBias: false,
//         kernelInitializer: () => lut,
//       }),
//     ],
//   });
//   return model;
// }
