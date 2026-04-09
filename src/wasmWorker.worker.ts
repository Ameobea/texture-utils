import * as Comlink from 'comlink';

const engineP: Promise<typeof import('./engineComp/engine')> = import('./engineComp/engine').then(
  async engineMod => {
    await engineMod.default();
    return engineMod;
  }
);

export interface CellParams {
  texIx: number;
  rotation: number;
  offsetX: number;
  offsetY: number;
}

export type GridParams = CellParams[][];

export interface CrossfadeParams {
  threshold: number;
  debug: boolean;
  contrastCorrectionFactor: number;
  grid: GridParams;
}

export interface ColorRampStep {
  color: [number, number, number];
  position: number;
}

export interface ColorRamp {
  steps: ColorRampStep[];
}

export interface ReverseColorRampParams {
  colorA_srgb: [number, number, number];
  colorB_srgb: [number, number, number];
  vMin: number;
  vMax: number;
  curveSteepness: number;
  curveOffset: number;
  perpSigma: number;
  baseFallback: number;
  colorSpace?: 'srgb' | 'linear';
}

const methods = {
  // lut
  encodeImage: async (palette: Uint8Array, imgPixelData: Uint8Array): Promise<Uint8Array> => {
    const engine = await engineP;

    const encoded = engine.encode_image(palette, imgPixelData);
    return Comlink.transfer(encoded, [encoded.buffer]);
  },
  decodeImage: async (palette: Uint8Array, encoded: Uint8Array): Promise<Uint8Array> => {
    const engine = await engineP;

    const decoded = engine.decode_pixels(palette, encoded);
    return Comlink.transfer(decoded, [decoded.buffer]);
  },
  genPalette: async (
    imgPixelData: Uint8Array,
    count: number
  ): Promise<{ palette: Uint8Array; score: number }> => {
    const engine = await engineP;

    const palette = engine.gen_palette(count, imgPixelData, Math.random() * 1000000);
    const score = engine.get_palette_gen_score();
    return { palette: Comlink.transfer(palette, [palette.buffer]), score };
  },
  computeLoss: async (orig: Uint8Array, roundtripped: Uint8Array) => {
    const engine = await engineP;

    return engine.compute_loss(orig, roundtripped);
  },
  buildFullLUT: async (palette: Uint8Array) => {
    const engine = await engineP;

    const lut = engine.build_full_lookup_table(palette);
    return Comlink.transfer(lut, [lut.buffer]);
  },

  // crossfade
  setCrossfadeTextures: async (textureData: Uint8Array[]) => {
    const engine = await engineP;

    engine.crossfade_reset();
    textureData.forEach((data, texIx) => engine.crossfade_set_texture(data, texIx));
  },
  crossfadeGenerate: async (width: number, height: number, params: CrossfadeParams) => {
    const engine = await engineP;

    const tileCount = params.grid.length;
    const indices = new Uint32Array(tileCount * tileCount);
    const rotations = new Uint8Array(tileCount * tileCount);
    const xOffsets = new Uint32Array(tileCount * tileCount);
    const yOffsets = new Uint32Array(tileCount * tileCount);

    params.grid.forEach((row, rowIx) => {
      row.forEach((col, colIx) => {
        indices[rowIx * tileCount + colIx] = col.texIx;
        rotations[rowIx * tileCount + colIx] = col.rotation;
        xOffsets[rowIx * tileCount + colIx] = col.offsetX;
        yOffsets[rowIx * tileCount + colIx] = col.offsetY;
      });
    });

    engine.crossfade_set_texture_indices(indices);
    engine.crossfade_set_texture_rotations(rotations);
    engine.crossfade_set_texture_offsets(xOffsets, yOffsets);

    const generated = engine.crossfade_generate(
      width,
      height,
      tileCount,
      params.threshold,
      params.debug,
      params.contrastCorrectionFactor
    );
    return Comlink.transfer(generated, [generated.buffer]);
  },

  // color ramp
  setColorRampInputTexture: async (textureData: Uint8Array) => {
    const engine = await engineP;

    engine.color_ramp_set_input_texture(textureData);
  },
  applyColorRamp: async (ramp: ColorRamp) => {
    const engine = await engineP;

    const encodedRamp = new Float32Array(ramp.steps.length * 4);
    for (let i = 0; i < ramp.steps.length; i++) {
      const step = ramp.steps[i];
      encodedRamp[i * 4 + 0] = step.color[0];
      encodedRamp[i * 4 + 1] = step.color[1];
      encodedRamp[i * 4 + 2] = step.color[2];
      encodedRamp[i * 4 + 3] = step.position;
    }

    const generated = engine.color_ramp_apply_ramp(encodedRamp);
    return Comlink.transfer(generated, [generated.buffer]);
  },
  getGrayscaleColorRampImageData: async (): Promise<Uint8Array> => {
    const engine = await engineP;

    const imgData = engine.color_ramp_get_grayscale_image_data();
    return Comlink.transfer(imgData, [imgData.buffer]);
  },

  // normal map compose
  normalMapCompose: async (
    baseMapData: Uint8Array,
    detailMapData: Uint8Array,
    weight: number
  ): Promise<Uint8Array> => {
    const engine = await engineP;

    const composed = engine.normal_map_compose(baseMapData, detailMapData, weight);
    return Comlink.transfer(composed, [composed.buffer]);
  },

  // normal map filter
  normalMapFilter: async (
    pixelData: Uint8Array,
    width: number,
    height: number,
    filterMode: number,
    sigma: number,
    sigmaHigh: number,
    useBilateral: boolean,
    rangeSigma: number
  ): Promise<Uint8Array> => {
    const engine = await engineP;

    const filtered = engine.normal_map_filter(pixelData, width, height, filterMode, sigma, sigmaHigh, useBilateral, rangeSigma);
    return Comlink.transfer(filtered, [filtered.buffer]);
  },

  // reverse color ramp
  reverseColorRampSetInputTexture: async (textureData: Uint8Array, isSrgb: boolean) => {
    const engine = await engineP;

    engine.reverse_color_ramp_set_input_texture(textureData, isSrgb);
  },
  reverseColorRamp: async (
    width: number,
    height: number,
    params: ReverseColorRampParams
  ): Promise<Uint8Array<ArrayBuffer>> => {
    const engine = await engineP;

    const generated = engine.reverse_color_ramp(
      width,
      height,
      new Float32Array(params.colorA_srgb),
      new Float32Array(params.colorB_srgb),
      params.vMin,
      params.vMax,
      params.curveSteepness,
      params.curveOffset,
      params.perpSigma,
      params.baseFallback
    ) as Uint8Array<ArrayBuffer>;
    return Comlink.transfer(generated, [generated.buffer]);
  },
};

export type WorkerInterface = typeof methods;

Comlink.expose(methods);
