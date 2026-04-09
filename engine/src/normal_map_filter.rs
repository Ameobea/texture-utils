#[cfg(target_arch = "wasm32")]
use core::arch::wasm32::*;

use wasm_bindgen::prelude::*;

/// Decode a tangent-space normal channel from [0, 255] to [-1, 1]
fn decode_channel(v: u8) -> f32 { (v as f32 / 255.0) * 2.0 - 1.0 }

/// Encode a tangent-space normal channel from [-1, 1] to [0, 255]
fn encode_channel(v: f32) -> u8 { ((v * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8 }

/// Build a 1D Gaussian kernel with the given sigma.
/// Kernel radius = ceil(3 * sigma), total size = 2 * radius + 1.
fn build_gaussian_kernel(sigma: f32) -> Vec<f32> {
  let radius = (3.0 * sigma).ceil() as usize;
  if radius == 0 {
    return vec![1.0];
  }

  let size = 2 * radius + 1;
  let mut kernel = Vec::with_capacity(size);
  let two_sigma_sq = 2.0 * sigma * sigma;
  let mut sum = 0.0;

  for i in 0..size {
    let x = i as f32 - radius as f32;
    let val = (-x * x / two_sigma_sq).exp();
    kernel.push(val);
    sum += val;
  }

  // Normalize
  for v in &mut kernel {
    *v /= sum;
  }

  kernel
}

/// Separable Gaussian blur on a single-channel f32 image with wrap-around boundary.
fn gaussian_blur_channel(data: &[f32], width: usize, height: usize, kernel: &[f32]) -> Vec<f32> {
  let radius = kernel.len() / 2;
  let mut temp = vec![0.0f32; width * height];
  let mut output = vec![0.0f32; width * height];

  // Horizontal pass
  for y in 0..height {
    for x in 0..width {
      let mut sum = 0.0;
      for k in 0..kernel.len() {
        let sx = (x as isize + k as isize - radius as isize).rem_euclid(width as isize) as usize;
        sum += data[y * width + sx] * kernel[k];
      }
      temp[y * width + x] = sum;
    }
  }

  // Vertical pass
  for y in 0..height {
    for x in 0..width {
      let mut sum = 0.0;
      for k in 0..kernel.len() {
        let sy = (y as isize + k as isize - radius as isize).rem_euclid(height as isize) as usize;
        sum += temp[sy * width + x] * kernel[k];
      }
      output[y * width + x] = sum;
    }
  }

  output
}

/// Precompute normalized normals in SoA layout for SIMD-friendly access.
fn precompute_normals_soa(chan_x: &[f32], chan_y: &[f32]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
  let pixel_count = chan_x.len();
  let mut norm_x = Vec::with_capacity(pixel_count);
  let mut norm_y = Vec::with_capacity(pixel_count);
  let mut norm_z = Vec::with_capacity(pixel_count);

  for i in 0..pixel_count {
    let x = chan_x[i];
    let y = chan_y[i];
    let z = (1.0 - x * x - y * y).max(0.0).sqrt();
    let len = (x * x + y * y + z * z).sqrt();
    if len > 1e-8 {
      norm_x.push(x / len);
      norm_y.push(y / len);
      norm_z.push(z / len);
    } else {
      norm_x.push(0.0);
      norm_y.push(0.0);
      norm_z.push(1.0);
    }
  }

  (norm_x, norm_y, norm_z)
}

/// Precompute 2D spatial Gaussian weights for the kernel window.
fn precompute_spatial_weights(radius: usize, two_spatial_sq: f32) -> Vec<f32> {
  let kernel_size = 2 * radius + 1;
  let mut weights = Vec::with_capacity(kernel_size * kernel_size);

  for ky in 0..kernel_size {
    for kx in 0..kernel_size {
      let dx = kx as f32 - radius as f32;
      let dy = ky as f32 - radius as f32;
      weights.push((-(dx * dx + dy * dy) / two_spatial_sq).exp());
    }
  }

  weights
}

/// Fast exp approximation using Schraudolph's method, vectorized for f32x4.
/// Accurate to ~5% for the range [-87, 0] — plenty for Gaussian weights.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
fn fast_exp_f32x4(x: v128) -> v128 {
  // exp(x) ≈ reinterpret_as_f32(trunc_to_i32(x * (2^23 / ln2) + 127 * 2^23))
  let a = f32x4_splat(12102203.16f32);
  let b = f32x4_splat(1065353216.0f32);
  let min_val = f32x4_splat(-87.0);

  let x_clamped = f32x4_max(x, min_val);
  let val = f32x4_add(f32x4_mul(x_clamped, a), b);

  // Truncate float to i32; the resulting bit pattern, reinterpreted as f32,
  // is the Schraudolph approximation. WASM SIMD v128 is untyped, so
  // subsequent f32x4 ops on this value perform the reinterpret for free.
  i32x4_trunc_sat_f32x4(val)
}

/// SIMD-optimized bilateral blur processing 4 center pixels at a time.
#[cfg(target_arch = "wasm32")]
unsafe fn bilateral_blur_simd_row(
  cx_start: usize,
  cx_end: usize,
  cy: usize,
  width: usize,
  height: usize,
  radius: usize,
  kernel_size: usize,
  spatial_weights: &[f32],
  neg_inv_two_range_sq: f32,
  chan_x: &[f32],
  chan_y: &[f32],
  norm_x: &[f32],
  norm_y: &[f32],
  norm_z: &[f32],
  out_x: &mut [f32],
  out_y: &mut [f32],
) {
  unsafe {
    let one = f32x4_splat(1.0);
    let range_factor = f32x4_splat(neg_inv_two_range_sq);

    let mut cx = cx_start;
    while cx + 4 <= cx_end {
      let center_base = cy * width + cx;

      // Load center normals (4 consecutive pixels)
      let cn_x = v128_load(norm_x.as_ptr().add(center_base) as *const v128);
      let cn_y = v128_load(norm_y.as_ptr().add(center_base) as *const v128);
      let cn_z = v128_load(norm_z.as_ptr().add(center_base) as *const v128);

      let mut sum_x = f32x4_splat(0.0);
      let mut sum_y = f32x4_splat(0.0);
      let mut w_sum = f32x4_splat(0.0);

      for ky in 0..kernel_size {
        let dy = ky as isize - radius as isize;
        let sy = (cy as isize + dy).rem_euclid(height as isize) as usize;

        for kx in 0..kernel_size {
          let dx = kx as isize - radius as isize;
          // cx is in [radius, width - radius - 4), so cx + dx is in bounds
          let neighbor_base = sy * width + (cx as isize + dx) as usize;

          // Load 4 neighbor normals (consecutive in x)
          let nn_x = v128_load(norm_x.as_ptr().add(neighbor_base) as *const v128);
          let nn_y = v128_load(norm_y.as_ptr().add(neighbor_base) as *const v128);
          let nn_z = v128_load(norm_z.as_ptr().add(neighbor_base) as *const v128);

          // dot = cn·nn
          let dot = f32x4_add(
            f32x4_add(f32x4_mul(cn_x, nn_x), f32x4_mul(cn_y, nn_y)),
            f32x4_mul(cn_z, nn_z),
          );

          // range_w = exp(-(1 - dot)^2 / (2 * range_sigma^2))
          let range_diff = f32x4_sub(one, dot);
          let rd_sq = f32x4_mul(range_diff, range_diff);
          let exp_arg = f32x4_mul(rd_sq, range_factor);
          let range_w = fast_exp_f32x4(exp_arg);

          // Combined weight = precomputed spatial × range
          let spatial = f32x4_splat(spatial_weights[ky * kernel_size + kx]);
          let w = f32x4_mul(spatial, range_w);

          // Weighted accumulation of channel data
          let nx = v128_load(chan_x.as_ptr().add(neighbor_base) as *const v128);
          let ny = v128_load(chan_y.as_ptr().add(neighbor_base) as *const v128);

          sum_x = f32x4_add(sum_x, f32x4_mul(nx, w));
          sum_y = f32x4_add(sum_y, f32x4_mul(ny, w));
          w_sum = f32x4_add(w_sum, w);
        }
      }

      // Normalize and store
      let rx = f32x4_div(sum_x, w_sum);
      let ry = f32x4_div(sum_y, w_sum);
      v128_store(out_x.as_mut_ptr().add(center_base) as *mut v128, rx);
      v128_store(out_y.as_mut_ptr().add(center_base) as *mut v128, ry);

      cx += 4;
    }
  }
}

/// Scalar bilateral blur for edge pixels that can't use SIMD
/// (those within `radius` of the image boundary where neighbors wrap).
fn bilateral_blur_scalar_pixel(
  cx: usize,
  cy: usize,
  width: usize,
  height: usize,
  radius: usize,
  kernel_size: usize,
  spatial_weights: &[f32],
  neg_inv_two_range_sq: f32,
  chan_x: &[f32],
  chan_y: &[f32],
  norm_x: &[f32],
  norm_y: &[f32],
  norm_z: &[f32],
  out_x: &mut [f32],
  out_y: &mut [f32],
) {
  let center_idx = cy * width + cx;
  let cn_x = norm_x[center_idx];
  let cn_y = norm_y[center_idx];
  let cn_z = norm_z[center_idx];

  let mut sum_x = 0.0f32;
  let mut sum_y = 0.0f32;
  let mut w_sum = 0.0f32;

  for ky in 0..kernel_size {
    let dy = ky as isize - radius as isize;
    let sy = (cy as isize + dy).rem_euclid(height as isize) as usize;

    for kx in 0..kernel_size {
      let dx = kx as isize - radius as isize;
      let sx = (cx as isize + dx).rem_euclid(width as isize) as usize;
      let neighbor_idx = sy * width + sx;

      let dot =
        cn_x * norm_x[neighbor_idx] + cn_y * norm_y[neighbor_idx] + cn_z * norm_z[neighbor_idx];
      let range_diff = 1.0 - dot.clamp(-1.0, 1.0);
      let range_w = (range_diff * range_diff * neg_inv_two_range_sq).exp();

      let w = spatial_weights[ky * kernel_size + kx] * range_w;
      sum_x += chan_x[neighbor_idx] * w;
      sum_y += chan_y[neighbor_idx] * w;
      w_sum += w;
    }
  }

  if w_sum > 1e-8 {
    out_x[center_idx] = sum_x / w_sum;
    out_y[center_idx] = sum_y / w_sum;
  } else {
    out_x[center_idx] = chan_x[center_idx];
    out_y[center_idx] = chan_y[center_idx];
  }
}

/// Bilateral blur on X/Y channels simultaneously, using normal dot product as the range kernel.
/// Non-separable — O(n * k^2) — but preserves edges where normals differ sharply.
/// Uses WASM SIMD to process 4 center pixels at a time for the interior,
/// with scalar fallback for edge pixels that require wrap-around.
fn bilateral_blur(
  chan_x: &[f32],
  chan_y: &[f32],
  width: usize,
  height: usize,
  spatial_sigma: f32,
  range_sigma: f32,
) -> (Vec<f32>, Vec<f32>) {
  let radius = (3.0 * spatial_sigma).ceil() as usize;
  if radius == 0 {
    return (chan_x.to_vec(), chan_y.to_vec());
  }

  let pixel_count = width * height;
  let kernel_size = 2 * radius + 1;
  let two_spatial_sq = 2.0 * spatial_sigma * spatial_sigma;
  let neg_inv_two_range_sq = -1.0 / (2.0 * range_sigma * range_sigma);

  let spatial_weights = precompute_spatial_weights(radius, two_spatial_sq);
  let (norm_x, norm_y, norm_z) = precompute_normals_soa(chan_x, chan_y);

  let mut out_x = vec![0.0f32; pixel_count];
  let mut out_y = vec![0.0f32; pixel_count];

  // Interior region where SIMD loads won't go out of bounds:
  // cx in [radius, width - radius) and we need 4 consecutive, so cx + 3 + radius < width
  let simd_x_start = radius;
  let simd_x_end = if width > radius + 4 {
    width - radius
  } else {
    0
  };

  for cy in 0..height {
    // Left edge — scalar with wrap
    for cx in 0..radius.min(width) {
      bilateral_blur_scalar_pixel(
        cx,
        cy,
        width,
        height,
        radius,
        kernel_size,
        &spatial_weights,
        neg_inv_two_range_sq,
        chan_x,
        chan_y,
        &norm_x,
        &norm_y,
        &norm_z,
        &mut out_x,
        &mut out_y,
      );
    }

    // Interior — SIMD, 4 pixels at a time
    #[cfg(target_arch = "wasm32")]
    if simd_x_end > simd_x_start {
      unsafe {
        bilateral_blur_simd_row(
          simd_x_start,
          simd_x_end,
          cy,
          width,
          height,
          radius,
          kernel_size,
          &spatial_weights,
          neg_inv_two_range_sq,
          chan_x,
          chan_y,
          &norm_x,
          &norm_y,
          &norm_z,
          &mut out_x,
          &mut out_y,
        );
      }
    }

    // Remainder pixels between SIMD end and right edge — scalar
    // This covers both the non-SIMD-aligned tail of the interior and the right edge wrap zone
    let scalar_right_start = if simd_x_end > simd_x_start {
      // SIMD processed [simd_x_start, simd_x_end) in steps of 4,
      // so it stopped at the last aligned group
      let simd_processed_end = simd_x_start + ((simd_x_end - simd_x_start) / 4) * 4;
      simd_processed_end
    } else {
      radius.min(width)
    };
    for cx in scalar_right_start..width {
      bilateral_blur_scalar_pixel(
        cx,
        cy,
        width,
        height,
        radius,
        kernel_size,
        &spatial_weights,
        neg_inv_two_range_sq,
        chan_x,
        chan_y,
        &norm_x,
        &norm_y,
        &norm_z,
        &mut out_x,
        &mut out_y,
      );
    }
  }

  (out_x, out_y)
}

/// Blur dispatcher — calls either Gaussian or bilateral depending on `use_bilateral`.
fn blur(
  chan_x: &[f32],
  chan_y: &[f32],
  width: usize,
  height: usize,
  sigma: f32,
  use_bilateral: bool,
  range_sigma: f32,
) -> (Vec<f32>, Vec<f32>) {
  if use_bilateral {
    bilateral_blur(chan_x, chan_y, width, height, sigma, range_sigma)
  } else {
    let kernel = build_gaussian_kernel(sigma);
    let blur_x = gaussian_blur_channel(chan_x, width, height, &kernel);
    let blur_y = gaussian_blur_channel(chan_y, width, height, &kernel);
    (blur_x, blur_y)
  }
}

/// Filter mode: 0 = low-pass, 1 = high-pass, 2 = band-pass, 3 = band-reject
///
/// For low-pass and high-pass, only `sigma` is used.
/// For band-pass and band-reject, `sigma` is the low cutoff and `sigma_high` is the high cutoff.
/// If `use_bilateral` is true, uses a bilateral filter (edge-preserving) instead of Gaussian.
/// `range_sigma` controls the bilateral range kernel sensitivity (ignored when not bilateral).
#[wasm_bindgen]
pub fn normal_map_filter(
  pixel_data: &[u8],
  width: u32,
  height: u32,
  filter_mode: u32,
  sigma: f32,
  sigma_high: f32,
  use_bilateral: bool,
  range_sigma: f32,
) -> Vec<u8> {
  console_error_panic_hook::set_once();

  let w = width as usize;
  let h = height as usize;
  let pixel_count = w * h;
  assert_eq!(
    pixel_data.len(),
    pixel_count * 4,
    "Pixel data must be RGBA with matching dimensions"
  );

  // Decode X and Y channels to float [-1, 1]
  let mut chan_x = Vec::with_capacity(pixel_count);
  let mut chan_y = Vec::with_capacity(pixel_count);
  for i in 0..pixel_count {
    chan_x.push(decode_channel(pixel_data[i * 4]));
    chan_y.push(decode_channel(pixel_data[i * 4 + 1]));
  }

  let (out_x, out_y) = match filter_mode {
    // Low-pass
    0 => blur(&chan_x, &chan_y, w, h, sigma, use_bilateral, range_sigma),
    // High-pass (original - blurred)
    1 => {
      let (blur_x, blur_y) = blur(&chan_x, &chan_y, w, h, sigma, use_bilateral, range_sigma);
      let hp_x: Vec<f32> = chan_x
        .iter()
        .zip(blur_x.iter())
        .map(|(o, b)| o - b)
        .collect();
      let hp_y: Vec<f32> = chan_y
        .iter()
        .zip(blur_y.iter())
        .map(|(o, b)| o - b)
        .collect();
      (hp_x, hp_y)
    },
    // Band-pass (blur_low - blur_high), isolates frequencies between sigma and sigma_high
    2 => {
      let (blur_lo_x, blur_lo_y) = blur(&chan_x, &chan_y, w, h, sigma, use_bilateral, range_sigma);
      let (blur_hi_x, blur_hi_y) = blur(
        &chan_x,
        &chan_y,
        w,
        h,
        sigma_high,
        use_bilateral,
        range_sigma,
      );
      let bp_x: Vec<f32> = blur_lo_x
        .iter()
        .zip(blur_hi_x.iter())
        .map(|(lo, hi)| lo - hi)
        .collect();
      let bp_y: Vec<f32> = blur_lo_y
        .iter()
        .zip(blur_hi_y.iter())
        .map(|(lo, hi)| lo - hi)
        .collect();
      (bp_x, bp_y)
    },
    // Band-reject (original - band_pass)
    3 => {
      let (blur_lo_x, blur_lo_y) = blur(&chan_x, &chan_y, w, h, sigma, use_bilateral, range_sigma);
      let (blur_hi_x, blur_hi_y) = blur(
        &chan_x,
        &chan_y,
        w,
        h,
        sigma_high,
        use_bilateral,
        range_sigma,
      );
      let bp_x: Vec<f32> = blur_lo_x
        .iter()
        .zip(blur_hi_x.iter())
        .map(|(lo, hi)| lo - hi)
        .collect();
      let bp_y: Vec<f32> = blur_lo_y
        .iter()
        .zip(blur_hi_y.iter())
        .map(|(lo, hi)| lo - hi)
        .collect();
      let br_x: Vec<f32> = chan_x
        .iter()
        .zip(bp_x.iter())
        .map(|(o, bp)| o - bp)
        .collect();
      let br_y: Vec<f32> = chan_y
        .iter()
        .zip(bp_y.iter())
        .map(|(o, bp)| o - bp)
        .collect();
      (br_x, br_y)
    },
    _ => panic!("Invalid filter mode: {}", filter_mode),
  };

  // Reconstruct Z and encode output
  let mut output = Vec::with_capacity(pixel_count * 4);
  for i in 0..pixel_count {
    let x = out_x[i];
    let y = out_y[i];

    // Reconstruct Z from unit normal constraint, clamping to avoid NaN from sqrt
    let z_sq = (1.0 - x * x - y * y).max(0.0);
    let z = z_sq.sqrt();

    // Normalize the vector
    let len = (x * x + y * y + z * z).sqrt();
    let (nx, ny, nz) = if len > 1e-8 {
      (x / len, y / len, z / len)
    } else {
      (0.0, 0.0, 1.0)
    };

    output.push(encode_channel(nx));
    output.push(encode_channel(ny));
    output.push(encode_channel(nz));
    output.push(255);
  }

  output
}
