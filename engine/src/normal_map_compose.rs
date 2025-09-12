use nalgebra::Vector3;
use wasm_bindgen::prelude::*;

fn decode_ts(v: [u8; 3]) -> Vector3<f32> {
  let v = Vector3::new(v[0] as f32, v[1] as f32, v[2] as f32);
  (v / 255.) * 2. - Vector3::repeat(1.)
}

fn encode_ts(v: Vector3<f32>) -> [u8; 3] {
  let v = (v * 0.5 + Vector3::repeat(0.5)) * 255.;
  [v.x as u8, v.y as u8, v.z as u8]
}

/// Blends two tangent-space normal maps using Reoriented Normal Mapping
/// (https://colinbarrebrisebois.com/category/reoriented-normal-mapping/)
fn blend_rnm(base_ts: Vector3<f32>, detail_ts: Vector3<f32>) -> Vector3<f32> {
  let n1 = base_ts;
  let n2 = detail_ts;

  let r = Vector3::new(
    n1.x + n2.x,
    n1.y + n2.y,
    n1.z * n2.z - n1.xy().dot(&n2.xy()),
  );
  r.normalize()
}

#[wasm_bindgen]
pub fn normal_map_compose(base_map_data: &[u8], detail_map_data: &[u8], weight: f32) -> Vec<u8> {
  console_error_panic_hook::set_once();

  assert_eq!(
    base_map_data.len(),
    detail_map_data.len(),
    "Normal maps must be the same size"
  );
  assert_eq!(base_map_data.len() % 4, 0, "Pixel data must be RGBA");

  let mut out = Vec::with_capacity(base_map_data.len());

  for i in 0..base_map_data.len() / 4 {
    let base_pixel = [
      base_map_data[i * 4],
      base_map_data[i * 4 + 1],
      base_map_data[i * 4 + 2],
    ];
    let detail_pixel = [
      detail_map_data[i * 4],
      detail_map_data[i * 4 + 1],
      detail_map_data[i * 4 + 2],
    ];

    let base_ts = decode_ts(base_pixel);
    let detail_ts = decode_ts(detail_pixel);

    let blended_ts = blend_rnm(base_ts, detail_ts);

    // lerp between the base and the blended normal
    let final_ts = base_ts.lerp(&blended_ts, weight);

    let final_pixel = encode_ts(final_ts);
    out.push(final_pixel[0]);
    out.push(final_pixel[1]);
    out.push(final_pixel[2]);
    out.push(255);
  }

  out
}
