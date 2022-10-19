use wasm_bindgen::prelude::*;

static mut TEXTURE_PTRS: [Option<Vec<u8>>; 64] = [
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
];
static mut TEXTURE_AVERAGE_COLORS: [[f32; 3]; 64] = [[0.0; 3]; 64];

static mut TEXTURE_INDICES: [usize; 64] = [0; 64];
static mut TEXTURE_ROTATIONS: [u8; 64] = [0; 64];
static mut TEXTURE_OFFSETS_X: [usize; 64] = [0; 64];
static mut TEXTURE_OFFSETS_Y: [usize; 64] = [0; 64];

fn compute_average_color(data: &[u8]) -> [f32; 3] {
  let mut sum = [0.0; 3];
  for i in 0..data.len() / 4 {
    sum[0] += data[i * 4] as f32;
    sum[1] += data[i * 4 + 1] as f32;
    sum[2] += data[i * 4 + 2] as f32;
  }
  sum[0] /= data.len() as f32 / 4.0;
  sum[1] /= data.len() as f32 / 4.0;
  sum[2] /= data.len() as f32 / 4.0;
  sum
}

#[wasm_bindgen]
pub fn crossfade_set_texture(data: Vec<u8>, index: usize) {
  unsafe {
    TEXTURE_AVERAGE_COLORS[index] = compute_average_color(&data);
    TEXTURE_PTRS[index] = Some(data);
  }
}

#[wasm_bindgen]
pub fn crossfade_set_texture_indices(indices: &[usize]) {
  for i in 0..indices.len() {
    unsafe { TEXTURE_INDICES[i] = indices[i] }
  }
}

#[wasm_bindgen]
pub fn crossfade_set_texture_rotations(rotations: &[u8]) {
  for i in 0..rotations.len() {
    unsafe { TEXTURE_ROTATIONS[i] = rotations[i] }
  }
}

#[wasm_bindgen]
pub fn crossfade_set_texture_offsets(offsets_x: &[usize], offsets_y: &[usize]) {
  for i in 0..offsets_x.len() {
    unsafe { TEXTURE_OFFSETS_X[i] = offsets_x[i] }
  }
  for i in 0..offsets_y.len() {
    unsafe { TEXTURE_OFFSETS_Y[i] = offsets_y[i] }
  }
}

struct TileDescriptor<'a> {
  width: usize,
  height: usize,
  tex_ix: usize,
  data: &'a [u8],
  average_color: [f32; 3],
  rotation: u8,
  offset_x: usize,
  offset_y: usize,
}

#[wasm_bindgen]
pub fn crossfade_reset() {
  console_error_panic_hook::set_once();

  unsafe {
    for i in 0..TEXTURE_PTRS.len() {
      TEXTURE_PTRS[i] = None;
    }
  }
}

/// Projects the coordinates from [0, 0], [1, 1] relative to the current tile to
/// coordinates relative to a corner of a tile from [-1, -1], [1, 1] within
/// `threshold` of that corner.
fn project_box_coord(x: f32, y: f32, threshold: f32) -> (f32, f32, i8, i8) {
  let half_threshold = threshold / 2.;

  let x_side;
  let y_side;

  let normalized_x = if x < half_threshold {
    x_side = -1;
    x / half_threshold
  } else if x > 1. - half_threshold {
    x_side = 1;
    (x - 1.) / half_threshold
  } else {
    if x < 0.5 {
      x_side = -1;
      1.
    } else {
      x_side = 1;
      -1.
    }
  };
  let normalized_y = if y < half_threshold {
    y_side = -1;
    y / half_threshold
  } else if y > 1. - half_threshold {
    y_side = 1;
    (y - 1.) / half_threshold
  } else {
    if y < 0.5 {
      y_side = -1;
      1.
    } else {
      y_side = 1;
      -1.
    }
  };

  (normalized_x, normalized_y, x_side, y_side)
}

#[test]
fn project_box_coord_correctness() {
  let threshold = 0.2;
  assert_eq!(project_box_coord(0., 0., threshold), (0., 0., -1, -1));
  assert_eq!(project_box_coord(1., 1., threshold), (0., 0., 1, 1));
  assert_eq!(project_box_coord(0.5, 0.5, threshold), (-1., -1., 1, 1));
  assert_eq!(
    project_box_coord(0.05, 0.95, threshold),
    (0.5, -0.5000001, -1, 1)
  );
}

