import { browser } from '$app/environment';

export const parseImageToRGBA = async (
  file: File
): Promise<{ data: Uint8ClampedArray; width: number; height: number }> => {
  const image = await createImageBitmap(file);
  const canvas = document.createElement('canvas');
  canvas.width = image.width;
  canvas.height = image.height;
  const context = canvas.getContext('2d')!;
  context.drawImage(image, 0, 0);
  const data = context.getImageData(0, 0, image.width, image.height).data;
  return { data, width: image.width, height: image.height };
};

export const setPixelsToCanvas = (
  ysDataUint8: Uint8ClampedArray,
  width: number,
  height: number
): HTMLCanvasElement => {
  const scratchCanvas = document.createElement('canvas');
  const scratchCtx = scratchCanvas.getContext('2d')!;
  scratchCanvas.width = width;
  scratchCanvas.height = height;
  const imageData = new ImageData(ysDataUint8, width, height);
  scratchCtx.putImageData(imageData, 0, 0);
  return scratchCanvas;
};

export const setImageData = (
  img: HTMLImageElement,
  { data: ysDataUint8, width, height }: { data: Uint8ClampedArray; width: number; height: number }
) => {
  if (!browser) {
    return;
  }

  const scratchCanvas = setPixelsToCanvas(ysDataUint8, width, height);
  img.src = scratchCanvas.toDataURL();
};

export const setImageDataToCanvas = (
  canvas: HTMLCanvasElement,
  { data: ysDataUint8, width, height }: { data: Uint8ClampedArray; width: number; height: number }
) => {
  if (!browser) {
    return;
  }

  const ctx = canvas.getContext('2d')!;
  const ysDataUint8Clamped = new Uint8ClampedArray(ysDataUint8.buffer);
  const imageData = new ImageData(ysDataUint8Clamped, width, height);
  ctx.putImageData(imageData, 0, 0);
};
