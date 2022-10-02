import * as Comlink from 'comlink';

const engineP: Promise<typeof import('./engineComp/engine')> = import('./engineComp/engine').then(
  async engineMod => {
    await engineMod.default();
    return engineMod;
  }
);

const methods = {
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
};

Comlink.expose(methods);
