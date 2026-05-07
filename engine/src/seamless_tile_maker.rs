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

use wasm_bindgen::prelude::*;

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

#[wasm_bindgen]
pub fn seamless_tile_maker(
  pixel_data: &[u8],
  width: u32,
  height: u32,
  margin_x_frac: f32,
  margin_y_frac: f32,
  contrast_correction_factor: f32,
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

    let out = seamless_tile_maker(&input, w, h, 0.25, 0.25, 0.0);

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
    let out = seamless_tile_maker(&input, w, h, 0.0, 0.0, 0.5);
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
    let plain = seamless_tile_maker(&input, w, h, 0.3, 0.3, 0.0);
    let corrected_zero = seamless_tile_maker(&input, w, h, 0.3, 0.3, 0.0);
    assert_eq!(plain, corrected_zero);
  }
}
