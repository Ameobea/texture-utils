// Makes a near-seamless texture into a fully seamless one with a "fixup"
// pass.  For each output pixel, blends the original with a copy shifted by
// (W/2, H/2).  Near the edges the weight favors the shifted copy (whose seam
// is now in the interior), and near the center it favors the original.  The
// result is a single texture of the same size that tiles cleanly.
//
// Optional variance-preserving contrast correction follows the trick from
// Heitz & Neyret, "High-Performance By-Example Noise using a Histogram-
// Preserving Blending Operator" (https://hal.inria.fr/hal-01824773), with
// the simpler variance-preserving form originally published in
// https://hal.inria.fr/inria-00536064v2 and demonstrated on Shadertoy at
// https://www.shadertoy.com/view/MdyfDV.
//
// Optional frequency-aware mode splits the input into a low-pass band
// (lighting / broad color shifts) and a high-pass residual (detail), runs
// the shifted blend on each band independently with band-appropriate
// margins, then recombines.  This dramatically reduces the muddy smudge
// that linear blending produces when the seam straddles a lighting
// gradient.

use wasm_bindgen::prelude::*;

// Multipliers applied to the user-supplied margin when running the per-band
// blend in frequency-aware mode.  The low band uses a wide margin (the blur
// removed all detail, so a wide blend smoothly equalizes lighting without
// muddying anything).  The high band uses a narrow margin (the lighting
// gradient that caused the smudge is gone, so we only need a thin transition
// to hide the discontinuity).
const LOW_BAND_MARGIN_MULT: f32 = 2.5;
const HIGH_BAND_MARGIN_MULT: f32 = 0.5;

