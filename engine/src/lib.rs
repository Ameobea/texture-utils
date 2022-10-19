use kmeans_colors::{get_kmeans_hamerly, Sort};
use palette::{white_point::D65, ColorDifference, FromColor, IntoColor, Lab, Pixel, Srgb};
use wasm_bindgen::prelude::*;

mod texture_crossfade;

#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(js_namespace = console)]
  fn log(s: &str);
}

static mut PALETTE_GEN_SCORE: f32 = 8888.888;

#[wasm_bindgen]
pub fn get_palette_gen_score() -> f32 {
  unsafe { PALETTE_GEN_SCORE }
}

#[wasm_bindgen]
pub fn gen_palette(count: usize, pixel_data: &[u8], seed: f64) -> Vec<u8> {
  console_error_panic_hook::set_once();

  assert_eq!(pixel_data.len() % 4, 0, "Pixel data must be RGBA");
  let mut pixel_data_without_alpha = Vec::with_capacity(pixel_data.len() / 4 * 3);
  for i in 0..pixel_data.len() / 4 {
    pixel_data_without_alpha.push(pixel_data[i * 4]);
    pixel_data_without_alpha.push(pixel_data[i * 4 + 1]);
    pixel_data_without_alpha.push(pixel_data[i * 4 + 2]);
  }
  assert_eq!(pixel_data_without_alpha.len() % 3, 0);

  let srgb = Srgb::from_raw_slice(&pixel_data_without_alpha);
  let lab: Vec<Lab> = srgb.iter().map(|x| x.into_format().into_color()).collect();

  let max_iter = 5;
  let converge = 0.001;
  let seed = unsafe { std::mem::transmute(seed) };
  let result = get_kmeans_hamerly(count, max_iter, converge, false, &lab, seed);

  log("Successfully generated palette");
  log(&format!("Score: {}", result.score));
  unsafe {
    PALETTE_GEN_SCORE = result.score;
  }

  // sort the colors by luminance
  let sorted = Lab::sort_indexed_colors(&result.centroids, &result.indices);

  // Convert indexed colors back to Srgb<u8> for output
  let rgb = &sorted
    .iter()
    .map(|x| Srgb::from_color(x.centroid).into_format())
    .collect::<Vec<Srgb<u8>>>();

  let mut pixels = Vec::with_capacity(rgb.len() * 4);
  for px in rgb {
    pixels.push(px.red);
    pixels.push(px.green);
    pixels.push(px.blue);
    pixels.push(255);
  }

  pixels
}

fn build_lookup_table(palette: &[u8]) -> Vec<Lab<D65, f32>> {
  // There are 256 possible slots available in this single-channel encoding, so we can determine the color at each of them
  let mut lut: Vec<Lab<D65, f32>> = Vec::with_capacity(256);
  for slot_ix in 0..256 {
    // [0, 255] -> [0, palette.len() - 1]
    let palette_ix = slot_ix as f32 / 255.0 * (palette.len() / 4 - 1) as f32;
    // interpolate between the two closest palette colors
    let palette_ix_floor = palette_ix.floor() as usize;
    let palette_ix_ceil = (palette_ix.ceil() as usize).min(palette.len() / 4 - 1);
    let palette_ix_frac = palette_ix - palette_ix_floor as f32;

    let palette_color_floor = [
      palette[palette_ix_floor * 4],
      palette[palette_ix_floor * 4 + 1],
      palette[palette_ix_floor * 4 + 2],
    ];
    let palette_color_ceil = [
      palette[palette_ix_ceil * 4],
      palette[palette_ix_ceil * 4 + 1],
      palette[palette_ix_ceil * 4 + 2],
    ];
    let palette_color_f32 = [
      palette_color_floor[0] as f32 * (1.0 - palette_ix_frac)
        + palette_color_ceil[0] as f32 * palette_ix_frac,
      palette_color_floor[1] as f32 * (1.0 - palette_ix_frac)
        + palette_color_ceil[1] as f32 * palette_ix_frac,
      palette_color_floor[2] as f32 * (1.0 - palette_ix_frac)
        + palette_color_ceil[2] as f32 * palette_ix_frac,
    ];
    let palette_color = [
      palette_color_f32[0].round() as u8,
      palette_color_f32[1].round() as u8,
      palette_color_f32[2].round() as u8,
    ];
    let palette_color_rgb = Srgb::new(palette_color[0], palette_color[1], palette_color[2]);
    let palette_color_lab = palette_color_rgb.into_format().into_color();
    lut.push(palette_color_lab);
  }

  lut
}

