//! Noise-signature fit + synthesis, param spec v1 (dream/docs/noise-signature-plan.md).
//! Executable reference: texture-synth-experiments/scripts/spec_v1.py — keep in sync.

use std::collections::VecDeque;
use std::f64::consts::PI;

use rustfft::{FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Spec constant: band centers span cycles/px [1/N_SPEC, 0.5] regardless of the analysis
/// resolution, so fits at 256/512/1024 all emit portable params (frequencies below
/// 1/N_SPEC just land in the lowest band).
const N_SPEC: usize = 256;
const KR: usize = 8;
const KA: usize = 4;
const BAND_NATS: f64 = 14.0;
const SIG_RANGE: (f64, f64) = (-3.0, -0.5);
const EN_RANGE: (f64, f64) = (-4.0, 2.0);
const PEAK_MIN_WHITENESS: f64 = 2.0;

fn centers() -> [f64; KR] {
  let c0 = (1.0 / N_SPEC as f64).ln();
  let step = ((0.5f64).ln() - c0) / (KR - 1) as f64;
  core::array::from_fn(|i| c0 + step * i as f64)
}

fn band_step() -> f64 {
  ((0.5f64).ln() - (1.0 / N_SPEC as f64).ln()) / (KR - 1) as f64
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Kernel {
  pub f0: [f64; 2],
  pub sig: [f64; 2],
  pub angle: f64,
  pub energy: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SpectralParams {
  pub bands: Vec<Vec<f64>>,
  pub kernels: Vec<Kernel>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RampStop {
  pub pos: f64,
  pub rgb: [u8; 3],
}

/// Emit params as plain floats rounded to 3 decimals, clamped to the spec range.
fn r3(x: f64, lo: f64, hi: f64) -> f64 {
  (x.clamp(lo, hi) * 1e3).round() / 1e3 + 0.0
}

fn fftfreq(i: usize, n: usize) -> f64 {
  if i < n.div_ceil(2) { i as f64 / n as f64 } else { (i as f64 - n as f64) / n as f64 }
}

fn fft2(buf: &mut [Complex<f64>], h: usize, w: usize, inverse: bool) {
  let mut planner = FftPlanner::new();
  let row_fft = if inverse { planner.plan_fft_inverse(w) } else { planner.plan_fft_forward(w) };
  for row in buf.chunks_exact_mut(w) {
    row_fft.process(row);
  }
  let col_fft = if inverse { planner.plan_fft_inverse(h) } else { planner.plan_fft_forward(h) };
  let mut col = vec![Complex::default(); h];
  for x in 0..w {
    for y in 0..h {
      col[y] = buf[y * w + x];
    }
    col_fft.process(&mut col);
    for y in 0..h {
      buf[y * w + x] = col[y];
    }
  }
  if inverse {
    let s = 1.0 / (h * w) as f64;
    for v in buf.iter_mut() {
      *v *= s;
    }
  }
}

/// Moisan periodic+smooth decomposition; returns the periodic component.
fn periodic(u: &[f64], n: usize) -> Vec<f64> {
  let mut v = vec![Complex::new(0.0, 0.0); n * n];
  for x in 0..n {
    let d = u[(n - 1) * n + x] - u[x];
    v[x].re = d;
    v[(n - 1) * n + x].re = -d;
  }
  for y in 0..n {
    let d = u[y * n + n - 1] - u[y * n];
    v[y * n].re += d;
    v[y * n + n - 1].re -= d;
  }
  fft2(&mut v, n, n, false);
  for y in 0..n {
    for x in 0..n {
      let den = 2.0 * (2.0 * PI * y as f64 / n as f64).cos()
        + 2.0 * (2.0 * PI * x as f64 / n as f64).cos()
        - 4.0;
      if y == 0 && x == 0 {
        v[0] = Complex::new(0.0, 0.0);
      } else {
        v[y * n + x] /= den;
      }
    }
  }
  fft2(&mut v, n, n, true);
  (0..n * n).map(|i| u[i] - v[i].re).collect()
}

fn gauss_smooth_wrap(a: &[f64], n: usize) -> Vec<f64> {
  const R: isize = 3;
  let mut k = [0.0f64; 7];
  let mut ksum = 0.0;
  for (i, kv) in k.iter_mut().enumerate() {
    *kv = (-0.5 * (i as f64 - R as f64).powi(2)).exp();
    ksum += *kv;
  }
  for kv in k.iter_mut() {
    *kv /= ksum;
  }
  let mut tmp = vec![0.0f64; n * n];
  for y in 0..n {
    for x in 0..n {
      let mut acc = 0.0;
      for (i, kv) in k.iter().enumerate() {
        let yy = (y as isize + i as isize - R).rem_euclid(n as isize) as usize;
        acc += kv * a[yy * n + x];
      }
      tmp[y * n + x] = acc;
    }
  }
  let mut out = vec![0.0f64; n * n];
  for y in 0..n {
    for x in 0..n {
      let mut acc = 0.0;
      for (i, kv) in k.iter().enumerate() {
        let xx = (x as isize + i as isize - R).rem_euclid(n as isize) as usize;
        acc += kv * tmp[y * n + xx];
      }
      out[y * n + x] = acc;
    }
  }
  out
}

fn median(vals: &mut Vec<f64>) -> f64 {
  vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
  let m = vals.len();
  if m == 0 {
    return 0.0;
  }
  if m % 2 == 1 { vals[m / 2] } else { 0.5 * (vals[m / 2 - 1] + vals[m / 2]) }
}

pub fn luma_from_rgba(rgba: &[u8], px_count: usize) -> Vec<f64> {
  (0..px_count)
    .map(|i| {
      (0.2126 * rgba[i * 4] as f64 + 0.7152 * rgba[i * 4 + 1] as f64 + 0.0722 * rgba[i * 4 + 2] as f64)
        / 255.0
    })
    .collect()
}

pub fn fit_spectral(gray: &[f64], n: usize, max_kernels: usize) -> SpectralParams {
  assert_eq!(gray.len(), n * n);
  let up = periodic(gray, n);
  let mut fbuf: Vec<Complex<f64>> = up.iter().map(|&v| Complex::new(v, 0.0)).collect();
  fft2(&mut fbuf, n, n, false);
  let mut psd: Vec<f64> = fbuf.iter().map(|c| c.norm_sqr()).collect();
  psd[0] = 0.0;

  let cs = centers();
  let step = band_step();
  let idx_of = |y: usize, x: usize| y * n + x;
  let mut r = vec![0.0f64; n * n];
  let mut th = vec![0.0f64; n * n];
  let mut ri_round = vec![0usize; n * n];
  for y in 0..n {
    let fy = fftfreq(y, n);
    for x in 0..n {
      let fx = fftfreq(x, n);
      let i = idx_of(y, x);
      r[i] = (fx * fx + fy * fy).sqrt();
      th[i] = fy.atan2(fx).rem_euclid(PI);
      let lnr = r[i].max(1e-9).ln();
      // half-up rounding + eps: keeps bin assignment identical across float libms
      ri_round[i] = ((lnr - cs[0]) / step + 0.5 + 1e-9).floor().clamp(0.0, (KR - 1) as f64) as usize;
    }
  }

  let smoothed = gauss_smooth_wrap(&psd, n);
  let mut base_bands = [0.0f64; KR];
  for (k, bb) in base_bands.iter_mut().enumerate() {
    let mut vals: Vec<f64> =
      (0..n * n).filter(|&i| r[i] > 0.0 && ri_round[i] == k).map(|i| smoothed[i]).collect();
    *bb = median(&mut vals);
  }
  let baseline: Vec<f64> = (0..n * n).map(|i| base_bands[ri_round[i]]).collect();
  let peak_min_r = 2.5 / n as f64;
  let mut whiten: Vec<f64> = (0..n * n)
    .map(|i| {
      if r[i] > 0.0 && r[i] >= peak_min_r { smoothed[i] / baseline[i].max(1e-12) } else { 0.0 }
    })
    .collect();
  let half: Vec<bool> = (0..n * n)
    .map(|i| {
      let fy = fftfreq(i / n, n);
      let fx = fftfreq(i % n, n);
      fy > 0.0 || (fy == 0.0 && fx > 0.0)
    })
    .collect();
  let excess: Vec<f64> = (0..n * n).map(|i| (psd[i] - baseline[i]).max(0.0)).collect();

  struct RawKernel {
    f0: [f64; 2],
    sig_log10: [f64; 2],
    angle: f64,
    energy: f64,
  }
  let mut kernels: Vec<RawKernel> = Vec::new();
  let mut claimed = vec![false; n * n];
  for _ in 0..max_kernels {
    let mut peak_i = 0;
    let mut peak = 0.0;
    for i in 0..n * n {
      if half[i] && whiten[i] > peak {
        peak = whiten[i];
        peak_i = i;
      }
    }
    if peak < PEAK_MIN_WHITENESS {
      break;
    }
    let thresh = peak / 4.0;
    let mut comp = vec![false; n * n];
    let mut dq = VecDeque::new();
    comp[peak_i] = true;
    dq.push_back(peak_i);
    while let Some(i) = dq.pop_front() {
      let (y, x) = (i / n, i % n);
      for (dy, dx) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
        let yy = (y as isize + dy).rem_euclid(n as isize) as usize;
        let xx = (x as isize + dx).rem_euclid(n as isize) as usize;
        let j = idx_of(yy, xx);
        if whiten[j] >= thresh && !comp[j] {
          comp[j] = true;
          dq.push_back(j);
        }
      }
    }
    for i in 0..n * n {
      if comp[i] {
        claimed[i] = true;
        let refl = idx_of((n - i / n) % n, (n - i % n) % n);
        claimed[refl] = true;
      }
    }
    let mut wsum = 0.0;
    for i in 0..n * n {
      if comp[i] && half[i] {
        wsum += excess[i];
      }
    }
    if wsum <= 0.0 {
      for i in 0..n * n {
        if comp[i] {
          whiten[i] = 0.0;
        }
      }
      continue;
    }
    let (mut f0y, mut f0x) = (0.0, 0.0);
    for i in 0..n * n {
      if comp[i] && half[i] {
        f0y += fftfreq(i / n, n) * excess[i];
        f0x += fftfreq(i % n, n) * excess[i];
      }
    }
    f0y /= wsum;
    f0x /= wsum;
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    for i in 0..n * n {
      if comp[i] && half[i] {
        let dy = fftfreq(i / n, n) - f0y;
        let dx = fftfreq(i % n, n) - f0x;
        a += dy * dy * excess[i];
        b += dy * dx * excess[i];
        c += dx * dx * excess[i];
      }
    }
    let reg = (0.5 / n as f64).powi(2);
    a = a / wsum + reg;
    c = c / wsum + reg;
    b /= wsum;
    let disc = (((a - c) / 2.0).powi(2) + b * b).sqrt();
    let e1 = (a + c) / 2.0 + disc;
    let e2 = ((a + c) / 2.0 - disc).max(1e-12);
    let angle = (0.5 * (2.0 * b).atan2(a - c)).rem_euclid(PI);
    kernels.push(RawKernel {
      f0: [f0y, f0x],
      sig_log10: [0.5 * e1.log10(), 0.5 * e2.log10()],
      angle,
      energy: 2.0 * wsum,
    });
    for i in 0..n * n {
      if comp[i] {
        whiten[i] = 0.0;
      }
    }
  }

  let mut g = [[0.0f64; KA]; KR];
  let mut cnt = [[0usize; KA]; KR];
  for i in 0..n * n {
    if r[i] <= 0.0 {
      continue;
    }
    let resid = if claimed[i] { baseline[i] } else { psd[i] };
    // eps: diagonal bins land exactly on angular boundaries; nudge for cross-libm determinism
    let ai = ((th[i] / PI * KA as f64 + 1e-9).floor() as isize).rem_euclid(KA as isize) as usize;
    g[ri_round[i]][ai] += (resid + 1e-12).ln();
    cnt[ri_round[i]][ai] += 1;
  }
  for row in 0..KR {
    let filled: Vec<usize> = (0..KA).filter(|&j| cnt[row][j] > 0).collect();
    let row_mean = if filled.is_empty() {
      0.0
    } else {
      filled.iter().map(|&j| g[row][j] / cnt[row][j] as f64).sum::<f64>() / filled.len() as f64
    };
    for j in 0..KA {
      g[row][j] = if cnt[row][j] > 0 { g[row][j] / cnt[row][j] as f64 } else { row_mean };
    }
  }
  let gmax = g.iter().flatten().cloned().fold(f64::MIN, f64::max);
  let bands: Vec<Vec<f64>> = (0..KR)
    .map(|i| (0..KA).map(|j| r3(g[i][j] - gmax, -BAND_NATS, 0.0)).collect())
    .collect();

  let e_model = band_spectrum_sum(&bands, n, n) * gmax.exp();
  let out_kernels = kernels
    .into_iter()
    .map(|k| Kernel {
      f0: [(k.f0[0] * 1e6).round() / 1e6, (k.f0[1] * 1e6).round() / 1e6],
      sig: [
        r3(k.sig_log10[0], SIG_RANGE.0, SIG_RANGE.1),
        r3(k.sig_log10[1], SIG_RANGE.0, SIG_RANGE.1),
      ],
      angle: r3(k.angle, 0.0, PI),
      energy: r3((k.energy / e_model.max(1e-12)).max(1e-12).log10(), EN_RANGE.0, EN_RANGE.1),
    })
    .collect();

  SpectralParams { bands, kernels: out_kernels }
}

fn eval_bands_at(bands: &[Vec<f64>], lnr: f64, th: f64) -> f64 {
  let cs = centers();
  let step = band_step();
  let lnr = lnr.clamp(cs[0], cs[KR - 1]);
  let ri = (((lnr - cs[0]) / step).floor().clamp(0.0, (KR - 2) as f64)) as usize;
  let t = ((lnr - cs[ri]) / step).clamp(0.0, 1.0);
  let ap = th / PI * KA as f64 - 0.5;
  let a0 = (ap.floor() as isize).rem_euclid(KA as isize) as usize;
  let a1 = (a0 + 1) % KA;
  let at = ap - ap.floor();
  let gv = |i: usize, j: usize| bands[i][j];
  (gv(ri, a0) * (1.0 - t) * (1.0 - at)
    + gv(ri + 1, a0) * t * (1.0 - at)
    + gv(ri, a1) * (1.0 - t) * at
    + gv(ri + 1, a1) * t * at)
    .exp()
}

fn band_spectrum(bands: &[Vec<f64>], h: usize, w: usize) -> Vec<f64> {
  let mut s = vec![0.0f64; h * w];
  for y in 0..h {
    let fy = fftfreq(y, h);
    for x in 0..w {
      if y == 0 && x == 0 {
        continue;
      }
      let fx = fftfreq(x, w);
      let r = (fx * fx + fy * fy).sqrt();
      s[y * w + x] = eval_bands_at(bands, r.max(1e-9).ln(), fy.atan2(fx).rem_euclid(PI));
    }
  }
  s
}

fn band_spectrum_sum(bands: &[Vec<f64>], h: usize, w: usize) -> f64 {
  band_spectrum(bands, h, w).iter().sum()
}

struct SplitMix64(u64);
impl SplitMix64 {
  fn next_u64(&mut self) -> u64 {
    self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = self.0;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
  }

  fn next_f64(&mut self) -> f64 {
    (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
  }

  fn next_gaussian(&mut self) -> f64 {
    let u1 = self.next_f64().max(1e-300);
    let u2 = self.next_f64();
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
  }
}

fn erf(x: f64) -> f64 {
  // Abramowitz & Stegun 7.1.26
  let sign = if x < 0.0 { -1.0 } else { 1.0 };
  let x = x.abs();
  let t = 1.0 / (1.0 + 0.3275911 * x);
  let y = 1.0
    - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592)
      * t
      * (-x * x).exp();
  sign * y
}

/// Inverse normal CDF (Acklam's rational approximation, ~1e-9 abs error).
fn probit(p: f64) -> f64 {
  const A: [f64; 6] = [
    -3.969683028665376e+01,
    2.209460984245205e+02,
    -2.759285104469687e+02,
    1.383577518672690e+02,
    -3.066479806614716e+01,
    2.506628277459239e+00,
  ];
  const B: [f64; 5] = [
    -5.447609879822406e+01,
    1.615858368580409e+02,
    -1.556989798598866e+02,
    6.680131188771972e+01,
    -1.328068155288572e+01,
  ];
  const C: [f64; 6] = [
    -7.784894002430293e-03,
    -3.223964580411365e-01,
    -2.400758277161838e+00,
    -2.549732539343734e+00,
    4.374664141464968e+00,
    2.938163982698783e+00,
  ];
  const D: [f64; 4] = [
    7.784695709041462e-03,
    3.224671290700398e-01,
    2.445134137142996e+00,
    3.754408661907416e+00,
  ];
  let p = p.clamp(1e-12, 1.0 - 1e-12);
  if p < 0.02425 {
    let q = (-2.0 * p.ln()).sqrt();
    (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
      / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
  } else if p > 1.0 - 0.02425 {
    -probit(1.0 - p)
  } else {
    let q = p - 0.5;
    let r = q * q;
    (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
      / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
  }
}

/// Synthesize a seamless field of shape (h, w): standardized Gaussian (mean 0, std 1).
pub fn synth_field(params: &SpectralParams, h: usize, w: usize, seed: u32) -> Vec<f64> {
  let mut s = band_spectrum(&params.bands, h, w);
  let e_resid: f64 = s.iter().sum();
  for k in &params.kernels {
    let s1 = 10.0f64.powf(k.sig[0]);
    let s2 = 10.0f64.powf(k.sig[1]);
    let ang = k.angle;
    let ratio = 10.0f64.powf(k.energy);
    let (ca, sa) = (ang.cos(), ang.sin());
    let i11 = ca * ca / (s1 * s1) + sa * sa / (s2 * s2);
    let i22 = sa * sa / (s1 * s1) + ca * ca / (s2 * s2);
    let i12 = ca * sa * (1.0 / (s1 * s1) - 1.0 / (s2 * s2));
    let mut lobe = vec![0.0f64; h * w];
    let mut lsum = 0.0;
    for y in 0..h {
      let fy = fftfreq(y, h);
      for x in 0..w {
        if y == 0 && x == 0 {
          continue;
        }
        let fx = fftfreq(x, w);
        let mut v = 0.0;
        for sgn in [1.0f64, -1.0] {
          let dy = (fy - sgn * k.f0[0] + 0.5).rem_euclid(1.0) - 0.5;
          let dx = (fx - sgn * k.f0[1] + 0.5).rem_euclid(1.0) - 0.5;
          v += (-0.5 * (i11 * dy * dy + 2.0 * i12 * dy * dx + i22 * dx * dx)).exp();
        }
        lobe[y * w + x] = v;
        lsum += v;
      }
    }
    if lsum > 0.0 {
      let scale = ratio * e_resid / lsum;
      for i in 0..h * w {
        s[i] += lobe[i] * scale;
      }
    }
  }

  let mut rng = SplitMix64(0x243F6A8885A308D3u64 ^ (seed as u64).wrapping_mul(0x9E3779B97F4A7C15));
  let mut buf: Vec<Complex<f64>> =
    (0..h * w).map(|_| Complex::new(rng.next_gaussian(), 0.0)).collect();
  fft2(&mut buf, h, w, false);
  for i in 0..h * w {
    buf[i] *= s[i].sqrt();
  }
  buf[0] = Complex::new(0.0, 0.0);
  fft2(&mut buf, h, w, true);
  let field: Vec<f64> = buf.iter().map(|c| c.re).collect();
  let mean = field.iter().sum::<f64>() / field.len() as f64;
  let var = field.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / field.len() as f64;
  let std = var.sqrt().max(1e-12);
  field.iter().map(|v| (v - mean) / std).collect()
}

// ---------- texton ----------

/// Alternating-projection texton fit: the ksize² kernel whose scatter reproduces the
/// exemplar's spectrum. Iterates {match target magnitude spectrum, restrict spatial
/// support}; returns the fftshifted center crop.
pub fn fit_texton(gray: &[f64], n: usize, ksize: usize, seed: u32) -> Vec<f64> {
  const ITERS: usize = 120;
  assert_eq!(gray.len(), n * n);
  let up = periodic(gray, n);
  let mut fbuf: Vec<Complex<f64>> = up.iter().map(|&v| Complex::new(v, 0.0)).collect();
  fft2(&mut fbuf, n, n, false);
  let mut target: Vec<f64> = fbuf.iter().map(|c| c.norm()).collect();
  target[0] = 0.0;

  let h = ksize / 2;
  let in_support = |y: usize, x: usize| (y < h || y >= n - h) && (x < h || x >= n - h);

  let mut rng = SplitMix64(0x6A09E667F3BCC908u64 ^ (seed as u64).wrapping_mul(0x9E3779B97F4A7C15));
  let mut buf: Vec<Complex<f64>> = (0..n * n).map(|_| Complex::new(rng.next_gaussian(), 0.0)).collect();
  fft2(&mut buf, n, n, false);
  for (i, v) in buf.iter_mut().enumerate() {
    let a = v.norm().max(1e-300);
    *v = *v / a * target[i];
  }
  fft2(&mut buf, n, n, true);
  let mut hcur: Vec<f64> = (0..n * n)
    .map(|i| if in_support(i / n, i % n) { buf[i].re } else { 0.0 })
    .collect();

  for _ in 0..ITERS {
    let mut hf: Vec<Complex<f64>> = hcur.iter().map(|&v| Complex::new(v, 0.0)).collect();
    fft2(&mut hf, n, n, false);
    for (i, v) in hf.iter_mut().enumerate() {
      let a = v.norm();
      let a = if a == 0.0 { 1.0 } else { a };
      *v = *v / a * target[i];
    }
    fft2(&mut hf, n, n, true);
    for i in 0..n * n {
      hcur[i] = if in_support(i / n, i % n) { hf[i].re } else { 0.0 };
    }
  }

  let mut kern = Vec::with_capacity(ksize * ksize);
  for j in 0..ksize {
    for i in 0..ksize {
      let y = (j + n - h) % n;
      let x = (i + n - h) % n;
      kern.push(hcur[y * n + x]);
    }
  }
  kern
}

#[derive(Serialize)]
struct TextonFit {
  kernel_u8: Vec<u8>,
  ksize: usize,
  scale: f64,
  offset: f64,
  mean_h2: f64,
}

/// Signed ±1 dense scatter of the kernel, toroidal, with the analytic normalization
/// (var = coverage · mean(h²)) — mirrors the emitted geoscript composition exactly.
pub fn texton_field(
  kern: &[f64],
  ksize: usize,
  w: usize,
  h: usize,
  seed: u32,
  coverage: f64,
) -> Vec<f64> {
  let mean_h2 = kern.iter().map(|v| v * v).sum::<f64>() / (ksize * ksize) as f64;
  let count = (coverage * (w * h) as f64 / (ksize * ksize) as f64) as usize;
  let mut rng = SplitMix64(0xB7E151628AED2A6Bu64 ^ (seed as u64).wrapping_mul(0x9E3779B97F4A7C15));
  let mut field = vec![0.0f64; w * h];
  for _ in 0..count {
    let x0 = (rng.next_u64() % w as u64) as usize;
    let y0 = (rng.next_u64() % h as u64) as usize;
    let sign = if rng.next_f64() < 0.5 { -1.0 } else { 1.0 };
    for j in 0..ksize {
      let y = (y0 + j) % h;
      for i in 0..ksize {
        let x = (x0 + i) % w;
        field[y * w + x] += sign * kern[j * ksize + i];
      }
    }
  }
  let norm = 1.0 / (coverage * mean_h2).sqrt().max(1e-300);
  for v in field.iter_mut() {
    *v *= norm;
  }
  field
}

// ---------- Oklab + ramp ----------

fn srgb_to_linear(c: f64) -> f64 {
  if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f64) -> f64 {
  let c = c.max(0.0);
  if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn srgb_to_oklab(rgb: [f64; 3]) -> [f64; 3] {
  let [r, g, b] = rgb.map(srgb_to_linear);
  let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
  let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
  let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
  [
    0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
  ]
}

fn oklab_to_srgb(lab: [f64; 3]) -> [f64; 3] {
  let l = lab[0] + 0.3963377774 * lab[1] + 0.2158037573 * lab[2];
  let m = lab[0] - 0.1055613458 * lab[1] - 0.0638541728 * lab[2];
  let s = lab[0] - 0.0894841775 * lab[1] - 1.2914855480 * lab[2];
  let (l, m, s) = (l * l * l, m * m * m, s * s * s);
  [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
  ]
  .map(|c| linear_to_srgb(c).clamp(0.0, 1.0))
}

pub fn fit_ramp(rgba: &[u8], px_count: usize, n_stops: usize) -> Vec<RampStop> {
  const BINS: usize = 64;
  let lum = luma_from_rgba(rgba, px_count);
  let mut order: Vec<usize> = (0..px_count).collect();
  order.sort_by(|&a, &b| lum[a].partial_cmp(&lum[b]).unwrap().then(a.cmp(&b)));
  let mut bin_lab = [[0.0f64; 3]; BINS];
  let mut bin_n = [0usize; BINS];
  for (rank, &i) in order.iter().enumerate() {
    let b = (rank * BINS / px_count).min(BINS - 1);
    let lab = srgb_to_oklab([
      rgba[i * 4] as f64 / 255.0,
      rgba[i * 4 + 1] as f64 / 255.0,
      rgba[i * 4 + 2] as f64 / 255.0,
    ]);
    for c in 0..3 {
      bin_lab[b][c] += lab[c];
    }
    bin_n[b] += 1;
  }
  for b in 0..BINS {
    for c in 0..3 {
      bin_lab[b][c] /= bin_n[b].max(1) as f64;
    }
  }
  let bin_pos: Vec<f64> = (0..BINS).map(|b| (b as f64 + 0.5) / BINS as f64).collect();

  let mut stops: Vec<usize> = vec![0, BINS - 1];
  while stops.len() < n_stops.clamp(2, BINS) {
    let interp = |q: f64| -> [f64; 3] {
      let mut seg = 0;
      while seg + 2 < stops.len() && q > bin_pos[stops[seg + 1]] {
        seg += 1;
      }
      let (p0, p1) = (bin_pos[stops[seg]], bin_pos[stops[seg + 1]]);
      let t = ((q - p0) / (p1 - p0)).clamp(0.0, 1.0);
      core::array::from_fn(|c| bin_lab[stops[seg]][c] * (1.0 - t) + bin_lab[stops[seg + 1]][c] * t)
    };
    let mut best = (0usize, 0.0f64);
    for b in 0..BINS {
      if stops.contains(&b) {
        continue;
      }
      let pred = interp(bin_pos[b]);
      let err = (0..3).map(|c| (pred[c] - bin_lab[b][c]).powi(2)).sum::<f64>().sqrt();
      if err > best.1 {
        best = (b, err);
      }
    }
    if best.1 == 0.0 {
      break;
    }
    stops.push(best.0);
    stops.sort();
  }

  // z-score positions: ramps apply directly to the Gaussian fields synth_field emits,
  // including equal-power seed crossfades
  stops
    .iter()
    .map(|&b| {
      let srgb = oklab_to_srgb(bin_lab[b]);
      RampStop {
        pos: (probit(bin_pos[b]) * 1e3).round() / 1e3,
        rgb: srgb.map(|c| (c * 255.0).round() as u8),
      }
    })
    .collect()
}

fn apply_ramp(field: &[f64], ramp: &[RampStop]) -> Vec<u8> {
  let labs: Vec<[f64; 3]> =
    ramp.iter().map(|s| srgb_to_oklab(s.rgb.map(|c| c as f64 / 255.0))).collect();
  let mut out = Vec::with_capacity(field.len() * 4);
  for &u in field {
    let q = u.clamp(ramp[0].pos, ramp[ramp.len() - 1].pos);
    let mut seg = 0;
    while seg + 2 < ramp.len() && q > ramp[seg + 1].pos {
      seg += 1;
    }
    let (p0, p1) = (ramp[seg].pos, ramp[seg + 1].pos);
    let t = if p1 > p0 { ((q - p0) / (p1 - p0)).clamp(0.0, 1.0) } else { 0.0 };
    let lab: [f64; 3] = core::array::from_fn(|c| labs[seg][c] * (1.0 - t) + labs[seg + 1][c] * t);
    let srgb = oklab_to_srgb(lab);
    out.extend_from_slice(&[
      (srgb[0] * 255.0).round() as u8,
      (srgb[1] * 255.0).round() as u8,
      (srgb[2] * 255.0).round() as u8,
      255,
    ]);
  }
  out
}

// ---------- wasm exports ----------

/// Remove low-frequency lighting (gradients, vignettes, partial shadows) before fitting:
/// subtracts a heavy Gaussian blur of the luminance from every channel, preserving the
/// global mean. `flatten_sigma` is the blur sigma in pixels; <= 0 is a no-op.
#[wasm_bindgen]
pub fn ns_preprocess(rgba: &[u8], size: usize, flatten_sigma: f64) -> Vec<u8> {
  console_error_panic_hook::set_once();
  assert_eq!(rgba.len(), size * size * 4);
  if flatten_sigma <= 0.0 {
    return rgba.to_vec();
  }
  let lum = luma_from_rgba(rgba, size * size);

  let radius = (flatten_sigma * 3.0).ceil() as isize;
  let mut kernel = Vec::with_capacity((radius * 2 + 1) as usize);
  let mut ksum = 0.0;
  for i in -radius..=radius {
    let v = (-0.5 * (i as f64 / flatten_sigma).powi(2)).exp();
    kernel.push(v);
    ksum += v;
  }
  for v in &mut kernel {
    *v /= ksum;
  }
  let blur_pass = |src: &[f64], vertical: bool| -> Vec<f64> {
    let mut out = vec![0.0; size * size];
    for y in 0..size {
      for x in 0..size {
        let mut acc = 0.0;
        for (ki, kv) in kernel.iter().enumerate() {
          let o = ki as isize - radius;
          let (sx, sy) = if vertical {
            (x as isize, (y as isize + o).clamp(0, size as isize - 1))
          } else {
            ((x as isize + o).clamp(0, size as isize - 1), y as isize)
          };
          acc += kv * src[sy as usize * size + sx as usize];
        }
        out[y * size + x] = acc;
      }
    }
    out
  };
  let blur = blur_pass(&blur_pass(&lum, false), true);
  let mean_b = blur.iter().sum::<f64>() / blur.len() as f64;

  let mut out = rgba.to_vec();
  for i in 0..size * size {
    let delta = (mean_b - blur[i]) * 255.0;
    for c in 0..3 {
      out[i * 4 + c] = (rgba[i * 4 + c] as f64 + delta).round().clamp(0.0, 255.0) as u8;
    }
  }
  out
}

#[derive(Serialize)]
struct AnalyzeResult {
  params: SpectralParams,
  ramp: Vec<RampStop>,
}

#[wasm_bindgen]
pub fn ns_analyze(rgba: &[u8], size: usize, max_kernels: usize, ramp_stops: usize) -> String {
  console_error_panic_hook::set_once();
  assert!([256, 512, 1024].contains(&size), "analysis size must be 256/512/1024");
  assert_eq!(rgba.len(), size * size * 4);
  let gray = luma_from_rgba(rgba, size * size);
  let params = fit_spectral(&gray, size, max_kernels.min(4));
  let ramp = fit_ramp(rgba, size * size, ramp_stops);
  serde_json::to_string(&AnalyzeResult { params, ramp }).unwrap()
}

/// Fit a texton kernel to the (preprocessed) exemplar and quantize it for export:
/// value = u8/255 · scale + offset. `mean_h2` is computed on the DEQUANTIZED kernel so
/// the analytic normalization matches what the emitted geoscript synthesizes.
#[wasm_bindgen]
pub fn ns_fit_texton(rgba: &[u8], size: usize, ksize: usize, seed: u32) -> String {
  console_error_panic_hook::set_once();
  assert!([256, 512, 1024].contains(&size), "analysis size must be 256/512/1024");
  assert!(ksize.is_power_of_two() && (8..=128).contains(&ksize) && ksize < size);
  let gray = luma_from_rgba(rgba, size * size);
  let kern = fit_texton(&gray, size, ksize, seed);
  let kmin = kern.iter().cloned().fold(f64::MAX, f64::min);
  let kmax = kern.iter().cloned().fold(f64::MIN, f64::max);
  let scale = (kmax - kmin).max(1e-12);
  let kernel_u8: Vec<u8> =
    kern.iter().map(|&v| (((v - kmin) / scale * 255.0).round().clamp(0.0, 255.0)) as u8).collect();
  let mean_h2 = kernel_u8
    .iter()
    .map(|&q| {
      let v = q as f64 / 255.0 * scale + kmin;
      v * v
    })
    .sum::<f64>()
    / (ksize * ksize) as f64;
  serde_json::to_string(&TextonFit { kernel_u8, ksize, scale, offset: kmin, mean_h2 }).unwrap()
}

#[wasm_bindgen]
pub fn ns_texton_preview(
  kernel_u8: &[u8],
  ksize: usize,
  scale: f64,
  offset: f64,
  w: usize,
  h: usize,
  seed: u32,
  coverage: f64,
  ramp_json: &str,
  grayscale: bool,
) -> Vec<u8> {
  console_error_panic_hook::set_once();
  let kern: Vec<f64> = kernel_u8.iter().map(|&q| q as f64 / 255.0 * scale + offset).collect();
  let field = texton_field(&kern, ksize, w, h, seed, coverage);
  if grayscale {
    let mut out = Vec::with_capacity(field.len() * 4);
    for &z in &field {
      let u = 0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2));
      let v = (u * 255.0).round() as u8;
      out.extend_from_slice(&[v, v, v, 255]);
    }
    out
  } else {
    let ramp: Vec<RampStop> = serde_json::from_str(ramp_json).unwrap();
    apply_ramp(&field, &ramp)
  }
}

/// Raw standardized-Gaussian field (row-major z-scores) for client-side coloring.
#[wasm_bindgen]
pub fn ns_field(params_json: &str, w: usize, h: usize, seed: u32) -> Vec<f32> {
  console_error_panic_hook::set_once();
  let params: SpectralParams = serde_json::from_str(params_json).unwrap();
  synth_field(&params, h, w, seed).into_iter().map(|v| v as f32).collect()
}

#[wasm_bindgen]
pub fn ns_preview(
  params_json: &str,
  ramp_json: &str,
  w: usize,
  h: usize,
  seed: u32,
  grayscale: bool,
) -> Vec<u8> {
  console_error_panic_hook::set_once();
  let params: SpectralParams = serde_json::from_str(params_json).unwrap();
  let field = synth_field(&params, h, w, seed);
  if grayscale {
    let mut out = Vec::with_capacity(field.len() * 4);
    for &z in &field {
      let u = 0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2));
      let v = (u * 255.0).round() as u8;
      out.extend_from_slice(&[v, v, v, 255]);
    }
    out
  } else {
    let ramp: Vec<RampStop> = serde_json::from_str(ramp_json).unwrap();
    apply_ramp(&field, &ramp)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn matches_python_reference() {
    let rgba = include_bytes!("../fixtures/ex10_rgba.bin");
    let expected: SpectralParams =
      serde_json::from_str(include_str!("../fixtures/ex10_params.json")).unwrap();
    let gray = luma_from_rgba(rgba, 256 * 256);
    let got = fit_spectral(&gray, 256, 4);
    for i in 0..KR {
      for j in 0..KA {
        let (a, b) = (got.bands[i][j], expected.bands[i][j]);
        assert!((a - b).abs() <= 0.2, "band[{i}][{j}]: {a} vs {b}");
      }
    }
    assert_eq!(got.kernels.len(), expected.kernels.len());
    for (g, e) in got.kernels.iter().zip(&expected.kernels) {
      assert!((g.f0[0] - e.f0[0]).abs() < 0.01, "f0y {} vs {}", g.f0[0], e.f0[0]);
      assert!((g.f0[1] - e.f0[1]).abs() < 0.01, "f0x {} vs {}", g.f0[1], e.f0[1]);
      assert!((g.energy - e.energy).abs() <= 0.25);
      assert!((g.sig[0] - e.sig[0]).abs() <= 0.1);
      assert!((g.sig[1] - e.sig[1]).abs() <= 0.1);
      let da = (g.angle - e.angle).rem_euclid(PI).min((e.angle - g.angle).rem_euclid(PI));
      assert!(da <= 0.15, "angle {} vs {}", g.angle, e.angle);
    }
  }

  /// Fit at 512 recovers the band structure of a field synthesized from known params
  /// (band centers are spec constants, so params are resolution-portable).
  #[test]
  fn fit_at_512_recovers_bands() {
    let params: SpectralParams =
      serde_json::from_str(include_str!("../fixtures/ex10_params.json")).unwrap();
    let field = synth_field(&params, 512, 512, 11);
    let gray: Vec<f64> = field.iter().map(|z| 0.5 + 0.15 * z).collect();
    let got = fit_spectral(&gray, 512, 0);
    // skip the two lowest bands: at 1/256 c/px a 512 grid has only a handful of bins
    for i in 2..KR {
      for j in 0..KA {
        let (a, b) = (got.bands[i][j], params.bands[i][j]);
        assert!((a - b).abs() < 1.5, "band[{i}][{j}]: {a} vs {b}");
      }
    }
    assert_eq!(fit_spectral(&gray, 512, 0).bands, got.bands);
  }

  #[test]
  fn texton_fit_and_field() {
    let rgba = include_bytes!("../fixtures/ex10_rgba.bin");
    let gray = luma_from_rgba(rgba, 256 * 256);
    let kern = fit_texton(&gray, 256, 16, 0);
    assert_eq!(kern.len(), 256);
    let energy: f64 = kern.iter().map(|v| v * v).sum();
    assert!(energy > 0.0 && energy.is_finite());
    assert_eq!(fit_texton(&gray, 256, 16, 0), kern, "fit must be deterministic");

    let field = texton_field(&kern, 16, 128, 128, 3, 20.0);
    let n = field.len() as f64;
    let mean = field.iter().sum::<f64>() / n;
    let var = field.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    assert!(mean.abs() < 0.15, "mean {mean}");
    assert!((var.sqrt() - 1.0).abs() < 0.25, "std {}", var.sqrt());
  }

  #[test]
  fn synth_is_standardized_gaussian() {
    let expected: SpectralParams =
      serde_json::from_str(include_str!("../fixtures/ex10_params.json")).unwrap();
    let f = synth_field(&expected, 128, 256, 7);
    assert_eq!(f.len(), 128 * 256);
    let mean = f.iter().sum::<f64>() / f.len() as f64;
    let var = f.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / f.len() as f64;
    assert!(mean.abs() < 1e-9, "mean {mean}");
    assert!((var.sqrt() - 1.).abs() < 1e-9, "std {}", var.sqrt());
    assert!((probit(0.975) - 1.959964).abs() < 1e-4);
  }
}