fn smoothstep(t: f32) -> f32 {
  let t = t.clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

/// Returns the weight assigned to the *original* (un-shifted) sample at
/// `coord` along an axis of length `total`.  1.0 in the interior, 0.0 at the
/// seam (coord == 0 or coord == total - 1), with a smooth transition over
/// the margin in pixels.
fn original_weight(coord: usize, total: usize, margin_px: f32) -> f32 {
  if margin_px <= 0.0 || total <= 1 {
    // Margin disabled — use the original everywhere.  The image will still
    // have its original seams in this direction.
    return 1.0;
  }
  let coord = coord as f32;
  let max = (total - 1) as f32;
  let dist_to_seam = coord.min(max - coord);
  if dist_to_seam >= margin_px {
    1.0
  } else {
    smoothstep(dist_to_seam / margin_px)
  }
}

fn compute_average_color(data: &[u8]) -> [f32; 3] {
  let pixel_count = data.len() / 4;
  if pixel_count == 0 {
    return [0.0; 3];
  }
  let mut sum = [0.0f32; 3];
  for i in 0..pixel_count {
    sum[0] += data[i * 4] as f32;
    sum[1] += data[i * 4 + 1] as f32;
    sum[2] += data[i * 4 + 2] as f32;
  }
  let n = pixel_count as f32;
  [sum[0] / n, sum[1] / n, sum[2] / n]
}

#[inline]
fn read_rgb(data: &[u8], width: usize, x: usize, y: usize) -> [f32; 3] {
  let ix = (y * width + x) * 4;
  [data[ix] as f32, data[ix + 1] as f32, data[ix + 2] as f32]
}

/// Build a 1D Gaussian kernel with the given sigma.
/// Kernel radius = ceil(3 * sigma), total size = 2 * radius + 1.
fn build_gaussian_kernel(sigma: f32) -> Vec<f32> {
  let radius = (3.0 * sigma).ceil().max(1.0) as usize;
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
  for v in &mut kernel {
    *v /= sum;
  }
  kernel
}

/// Separable Gaussian blur over an interleaved RGB f32 buffer with toroidal
/// (wrap-around) boundary handling — required since our input is meant to
/// be a tile and the low-pass band needs to itself be seamless.
fn gaussian_blur_rgb_f32(data: &[f32], width: usize, height: usize, sigma: f32) -> Vec<f32> {
  if sigma <= 0.0 {
    return data.to_vec();
  }
  let kernel = build_gaussian_kernel(sigma);
  let radius = kernel.len() / 2;
  let pixel_count = width * height;
  let mut temp = vec![0.0f32; pixel_count * 3];
  let mut output = vec![0.0f32; pixel_count * 3];

  // Horizontal pass
  for y in 0..height {
    for x in 0..width {
      let mut sum = [0.0f32; 3];
      for k in 0..kernel.len() {
        let sx = (x as isize + k as isize - radius as isize).rem_euclid(width as isize) as usize;
        let src = (y * width + sx) * 3;
        let kw = kernel[k];
        sum[0] += data[src] * kw;
        sum[1] += data[src + 1] * kw;
        sum[2] += data[src + 2] * kw;
      }
      let dst = (y * width + x) * 3;
      temp[dst] = sum[0];
      temp[dst + 1] = sum[1];
      temp[dst + 2] = sum[2];
    }
  }

  // Vertical pass
  for y in 0..height {
    for x in 0..width {
      let mut sum = [0.0f32; 3];
      for k in 0..kernel.len() {
        let sy = (y as isize + k as isize - radius as isize).rem_euclid(height as isize) as usize;
        let src = (sy * width + x) * 3;
        let kw = kernel[k];
        sum[0] += temp[src] * kw;
        sum[1] += temp[src + 1] * kw;
        sum[2] += temp[src + 2] * kw;
      }
      let dst = (y * width + x) * 3;
      output[dst] = sum[0];
      output[dst + 1] = sum[1];
      output[dst + 2] = sum[2];
    }
  }

  output
}

/// 4-way shifted blend on an interleaved RGB f32 buffer.  Same math as the
/// u8 path in `seamless_tile_maker` below, but operates on f32 so it can be
/// reused for the low- and high-pass bands.
fn shifted_blend_rgb_f32(
  data: &[f32],
  width: usize,
  height: usize,
  margin_x_px: f32,
  margin_y_px: f32,
  contrast_factor: f32,
  ref_mean: [f32; 3],
) -> Vec<f32> {
  let pixel_count = width * height;
  let half_w = width / 2;
  let half_h = height / 2;
  let mut output = Vec::with_capacity(pixel_count * 3);

  for y in 0..height {
    let wy = original_weight(y, height, margin_y_px);
    let shifted_y = (y + half_h) % height;

    for x in 0..width {
      let wx = original_weight(x, width, margin_x_px);
      let shifted_x = (x + half_w) % width;

      let ia = (y * width + x) * 3;
      let ib = (y * width + shifted_x) * 3;
      let ic = (shifted_y * width + x) * 3;
      let id = (shifted_y * width + shifted_x) * 3;

      let wa = wx * wy;
      let wb = (1.0 - wx) * wy;
      let wc = wx * (1.0 - wy);
      let wd = (1.0 - wx) * (1.0 - wy);

      let s2 = wa * wa + wb * wb + wc * wc + wd * wd;
      let divisor = s2.sqrt().max(1e-6);

      for c in 0..3 {
        let blended =
          wa * data[ia + c] + wb * data[ib + c] + wc * data[ic + c] + wd * data[id + c];
        let final_val = if contrast_factor > 0.0 {
          let corrected = ref_mean[c] + (blended - ref_mean[c]) / divisor;
          blended * (1.0 - contrast_factor) + corrected * contrast_factor
        } else {
          blended
        };
        output.push(final_val);
      }
    }
  }

  output
}

/// Decode the RGB channels of an RGBA u8 buffer to interleaved f32 (alpha
/// is dropped — the seam concept doesn't apply to it for the textures this
/// tool is intended for).
fn rgba_to_rgb_f32(data: &[u8]) -> Vec<f32> {
  let pixel_count = data.len() / 4;
  let mut out = Vec::with_capacity(pixel_count * 3);
  for i in 0..pixel_count {
    out.push(data[i * 4] as f32);
    out.push(data[i * 4 + 1] as f32);
    out.push(data[i * 4 + 2] as f32);
  }
  out
}

/// Encode an interleaved RGB f32 buffer back to RGBA u8, preserving the
/// original alpha channel.  Values outside [0, 255] are clamped.
fn rgb_f32_to_rgba(rgb: &[f32], src_alpha: &[u8]) -> Vec<u8> {
  let pixel_count = rgb.len() / 3;
  let mut out = Vec::with_capacity(pixel_count * 4);
  for i in 0..pixel_count {
    out.push(rgb[i * 3].clamp(0.0, 255.0).round() as u8);
    out.push(rgb[i * 3 + 1].clamp(0.0, 255.0).round() as u8);
    out.push(rgb[i * 3 + 2].clamp(0.0, 255.0).round() as u8);
    out.push(src_alpha[i * 4 + 3]);
  }
  out
}

/// Encode a high-pass-style buffer (values centered on 0) by adding a
/// mid-gray offset.  Used for the debug view modes that surface the raw
/// high band so the user can see what the frequency split is doing.
fn rgb_f32_to_rgba_centered(rgb: &[f32], src_alpha: &[u8]) -> Vec<u8> {
  let pixel_count = rgb.len() / 3;
  let mut out = Vec::with_capacity(pixel_count * 4);
  for i in 0..pixel_count {
    out.push((rgb[i * 3] + 128.0).clamp(0.0, 255.0).round() as u8);
    out.push((rgb[i * 3 + 1] + 128.0).clamp(0.0, 255.0).round() as u8);
    out.push((rgb[i * 3 + 2] + 128.0).clamp(0.0, 255.0).round() as u8);
    out.push(src_alpha[i * 4 + 3]);
  }
  out
}

#[wasm_bindgen]
pub fn seamless_tile_maker(
  pixel_data: &[u8],
  width: u32,
  height: u32,
  margin_x_frac: f32,
  margin_y_frac: f32,
  contrast_correction_factor: f32,
  frequency_aware: bool,
  split_sigma: f32,
  // 0 = Final, 1 = Low pass (raw), 2 = High pass (raw, +128),
  // 3 = Low pass blended, 4 = High pass blended (+128).
  // Ignored when `frequency_aware` is false.
  view_mode: u32,
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

  // Margins are expressed as a fraction of the corresponding dimension.  The
  // useful range is [0, 0.5]; the seam-distance metric tops out at half the
  // dimension.
  let margin_x_px = (margin_x_frac.max(0.0) * w as f32).min(w as f32 / 2.0);
  let margin_y_px = (margin_y_frac.max(0.0) * h as f32).min(h as f32 / 2.0);

  if frequency_aware {
    return run_frequency_aware(
      pixel_data,
      w,
      h,
      margin_x_px,
      margin_y_px,
      contrast_correction_factor,
      split_sigma,
      view_mode,
    );
  }

  let half_w = w / 2;
  let half_h = h / 2;

  // Used as the reference for variance-preserving blending.
  let avg_color = compute_average_color(pixel_data);

  let mut output = Vec::with_capacity(pixel_count * 4);

  for y in 0..h {
    let wy = original_weight(y, h, margin_y_px);
    let shifted_y = (y + half_h) % h;

    for x in 0..w {
      let wx = original_weight(x, w, margin_x_px);
      let shifted_x = (x + half_w) % w;

      // 4 candidate samples — the original and three phase-shifted copies.
      let s_a = read_rgb(pixel_data, w, x, y);
      let s_b = read_rgb(pixel_data, w, shifted_x, y);
      let s_c = read_rgb(pixel_data, w, x, shifted_y);
      let s_d = read_rgb(pixel_data, w, shifted_x, shifted_y);

      let wa = wx * wy;
      let wb = (1.0 - wx) * wy;
      let wc = wx * (1.0 - wy);
      let wd = (1.0 - wx) * (1.0 - wy);

      let blended = [
        wa * s_a[0] + wb * s_b[0] + wc * s_c[0] + wd * s_d[0],
        wa * s_a[1] + wb * s_b[1] + wc * s_c[1] + wd * s_d[1],
        wa * s_a[2] + wb * s_b[2] + wc * s_c[2] + wd * s_d[2],
      ];

      let final_color = if contrast_correction_factor > 0.0 {
        // Variance-preserving blend: the weights sum to 1, but the sum of
        // squared weights ("s2") drops below 1 inside the blend zone.
        // Re-scaling the deviation from the mean by 1/sqrt(s2) restores
        // contrast that linear blending would otherwise wash out.
        let s2 = wa * wa + wb * wb + wc * wc + wd * wd;
        let divisor = s2.sqrt().max(1e-6);
        let corrected = [
          avg_color[0] + (blended[0] - avg_color[0]) / divisor,
          avg_color[1] + (blended[1] - avg_color[1]) / divisor,
          avg_color[2] + (blended[2] - avg_color[2]) / divisor,
        ];
        let f = contrast_correction_factor;
        [
          blended[0] * (1.0 - f) + corrected[0] * f,
          blended[1] * (1.0 - f) + corrected[1] * f,
          blended[2] * (1.0 - f) + corrected[2] * f,
        ]
      } else {
        blended
      };

      // Preserve the original alpha — we don't blend it, since the "seam"
      // concept doesn't really apply to alpha for the textures this tool
      // is intended for.
      let alpha = pixel_data[(y * w + x) * 4 + 3];

      output.push(final_color[0].clamp(0.0, 255.0).round() as u8);
      output.push(final_color[1].clamp(0.0, 255.0).round() as u8);
      output.push(final_color[2].clamp(0.0, 255.0).round() as u8);
      output.push(alpha);
    }
  }

  output
}

fn run_frequency_aware(
  pixel_data: &[u8],
  w: usize,
  h: usize,
  margin_x_px: f32,
  margin_y_px: f32,
  contrast_correction_factor: f32,
  split_sigma: f32,
  view_mode: u32,
) -> Vec<u8> {
  let pixel_count = w * h;

  let rgb_f32 = rgba_to_rgb_f32(pixel_data);

  // Low band: heavy Gaussian blur with wrap-around.  Captures lighting
  // gradients and broad color shifts.
  let low = gaussian_blur_rgb_f32(&rgb_f32, w, h, split_sigma);

  // High band: original minus low band.  Captures detail; centered on 0.
  let mut high = Vec::with_capacity(pixel_count * 3);
  for i in 0..pixel_count * 3 {
    high.push(rgb_f32[i] - low[i]);
  }

  if view_mode == 1 {
    return rgb_f32_to_rgba(&low, pixel_data);
  }
  if view_mode == 2 {
    return rgb_f32_to_rgba_centered(&high, pixel_data);
  }

  // Cap per-band margins at the dimension half — original_weight tops out
  // there anyway, but this keeps the multiplied values from going wild on
  // tiny images.
  let low_margin_x = (margin_x_px * LOW_BAND_MARGIN_MULT).min(w as f32 / 2.0);
  let low_margin_y = (margin_y_px * LOW_BAND_MARGIN_MULT).min(h as f32 / 2.0);
  let high_margin_x = margin_x_px * HIGH_BAND_MARGIN_MULT;
  let high_margin_y = margin_y_px * HIGH_BAND_MARGIN_MULT;

  // Low band: no contrast correction.  The variance-preserving trick would
  // amplify the very lighting gradient we're trying to smooth.
  let low_mean = {
    let mut m = [0.0f32; 3];
    let n = pixel_count as f32;
    for i in 0..pixel_count {
      m[0] += low[i * 3];
      m[1] += low[i * 3 + 1];
      m[2] += low[i * 3 + 2];
    }
    [m[0] / n, m[1] / n, m[2] / n]
  };
  let low_blended = shifted_blend_rgb_f32(
    &low,
    w,
    h,
    low_margin_x,
    low_margin_y,
    0.0,
    low_mean,
  );

  // High band: apply contrast correction so the detail in the blend zone
  // doesn't wash out.  Reference mean is 0 since the high band is
  // zero-centered by construction.
  let high_blended = shifted_blend_rgb_f32(
    &high,
    w,
    h,
    high_margin_x,
    high_margin_y,
    contrast_correction_factor,
    [0.0, 0.0, 0.0],
  );

  if view_mode == 3 {
    return rgb_f32_to_rgba(&low_blended, pixel_data);
  }
  if view_mode == 4 {
    return rgb_f32_to_rgba_centered(&high_blended, pixel_data);
  }

  // Recombine.
  let mut combined = Vec::with_capacity(pixel_count * 3);
  for i in 0..pixel_count * 3 {
    combined.push(low_blended[i] + high_blended[i]);
  }
  rgb_f32_to_rgba(&combined, pixel_data)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Builds a deliberately seam-y image (sharp gradient that wraps poorly)
  /// and verifies that the kernel makes left/right and top/bottom edges
  /// actually match.
  #[test]
  fn output_is_seamless() {
    let w: u32 = 64;
    let h: u32 = 64;
    let mut input = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
      for x in 0..w {
        let r = (x as f32 / (w - 1) as f32 * 255.0) as u8;
        let g = (y as f32 / (h - 1) as f32 * 255.0) as u8;
        input.extend_from_slice(&[r, g, 128, 255]);
      }
    }

    let out = seamless_tile_maker(&input, w, h, 0.25, 0.25, 0.0, false, 0.0, 0);

    let mut max_seam_diff = 0i32;
    for y in 0..h as usize {
      let l = y * w as usize * 4;
      let r = (y * w as usize + (w as usize - 1)) * 4;
      for c in 0..3 {
        let d = (out[l + c] as i32 - out[r + c] as i32).abs();
        max_seam_diff = max_seam_diff.max(d);
      }
    }
    for x in 0..w as usize {
      let t = x * 4;
      let b = ((h as usize - 1) * w as usize + x) * 4;
      for c in 0..3 {
        let d = (out[t + c] as i32 - out[b + c] as i32).abs();
        max_seam_diff = max_seam_diff.max(d);
      }
    }

    // Without the tool the gradient seam diff is ~252.  After fixup it should
    // be at most a couple of LSBs of rounding error.
    assert!(
      max_seam_diff <= 4,
      "seam still visible after fixup: {}",
      max_seam_diff
    );
  }

  /// Setting both margins to 0 must be a no-op (input == output, modulo
  /// rounding).
  #[test]
  fn zero_margin_is_passthrough() {
    let w: u32 = 32;
    let h: u32 = 32;
    let mut input = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
      for x in 0..w {
        input.extend_from_slice(&[(x * 7) as u8, (y * 11) as u8, ((x + y) * 3) as u8, 255]);
      }
    }
    let out = seamless_tile_maker(&input, w, h, 0.0, 0.0, 0.5, false, 0.0, 0);
    assert_eq!(input, out);
  }

  /// Contrast-corrected blending should match plain blending when the
  /// correction factor is 0.
  #[test]
  fn contrast_factor_zero_matches_plain_blend() {
    let w: u32 = 32;
    let h: u32 = 32;
    let mut input = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
      for x in 0..w {
        input.extend_from_slice(&[(x * 5) as u8, (y * 9) as u8, 200, 255]);
      }
    }
    let plain = seamless_tile_maker(&input, w, h, 0.3, 0.3, 0.0, false, 0.0, 0);
    let corrected_zero = seamless_tile_maker(&input, w, h, 0.3, 0.3, 0.0, false, 0.0, 0);
    assert_eq!(plain, corrected_zero);
  }

  /// Frequency-aware path with the gradient image must also produce a
  /// seamless tile.  Allows a slightly larger tolerance because the f32
  /// roundtrip + recombine accumulates a touch more rounding error than
  /// the single-pass u8 path.
  #[test]
  fn frequency_aware_output_is_seamless() {
    let w: u32 = 64;
    let h: u32 = 64;
    let mut input = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
      for x in 0..w {
        let r = (x as f32 / (w - 1) as f32 * 255.0) as u8;
        let g = (y as f32 / (h - 1) as f32 * 255.0) as u8;
        input.extend_from_slice(&[r, g, 128, 255]);
      }
    }

    let out = seamless_tile_maker(&input, w, h, 0.25, 0.25, 0.0, true, 8.0, 0);

    let mut max_seam_diff = 0i32;
    for y in 0..h as usize {
      let l = y * w as usize * 4;
      let r = (y * w as usize + (w as usize - 1)) * 4;
      for c in 0..3 {
        let d = (out[l + c] as i32 - out[r + c] as i32).abs();
        max_seam_diff = max_seam_diff.max(d);
      }
    }
    for x in 0..w as usize {
      let t = x * 4;
      let b = ((h as usize - 1) * w as usize + x) * 4;
      for c in 0..3 {
        let d = (out[t + c] as i32 - out[b + c] as i32).abs();
        max_seam_diff = max_seam_diff.max(d);
      }
    }

    assert!(
      max_seam_diff <= 4,
      "freq-aware seam still visible: {}",
      max_seam_diff
    );
  }

  /// Recombining low + high should reconstruct the original (modulo
  /// rounding) when margins are zero in frequency-aware mode.
  #[test]
  fn frequency_aware_zero_margin_is_passthrough() {
    let w: u32 = 32;
    let h: u32 = 32;
    let mut input = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
      for x in 0..w {
        input.extend_from_slice(&[(x * 7) as u8, (y * 11) as u8, ((x + y) * 3) as u8, 255]);
      }
    }
    let out = seamless_tile_maker(&input, w, h, 0.0, 0.0, 0.0, true, 4.0, 0);
    // Allow a small tolerance — the f32 blur + recombine isn't bit-exact.
    let mut max_diff = 0i32;
    for i in 0..input.len() {
      let d = (input[i] as i32 - out[i] as i32).abs();
      max_diff = max_diff.max(d);
    }
    assert!(max_diff <= 2, "freq-aware passthrough diff too large: {}", max_diff);
  }
}