/// Returns the indices around the current corneras  (top left, top right,
/// bottom left, bottom right)
///
/// TODO: Probably would be good to make a LUT for this
fn get_tile_indices_for_corner(
  tile_count: usize,
  (tile_x, tile_y): (usize, usize),
  (x_side, y_side): (i8, i8),
) -> (usize, usize, usize, usize) {
  let inc_wrap = |x: usize| if x == tile_count - 1 { 0 } else { x + 1 };

  let dec_wrap = |x: usize| if x == 0 { tile_count - 1 } else { x - 1 };

  let coords_to_ix = |x: usize, y: usize| y * tile_count + x;

  match (x_side, y_side) {
    (-1, -1) => (
      coords_to_ix(dec_wrap(tile_x), dec_wrap(tile_y)),
      coords_to_ix(tile_x, dec_wrap(tile_y)),
      coords_to_ix(dec_wrap(tile_x), tile_y),
      coords_to_ix(tile_x, tile_y),
    ),
    (-1, 1) => (
      coords_to_ix(dec_wrap(tile_x), tile_y),
      coords_to_ix(tile_x, tile_y),
      coords_to_ix(dec_wrap(tile_x), inc_wrap(tile_y)),
      coords_to_ix(tile_x, inc_wrap(tile_y)),
    ),
    (1, -1) => (
      coords_to_ix(tile_x, dec_wrap(tile_y)),
      coords_to_ix(inc_wrap(tile_x), dec_wrap(tile_y)),
      coords_to_ix(tile_x, tile_y),
      coords_to_ix(inc_wrap(tile_x), tile_y),
    ),
    (1, 1) => (
      coords_to_ix(tile_x, tile_y),
      coords_to_ix(inc_wrap(tile_x), tile_y),
      coords_to_ix(tile_x, inc_wrap(tile_y)),
      coords_to_ix(inc_wrap(tile_x), inc_wrap(tile_y)),
    ),
    _ => unreachable!(),
  }
}

fn get_debug_color(ix: usize) -> [f32; 3] {
  match ix % 8 {
    0 => [255., 0., 0.],
    1 => [0., 255., 0.],
    2 => [0., 0., 255.],
    3 => [255., 255., 0.],
    4 => [255., 0., 255.],
    5 => [0., 255., 255.],
    6 => [255., 255., 255.],
    7 => [0., 0., 0.],
    _ => unreachable!(),
  }
}

fn read_texture(desc: &TileDescriptor, x: usize, y: usize) -> [f32; 3] {
  let x = ((x + desc.offset_x) % desc.width) as isize;
  let y = ((y + desc.offset_y) % desc.height) as isize;

  // rotation 0: 0 degrees
  // rotation 1: 90 degrees
  // rotation 2: 180 degrees
  // rotation 3: 270 degrees
  //
  // textures wrap around, so we need to do some extra work to make sure we
  // don't read out of bounds

  let transformed_x = if desc.rotation == 0 {
    x
  } else if desc.rotation == 1 {
    y
  } else if desc.rotation == 2 {
    desc.width as isize - x - 1
  } else if desc.rotation == 3 {
    desc.height as isize - y - 1
  } else {
    if cfg!(debug_assertions) {
      unreachable!()
    } else {
      0
    }
  };
  let transformed_x = if transformed_x < 0 {
    ((desc.width as isize + transformed_x) % desc.width as isize) as usize
  } else {
    (transformed_x as usize) % desc.width
  };

  let transformed_y = if desc.rotation == 0 {
    y
  } else if desc.rotation == 1 {
    desc.height as isize - x - 1
  } else if desc.rotation == 2 {
    desc.height as isize - y - 1
  } else if desc.rotation == 3 {
    x
  } else {
    unreachable!()
  };
  let transformed_y = if transformed_y < 0 {
    ((desc.height as isize + transformed_y) % desc.height as isize) as usize
  } else {
    (transformed_y as usize) % desc.height
  };

  let ix = (transformed_y * desc.width + transformed_x) * 4;
  if cfg!(debug_assertions) {
    [
      desc.data[ix] as f32,
      desc.data[ix + 1] as f32,
      desc.data[ix + 2] as f32,
    ]
  } else {
    unsafe {
      [
        *desc.data.get_unchecked(ix) as f32,
        *desc.data.get_unchecked(ix + 1) as f32,
        *desc.data.get_unchecked(ix + 2) as f32,
      ]
    }
  }
}

/// Based off of strategy from https://www.shadertoy.com/view/lsKGz3#
fn contrast_correct(sample: [f32; 3], avg_color: [f32; 3], opacity_parts_squared: f32) -> [f32; 3] {
  let divisor = opacity_parts_squared.sqrt();
  let mut out = avg_color;
  out[0] += (sample[0] - avg_color[0]) / divisor;
  out[1] += (sample[1] - avg_color[1]) / divisor;
  out[2] += (sample[2] - avg_color[2]) / divisor;
  out
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
  [
    a[0] * (1. - t) + b[0] * t,
    a[1] * (1. - t) + b[1] * t,
    a[2] * (1. - t) + b[2] * t,
  ]
}

