export interface Pt {
  x: number;
  y: number;
}

/** Solve the linear system `A x = b` for an N×N matrix via Gaussian elimination with partial pivoting. */
const solveLinear = (A: number[][], b: number[]): number[] => {
  const n = b.length;
  const m = A.map((row, i) => [...row, b[i]]);

  for (let col = 0; col < n; col++) {
    let pivot = col;
    for (let r = col + 1; r < n; r++) {
      if (Math.abs(m[r][col]) > Math.abs(m[pivot][col])) {
        pivot = r;
      }
    }
    [m[col], m[pivot]] = [m[pivot], m[col]];

    const pv = m[col][col];
    if (Math.abs(pv) < 1e-12) {
      throw new Error('singular system');
    }
    for (let r = 0; r < n; r++) {
      if (r === col) {
        continue;
      }
      const factor = m[r][col] / pv;
      for (let c = col; c <= n; c++) {
        m[r][c] -= factor * m[col][c];
      }
    }
  }

  return m.map((row, i) => row[n] / row[i]);
};

/**
 * Compute the 3×3 homography (row-major, 9 entries) mapping the four `src` points to the four `dst`
 * points. Points must be in corresponding order.
 */
export const computeHomography = (src: Pt[], dst: Pt[]): number[] => {
  const A: number[][] = [];
  const b: number[] = [];
  for (let i = 0; i < 4; i++) {
    const { x, y } = src[i];
    const { x: X, y: Y } = dst[i];
    A.push([x, y, 1, 0, 0, 0, -X * x, -X * y]);
    b.push(X);
    A.push([0, 0, 0, x, y, 1, -Y * x, -Y * y]);
    b.push(Y);
  }
  const h = solveLinear(A, b);
  return [...h, 1];
};

export const applyHomography = (h: number[], p: Pt): Pt => {
  const x = h[0] * p.x + h[1] * p.y + h[2];
  const y = h[3] * p.x + h[4] * p.y + h[5];
  const w = h[6] * p.x + h[7] * p.y + h[8];
  return { x: x / w, y: y / w };
};

const dist = (a: Pt, b: Pt) => Math.hypot(a.x - b.x, a.y - b.y);

/**
 * Pick output dimensions for a corrected image. `quad` is the source quad in TL/TR/BR/BL order; the
 * base size preserves the quad's apparent scale, and aspect/scale knobs refine it.
 */
export const outputSize = (
  quad: Pt[],
  aspectMode: 'auto' | 'locked',
  lockedAspect: number,
  longEdge: number
): { w: number; h: number } => {
  const [tl, tr, br, bl] = quad;
  const baseW = (dist(tl, tr) + dist(bl, br)) / 2;
  const baseH = (dist(tl, bl) + dist(tr, br)) / 2;
  const aspect = aspectMode === 'locked' ? lockedAspect : baseW / Math.max(baseH, 1e-6);

  let w: number;
  let h: number;
  if (aspect >= 1) {
    w = longEdge;
    h = longEdge / aspect;
  } else {
    h = longEdge;
    w = longEdge * aspect;
  }
  return { w: Math.max(16, Math.round(w)), h: Math.max(16, Math.round(h)) };
};
