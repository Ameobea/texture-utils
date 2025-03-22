export const toCssColor = (color: [number, number, number]): string =>
  `#${color.map(c => Math.round(c).toString(16).padStart(2, '0')).join('')}`;