#[wasm_bindgen]
pub fn crossfade_generate(
  width: usize,
  height: usize,
  tile_count: usize,
  threshold: f32,
  debug: bool,
  contrast_correction_factor: f32,
) -> Vec<u8> {
  if threshold < 0. || threshold > 1. {
    panic!("Threshold must be between 0 and 1");
  }

  let tiles = (0..(tile_count * tile_count))
    .map(|tile_ix| {
      let tex_ix = unsafe { &TEXTURE_INDICES }[tile_ix];
      let data = unsafe { &TEXTURE_PTRS }[tex_ix].as_ref().unwrap();

      TileDescriptor {
        width,
        height,
        tex_ix,
        data,
        average_color: unsafe { TEXTURE_AVERAGE_COLORS[tex_ix] },
        rotation: unsafe { TEXTURE_ROTATIONS[tile_ix] },
        offset_x: unsafe { TEXTURE_OFFSETS_X[tile_ix] },
        offset_y: unsafe { TEXTURE_OFFSETS_Y[tile_ix] },
      }
    })
    .collect::<Vec<_>>();

  let out_width = width * tile_count;
  let out_height = height * tile_count;
  let mut out: Vec<u8> = Vec::with_capacity(out_width * out_height * 4);
  for y in 0..out_height {
    let cur_tile_y = y % height;
    let y_cur_tile_progress = (y % height) as f32 / height as f32;
    let y_cur_tile = y / height;

    for x in 0..out_width {
      let cur_tile_x = x % width;
      let x_cur_tile_progress = (x % width) as f32 / width as f32;
      let x_cur_tile = x / width;

      let (normalized_x, normalized_y, x_side, y_side) =
        project_box_coord(x_cur_tile_progress, y_cur_tile_progress, threshold);
      let normalized_x = (normalized_x + 1.) / 2.;
      let normalized_y = (normalized_y + 1.) / 2.;

      let (tl_tile_ix, tr_tile_ix, bl_tile_ix, br_tile_ix) =
        get_tile_indices_for_corner(tile_count, (x_cur_tile, y_cur_tile), (x_side, y_side));
      let tl_tile = &tiles[tl_tile_ix];
      let tr_tile = &tiles[tr_tile_ix];
      let bl_tile = &tiles[bl_tile_ix];
      let br_tile = &tiles[br_tile_ix];

      let (tl_sample, tr_sample, bl_sample, br_sample) = if debug {
        (
          get_debug_color(tl_tile.tex_ix),
          get_debug_color(tr_tile.tex_ix),
          get_debug_color(bl_tile.tex_ix),
          get_debug_color(br_tile.tex_ix),
        )
      } else {
        let tl_sample = read_texture(tl_tile, cur_tile_x, cur_tile_y);
        let tr_sample = read_texture(tr_tile, cur_tile_x, cur_tile_y);
        let bl_sample = read_texture(bl_tile, cur_tile_x, cur_tile_y);
        let br_sample = read_texture(br_tile, cur_tile_x, cur_tile_y);
        (tl_sample, tr_sample, bl_sample, br_sample)
      };

      let horizontal_opacity_parts_squared =
        normalized_x * normalized_x + (1. - normalized_x) * (1. - normalized_x);

      let mut top_sample = mix(tl_sample, tr_sample, normalized_x);
      let top_avg_color = mix(tl_tile.average_color, tr_tile.average_color, normalized_x);
      let contrast_corredted_top_sample =
        contrast_correct(top_sample, top_avg_color, horizontal_opacity_parts_squared);
      top_sample = mix(
        top_sample,
        contrast_corredted_top_sample,
        contrast_correction_factor,
      );

      let mut bot_sample = mix(bl_sample, br_sample, normalized_x);
      let bot_avg_color = mix(bl_tile.average_color, br_tile.average_color, normalized_x);
      let contrast_corrected_bot_sample =
        contrast_correct(bot_sample, bot_avg_color, horizontal_opacity_parts_squared);
      bot_sample = mix(
        bot_sample,
        contrast_corrected_bot_sample,
        contrast_correction_factor,
      );

      let mut sample = mix(top_sample, bot_sample, normalized_y);
      let avg_color = mix(top_avg_color, bot_avg_color, normalized_y);
      let all_opacity_parts_squared = normalized_x * normalized_x * normalized_y
        + (1. - normalized_x) * (1. - normalized_x) * normalized_y
        + normalized_x * normalized_x * (1. - normalized_y)
        + (1. - normalized_x) * (1. - normalized_x) * (1. - normalized_y);
      let contrast_corrected_sample =
        contrast_correct(sample, avg_color, all_opacity_parts_squared);
      sample = mix(
        sample,
        contrast_corrected_sample,
        contrast_correction_factor,
      );

      out.extend_from_slice(&[sample[0] as u8, sample[1] as u8, sample[2] as u8, 255]);
    }
  }

  out
}
