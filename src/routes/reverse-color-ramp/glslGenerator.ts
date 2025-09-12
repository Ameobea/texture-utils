type RampParams = {
  colorA_srgb: [number, number, number];
  colorB_srgb: [number, number, number];
  vMin: number;
  vMax: number;
  gamma?: number;
  softKnee?: { a: number; b: number };
  perpSigma?: number;
  baseFallback?: number;
};

function srgbToLinear(c: number): number {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

export function generateRoughnessGLSL(p: RampParams): string {
  const A_lin = p.colorA_srgb.map(srgbToLinear) as [number, number, number];
  const B_lin = p.colorB_srgb.map(srgbToLinear) as [number, number, number];

  const ux = B_lin[0] - A_lin[0];
  const uy = B_lin[1] - A_lin[1];
  const uz = B_lin[2] - A_lin[2];
  const len = Math.sqrt(ux*ux + uy*uy + uz*uz) || 1.0;

  const u = [ux/len, uy/len, uz/len] as const;
  const invLen = 1.0 / len;

  const gamma = p.gamma ?? 1.0;
  const hasKnee = !!p.softKnee;
  const a = p.softKnee?.a ?? 0.0;
  const b = p.softKnee?.b ?? 1.0;

  const sigma = Math.max(p.perpSigma ?? 0.0, 0.0);
  const inv2s2 = sigma > 0 ? (0.5 / (sigma * sigma)) : 0.0;

  const vMin = p.vMin;
  const vMax = p.vMax;
  const base = p.baseFallback ?? vMin;

  return `
vec3 srgb_to_linear(vec3 c) {
  vec3 lo = c / 12.92;
  vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
  bvec3 useLo = lessThanEqual(c, vec3(0.04045));
  return vec3(useLo.x ? lo.x : hi.x, useLo.y ? lo.y : hi.y, useLo.z ? lo.z : hi.z);
}

float roughness_from_color(vec3 baseColor_srgb) {
  vec3 c = srgb_to_linear(baseColor_srgb);

  const vec3 A = vec3(${A_lin[0]}, ${A_lin[1]}, ${A_lin[2]});
  const vec3 U = vec3(${u[0]}, ${u[1]}, ${u[2]});
  const float invLen = ${invLen};
  const float vMin = ${vMin};
  const float vMax = ${vMax};
  const float gamma = ${gamma};
  const float baseVal = ${base};

  vec3 rel = c - A;
  float proj = dot(rel, U);
  float t = clamp(proj * invLen, 0.0, 1.0);

  float tCurved = ${hasKnee
    ? `smoothstep(${a}, ${b}, t)`
    : `pow(max(t, 1e-6), gamma)`};

  ${sigma > 0.0
    ? `float dPerp2 = dot(rel - proj*U, rel - proj*U);
       float gate = exp(-${inv2s2} * dPerp2);
       float v01 = mix(baseVal, tCurved, gate);`
    : `float v01 = tCurved;`}

  float outVal = mix(vMin, vMax, v01);
  return clamp(outVal, 0.0, 1.0);
}
`;
}
