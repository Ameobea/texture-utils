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
