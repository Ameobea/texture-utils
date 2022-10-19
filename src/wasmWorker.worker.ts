import * as Comlink from 'comlink';
import type { CrossfadeParams } from './routes/crossfade/+page.svelte';

const engineP: Promise<typeof import('./engineComp/engine')> = import('./engineComp/engine').then(
  async engineMod => {
    await engineMod.default();
    return engineMod;
  }
);

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

    console.log({ tileCount, indices, rotations, xOffsets, yOffsets, width, height });
    engine.crossfade_set_texture_indices(indices);
    engine.crossfade_set_texture_rotations(rotations);
    engine.crossfade_set_texture_offsets(xOffsets, yOffsets);

    const generated = engine.crossfade_generate(
      width,
      height,
      tileCount,
      params.threshold,
      params.debug
    );
    return Comlink.transfer(generated, [generated.buffer]);
  },
};

Comlink.expose(methods);