#[wasm_bindgen]
pub fn build_full_lookup_table(palette: &[u8]) -> Vec<u8> {
  let lut = build_lookup_table(palette);
  let mut lut_bytes = Vec::with_capacity(lut.len() * 4);
  for px in lut {
    let srbg = Srgb::from_color(px).into_format();
    lut_bytes.push(srbg.red);
    lut_bytes.push(srbg.green);
    lut_bytes.push(srbg.blue);
    lut_bytes.push(255);
  }
  lut_bytes
}

#[wasm_bindgen]
pub fn encode_image(palette: &[u8], img_pixel_data: &[u8]) -> Vec<u8> {
  assert_eq!(img_pixel_data.len() % 4, 0, "Pixel data must be RGBA");
  let mut img_pixel_data_without_alpha = Vec::with_capacity(img_pixel_data.len() / 4 * 3);
  for i in 0..img_pixel_data.len() / 4 {
    img_pixel_data_without_alpha.push(img_pixel_data[i * 4]);
    img_pixel_data_without_alpha.push(img_pixel_data[i * 4 + 1]);
    img_pixel_data_without_alpha.push(img_pixel_data[i * 4 + 2]);
  }
  assert_eq!(img_pixel_data_without_alpha.len() % 3, 0);

  let img_srgb = Srgb::from_raw_slice(&img_pixel_data_without_alpha);
  let img_lab: Vec<Lab> = img_srgb
    .iter()
    .map(|x| x.into_format().into_color())
    .collect();

  let lut = build_lookup_table(palette);

  let encoded_img: Vec<u8> = img_lab
    .into_iter()
    .map(|px| {
      let mut min_dist = f32::MAX;
      let mut min_slot = 0;
      for (slot_ix, slot) in lut.iter().enumerate() {
        let dist = px.get_color_difference(slot);
        if dist < min_dist {
          min_dist = dist;
          min_slot = slot_ix;
        }
      }
      min_slot as u8
    })
    .collect();

  encoded_img
}

#[wasm_bindgen]
pub fn decode_pixels(palette: &[u8], encoded_img: &[u8]) -> Vec<u8> {
  let lut = build_lookup_table(palette);

  let mut decoded_img: Vec<u8> = Vec::with_capacity(encoded_img.len() * 4);
  for px in encoded_img {
    let slot = lut[*px as usize];
    let slot_rgb = Srgb::from_color(slot).into_format();
    decoded_img.push(slot_rgb.red);
    decoded_img.push(slot_rgb.green);
    decoded_img.push(slot_rgb.blue);
    decoded_img.push(255);
  }

  decoded_img
}

#[wasm_bindgen]
pub fn compute_loss(original: &[u8], roundtripped: &[u8]) -> f32 {
  assert_eq!(original.len(), roundtripped.len());
  assert_eq!(original.len() % 4, 0);

  let mut original_without_alpha = Vec::with_capacity(original.len() / 4 * 3);
  for i in 0..original.len() / 4 {
    original_without_alpha.push(original[i * 4]);
    original_without_alpha.push(original[i * 4 + 1]);
    original_without_alpha.push(original[i * 4 + 2]);
  }

  let mut roundtripped_without_alpha = Vec::with_capacity(roundtripped.len() / 4 * 3);
  for i in 0..roundtripped.len() / 4 {
    roundtripped_without_alpha.push(roundtripped[i * 4]);
    roundtripped_without_alpha.push(roundtripped[i * 4 + 1]);
    roundtripped_without_alpha.push(roundtripped[i * 4 + 2]);
  }

  let original = original_without_alpha;
  let roundtripped = roundtripped_without_alpha;

  let original_srgb = Srgb::from_raw_slice(&original);
  let original_lab: Vec<Lab> = original_srgb
    .iter()
    .map(|x| x.into_format().into_color())
    .collect();

  let roundtripped_srgb = Srgb::from_raw_slice(&roundtripped);
  let roundtripped_lab: Vec<Lab> = roundtripped_srgb
    .iter()
    .map(|x| x.into_format().into_color())
    .collect();

  let mut loss = 0.0;
  for i in 0..original_lab.len() {
    loss += original_lab[i].get_color_difference(&roundtripped_lab[i]);
  }
  loss / original_lab.len() as f32
}
