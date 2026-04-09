export type ReverseColorRampParams = {
  colorA_srgb: [number, number, number];
  colorB_srgb: [number, number, number];
  vMin: number;
  vMax: number;
  curveSteepness: number; // n, >= 1.0
  curveOffset: number; // [0, 1]
  perpSigma: number;
  baseFallback: number;
  colorSpace?: 'srgb' | 'linear';
};

export type ReverseColorRampUniformData = {
  A_lin: [number, number, number];
  U: [number, number, number];
  invLen: number;
  vMin: number;
  vMax: number;
  curveSteepness: number;
  curveOffset: number;
  perpSigma: number;
  baseFallback: number;
};

const srgbToLinear = (c: number): number =>
  c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);

export const ReverseColorRampCommonFunctions = `
vec3 srgb_to_linear(vec3 c) {
  vec3 lo = c / 12.92;
  vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
  bvec3 useLo = lessThanEqual(c, vec3(0.04045));
  return vec3(useLo.x ? lo.x : hi.x, useLo.y ? lo.y : hi.y, useLo.z ? lo.z : hi.z);
}
`;

export const buildReverseColorRampUniformData = (
  p: ReverseColorRampParams
): ReverseColorRampUniformData => {
  const A_lin = p.colorA_srgb.map(srgbToLinear) as [number, number, number];
  const B_lin = p.colorB_srgb.map(srgbToLinear) as [number, number, number];

  const ux = B_lin[0] - A_lin[0];
  const uy = B_lin[1] - A_lin[1];
  const uz = B_lin[2] - A_lin[2];
  const len = Math.sqrt(ux * ux + uy * uy + uz * uz) || 1;

  const u = [ux / len, uy / len, uz / len] as const;
  const invLen = 1 / len;

  return {
    A_lin,
    U: [u[0], u[1], u[2]],
    invLen,
    vMin: p.vMin,
    vMax: p.vMax,
    curveSteepness: p.curveSteepness,
    curveOffset: p.curveOffset,
    perpSigma: p.perpSigma,
    baseFallback: p.baseFallback,
  };
};

export const buildReverseColorRampUniformFunction = (
  fnName: string,
  prefix = 'reverseColorRamp',
  colorSpace: 'srgb' | 'linear' = 'srgb'
): string => `
uniform vec3 ${prefix}A;
uniform vec3 ${prefix}U;
uniform float ${prefix}InvLen;
uniform float ${prefix}VMin;
uniform float ${prefix}VMax;
uniform float ${prefix}CurveSteepness;
uniform float ${prefix}CurveOffset;
uniform float ${prefix}PerpSigma;
uniform float ${prefix}BaseFallback;

float ${fnName}(vec3 baseColor${colorSpace === 'srgb' ? '_srgb' : '_linear'}) {
  ${colorSpace === 'srgb' ? 'vec3 c = srgb_to_linear(baseColor_srgb);\n  vec3 rel = c' : 'vec3 rel = baseColor_linear'} - ${prefix}A;
  float proj = dot(rel, ${prefix}U);

  float t = clamp(proj * ${prefix}InvLen, 0.0, 1.0);

  float curveOffset = clamp(${prefix}CurveOffset, 0.0, 1.0);
  if (curveOffset > 0.0 && curveOffset < 1.0) {
    t = clamp((t - curveOffset) / (1.0 - curveOffset), 0.0, 1.0);
  }

  float n = max(${prefix}CurveSteepness, 1.0);
  float t_n = pow(t, n);
  float one_minus_t_n = pow(1.0 - t, n);
  float t_curved = (t_n + one_minus_t_n > 0.0) ? t_n / (t_n + one_minus_t_n) : t;

  float gate = 1.0;
  if (${prefix}PerpSigma > 0.0) {
    float inv2s2 = 0.5 / (${prefix}PerpSigma * ${prefix}PerpSigma);
    vec3 perp = rel - proj * ${prefix}U;
    float dPerp2 = dot(perp, perp);
    gate = exp(-inv2s2 * dPerp2);
  }

  float v01 = mix(${prefix}BaseFallback, t_curved, gate);
  float outVal = mix(${prefix}VMin, ${prefix}VMax, v01);
  return clamp(outVal, 0.0, 1.0);
}
`;

export const buildReverseColorRampGenerator = (
  p: ReverseColorRampParams,
  fnName = 'reverse_color_ramp'
): string => {
  const colorSpace = p.colorSpace ?? 'srgb';
  const { A_lin, U, invLen, vMin, vMax, curveSteepness, curveOffset, perpSigma, baseFallback } =
    buildReverseColorRampUniformData(p);

  const n = Math.max(curveSteepness, 1);
  const sigma = Math.max(perpSigma, 0);
  const inv2s2 = sigma > 0 ? 0.5 / (sigma * sigma) : 0;

  return `
float ${fnName}(vec3 baseColor${colorSpace === 'srgb' ? '_srgb' : '_linear'}) {
  ${colorSpace === 'srgb' ? 'vec3 c = srgb_to_linear(baseColor_srgb);' : 'vec3 c = baseColor_linear;'}

  const vec3 A = vec3(${A_lin[0]}, ${A_lin[1]}, ${A_lin[2]});
  const vec3 U = vec3(${U[0]}, ${U[1]}, ${U[2]});
  const float invLen = ${invLen.toFixed(8)};
  const float vMin = ${vMin.toFixed(8)};
  const float vMax = ${vMax.toFixed(8)};
  const float n = ${n.toFixed(8)};
  const float curveOffset = ${curveOffset.toFixed(8)};
  const float baseVal = ${baseFallback.toFixed(8)};

  vec3 rel = c - A;
  float proj = dot(rel, U);

  float t = clamp(proj * invLen, 0.0, 1.0);

  // Shift t so that curveOffset is the midpoint
  if (curveOffset > 0.0 && curveOffset < 1.0) {
    t = clamp((t - curveOffset) / (1.0 - curveOffset), 0.0, 1.0);
  }

  float t_n = pow(t, n);
  float one_minus_t_n = pow(1.0 - t, n);
  float t_curved = (t_n + one_minus_t_n > 0.0) ? t_n / (t_n + one_minus_t_n) : t;

  ${
    sigma > 0
      ? `float dPerp2 = dot(rel - proj*U, rel - proj*U);
       float gate = exp(-${inv2s2} * dPerp2);
       float v01 = mix(baseVal, t_curved, gate);`
      : 'float v01 = t_curved;'
  }

  float outVal = mix(vMin, vMax, v01);
  return clamp(outVal, 0., 1.);
}
`;
};
