export const toCssColor = (color: [number, number, number]): string =>
  `#${color.map(c => Math.round(c).toString(16).padStart(2, '0')).join('')}`;

export const fromCssColor = (color: string): [number, number, number] => {
  const r = parseInt(color.slice(1, 3), 16);
  const g = parseInt(color.slice(3, 5), 16);
  const b = parseInt(color.slice(5, 7), 16);
  return [r, g, b];
};
