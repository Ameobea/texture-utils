use nalgebra::Vector3;
use wasm_bindgen::prelude::*;

static mut INPUT_TEXTURE: *mut Vec<u8> = std::ptr::null_mut();

fn srgb_to_linear(c: Vector3<f32>) -> Vector3<f32> { c.map(srgb_to_linear_f) }

fn linearize_image(image: &mut Vec<u8>) {
  for chunk in image.array_chunks_mut::<4>() {
    let r = chunk[0] as f32 / 255.;
    let g = chunk[1] as f32 / 255.;
    let b = chunk[2] as f32 / 255.;
    let a = chunk[3] as f32 / 255.;

    let linear = |c: f32| {
      if c <= 0.04045 {
        c / 12.92
      } else {
        ((c + 0.055) / 1.055).powf(2.4)
      }
    };
    chunk[0] = (linear(r) * 255.) as u8;
    chunk[1] = (linear(g) * 255.) as u8;
    chunk[2] = (linear(b) * 255.) as u8;
    chunk[3] = (a * 255.) as u8;
  }
}

#[wasm_bindgen]
pub fn reverse_color_ramp_set_input_texture(mut input_texture: Vec<u8>) {
  console_error_panic_hook::set_once();

  // expects RGBA
  assert_eq!(input_texture.len() % 4, 0, "Pixel data must be RGBA");

  linearize_image(&mut input_texture);
  unsafe {
    if !INPUT_TEXTURE.is_null() {
      drop(Box::from_raw(INPUT_TEXTURE));
    }
    INPUT_TEXTURE = Box::into_raw(Box::new(input_texture));
  }
}

#[wasm_bindgen]
pub fn reverse_color_ramp(
  width: u32,
  height: u32,
  color_a_srgb: &[f32],
  color_b_srgb: &[f32],
  v_min: f32,
  v_max: f32,
  curve_steepness: f32, // n, >= 1.0
  curve_offset: f32,    // [0, 1]
  perp_sigma: f32,
  base_fallback: f32,
) -> Vec<u8> {
  let mut out_texture_data = vec![0; (width * height * 4) as usize];

  let color_a_lin = srgb_to_linear(Vector3::from_column_slice(color_a_srgb));
  let color_b_lin = srgb_to_linear(Vector3::from_column_slice(color_b_srgb));

  let u = color_b_lin - color_a_lin;
  let len = u.norm();
  let inv_len = if len > 1e-6 { 1. / len } else { 0. };
  let u_norm = u * inv_len;

  let inv2s2 = if perp_sigma > 0. {
    0.5 / (perp_sigma * perp_sigma)
  } else {
    0.
  };

  let texture_data = unsafe {
    assert!(!INPUT_TEXTURE.is_null(), "Input texture not set");
    &*INPUT_TEXTURE
  };

  for y in 0..height {
    for x in 0..width {
      let i = ((y * width + x) * 4) as usize;
      let pixel_lin = Vector3::new(
        texture_data[i] as f32 / 255.,
        texture_data[i + 1] as f32 / 255.,
        texture_data[i + 2] as f32 / 255.,
      );

      let rel = pixel_lin - color_a_lin;
      let proj = rel.dot(&u_norm);

      // Offset and clamp t
      let mut t = (proj * inv_len).clamp(0., 1.);
      // Shift t so that curve_offset is the midpoint
      if curve_offset > 0. && curve_offset < 1. {
        t = ((t - curve_offset) / (1. - curve_offset)).clamp(0., 1.);
      }
      // Generalized smoothstep: t^n / (t^n + (1-t)^n)
      let n = curve_steepness.max(1.);
      // let t_n = t.powf(n);
      let t_n = fastapprox::fast::pow(t, n);
      // let one_minus_t_n = (1. - t).powf(n);
      let one_minus_t_n = fastapprox::fast::pow(1. - t, n);
      let t_curved = if t_n + one_minus_t_n > 0. {
        t_n / (t_n + one_minus_t_n)
      } else {
        t
      };

      let v01 = if perp_sigma > 0. {
        let d_perp_sq = (rel - u_norm * proj).norm_squared();
        let gate = (-inv2s2 * d_perp_sq).exp();
        base_fallback * (1. - gate) + t_curved * gate
      } else {
        t_curved
      };

      let out_val = v_min * (1. - v01) + v_max * v01;
      let out_val_clamped = out_val.clamp(0., 1.);

      let out_byte = (out_val_clamped * 255.) as u8;
      out_texture_data[i] = out_byte;
      out_texture_data[i + 1] = out_byte;
      out_texture_data[i + 2] = out_byte;
      out_texture_data[i + 3] = texture_data[i + 3];
    }
  }

  out_texture_data
}

fn srgb_to_linear_f(c: f32) -> f32 {
  if c <= 0.04045 {
    c / 12.92
  } else {
    ((c + 0.055) / 1.055).powf(2.4)
  }
}
