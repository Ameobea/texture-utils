// const startTraining = async () => {
//   if (!modelMod) {
//     alert('Model not loaded yet');
//     return;
//   }

//   if (processState.type !== 'converted') {
//     throw new Error('Invalid state: ' + processState.type);
//   }

//   const { data, width, height } = processState;
//   processState = { type: 'training', data, width, height };

//   const uniqPixels = new Set<number>();
//   const uniqPixelIndices = [];

//   // Map [0,255] to [-1,1] and strip off alpha channel
//   const dataFloat = new Float32Array(width * height * 3);
//   for (let pxIx = 0; pxIx < width * height; pxIx += 1) {
//     // dataFloat[pxIx * 3 + 0] = data[pxIx * 4 + 0] / 127.5 - 1;
//     // dataFloat[pxIx * 3 + 1] = data[pxIx * 4 + 1] / 127.5 - 1;
//     // dataFloat[pxIx * 3 + 2] = data[pxIx * 4 + 2] / 127.5 - 1;

//     const r = data[pxIx * 4 + 0];
//     const g = data[pxIx * 4 + 1];
//     const b = data[pxIx * 4 + 2];

//     // Convert RGB values to a single number
//     const px = r * 65536 + g * 256 + b;
//     if (!uniqPixels.has(px)) {
//       uniqPixels.add(px);
//       uniqPixelIndices.push(pxIx);
//     }
//     // else if (Math.random() > 0.8) {
//     //   uniqPixelIndices.push(pxIx);
//     // }

//     const [h, s, v] = RGBtoHSV(r, g, b);
//     dataFloat[pxIx * 3 + 0] = h * 2 - 1;
//     dataFloat[pxIx * 3 + 1] = s * 2 - 1;
//     dataFloat[pxIx * 3 + 2] = v * 2 - 1;
//   }

//   // modelMod.tf.setBackend('cpu');

//   const model = modelMod.buildModel(4, false);
//   model.summary();

//   const optimizer = modelMod.tf.train.sgd(0.1);

//   model.compile({
//     optimizer,
//     // loss: 'meanSquaredError',
//     loss: 'meanAbsoluteError',
//   });

//   const uniquePixelsData = new Float32Array(uniqPixelIndices.length * 3);
//   let i = 0;
//   for (const pxIx of uniqPixelIndices) {
//     const [h, s, v] = dataFloat.slice(pxIx * 3, pxIx * 3 + 3);
//     uniquePixelsData[i * 3 + 0] = h;
//     uniquePixelsData[i * 3 + 1] = s;
//     uniquePixelsData[i * 3 + 2] = v;
//     i += 1;
//   }

//   const trainingTensor = modelMod.tf.tensor2d(uniquePixelsData, [uniqPixelIndices.length, 3]);
//   const fullInputsTensor = modelMod.tf.tensor2d(dataFloat, [width * height, 3]);

//   await model.fit(trainingTensor, trainingTensor, {
//     epochs: 50,
//     callbacks: {
//       onEpochEnd: async (epoch, logs) => {
//         console.log(`Epoch ${epoch}: loss = ${logs?.loss}`);
//         // TODO: Record + plot
//         const ys = model.predict(fullInputsTensor) as Tensor;
//         const ysData = await ys.data();

//         if (!rxImg) {
//           return;
//         }

//         const ysDataUint8 = new Uint8ClampedArray(width * height * 4);
//         for (let i = 0; i < ysData.length / 3; i += 1) {
//           const h = ysData[i * 3 + 0] * 0.5 + 0.5;
//           const s = ysData[i * 3 + 1] * 0.5 + 0.5;
//           const v = ysData[i * 3 + 2] * 0.5 + 0.5;

//           const [r, g, b] = HSVtoRGB(h, s, v);

//           ysDataUint8[i * 4 + 0] = r;
//           ysDataUint8[i * 4 + 1] = g;
//           ysDataUint8[i * 4 + 2] = b;
//           ysDataUint8[i * 4 + 3] = 255;
//         }

//         setImageData(rxImg, { data: ysDataUint8, width, height });
//       },
//     },
//     // batchSize: width * height,
//     // batchSize: Math.round((width * height) / 16),
//     batchSize: 256,
//     // shuffle: true,
//     // stepsPerEpoch: 32,
//   });
// };
