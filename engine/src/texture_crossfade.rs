use wasm_bindgen::prelude::*;

static mut TEXTURE_PTRS: [Option<Vec<u8>>; 64] = [
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
  None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
];

#[wasm_bindgen]
pub fn crossfade_set_texture(data: Vec<u8>, index: usize) {
  unsafe { TEXTURE_PTRS[index] = Some(data) }
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
fn get_texture_indices_for_corner(
  texture_count: usize,
  base_texture_ix: usize,
  x_side: i8,
  y_side: i8,
) -> (usize, usize, usize, usize) {
  let get_prev_ix = |cur_ix: usize| {
    if cur_ix == 0 {
      texture_count - 1
    } else {
      cur_ix - 1
    }
  };
  let get_next_ix = |cur_ix: usize| {
    if cur_ix == texture_count - 1 {
      0
    } else {
      cur_ix + 1
    }
  };

  let prev_ix = get_prev_ix(base_texture_ix as usize);
  let prev_prev_ix = get_prev_ix(prev_ix);
  let next_ix = get_next_ix(base_texture_ix as usize);
  let next_next_ix = get_next_ix(next_ix);

  match (x_side, y_side) {
    (-1, -1) => (prev_prev_ix, prev_ix, prev_ix, base_texture_ix),
    (-1, 1) => (prev_ix, base_texture_ix, base_texture_ix, next_ix),
    (1, -1) => (prev_ix, base_texture_ix, base_texture_ix, next_ix),
    (1, 1) => (base_texture_ix, next_ix, next_ix, next_next_ix),
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

#[wasm_bindgen]
pub fn crossfade_generate(width: usize, height: usize, threshold: f32, debug: bool) -> Vec<u8> {
  if threshold < 0. || threshold > 1. {
    panic!("Threshold must be between 0 and 1");
  }

  let textures = unsafe { &TEXTURE_PTRS }
    .iter()
    .take_while(|x| x.is_some())
    .map(|data| data.as_ref().unwrap())
    .collect::<Vec<_>>();

  // textures count must be a perfect square
  let texture_count = textures.len();
  let texture_count_sqrt = (texture_count as f32).sqrt() as usize;
  if texture_count_sqrt * texture_count_sqrt != texture_count {
    panic!("Texture count must be a perfect square");
  }

  let out_width = width * textures.len();
  let out_height = height * textures.len();
  let mut out: Vec<u8> = Vec::with_capacity(out_width * out_height * 4);
  for y in 0..out_height {
    let y_cur_tile_progress = (y % height) as f32 / height as f32;
    let y_cur_tile = y / height;

    for x in 0..out_width {
      let x_cur_tile_progress = (x % width) as f32 / width as f32;
      let x_cur_tile = x / width;
      let base_tx_ix = (x_cur_tile + y_cur_tile) % textures.len();
      let base_texture_ix = {
        let x = x % width;
        let y = y % height;
        y * height * 4 + x * 4
      };

      let (normalized_x, normalized_y, x_side, y_side) =
        match project_box_coord(x_cur_tile_progress, y_cur_tile_progress, threshold) {
          o => o,
        };
      let normalized_x = (normalized_x + 1.) / 2.;
      let normalized_y = (normalized_y + 1.) / 2.;

      let (top_left_ix, top_right_ix, bot_left_ix, bot_right_ix) =
        get_texture_indices_for_corner(textures.len(), base_tx_ix, x_side, y_side);

      let (tl_sample, tr_sample, bl_sample, br_sample) = if debug {
        (
          get_debug_color(top_left_ix),
          get_debug_color(top_right_ix),
          get_debug_color(bot_left_ix),
          get_debug_color(bot_right_ix),
        )
      } else {
        let top_left_texture = textures[top_left_ix];
        let tl_sample = [
          top_left_texture[base_texture_ix + 0] as f32,
          top_left_texture[base_texture_ix + 1] as f32,
          top_left_texture[base_texture_ix + 2] as f32,
        ];
        let top_right_texture = textures[top_right_ix];
        let tr_sample = [
          top_right_texture[base_texture_ix + 0] as f32,
          top_right_texture[base_texture_ix + 1] as f32,
          top_right_texture[base_texture_ix + 2] as f32,
        ];
        let bot_left_texture = textures[bot_left_ix];
        let bl_sample = [
          bot_left_texture[base_texture_ix + 0] as f32,
          bot_left_texture[base_texture_ix + 1] as f32,
          bot_left_texture[base_texture_ix + 2] as f32,
        ];
        let bot_right_texture = textures[bot_right_ix];
        let br_sample = [
          bot_right_texture[base_texture_ix + 0] as f32,
          bot_right_texture[base_texture_ix + 1] as f32,
          bot_right_texture[base_texture_ix + 2] as f32,
        ];
        (tl_sample, tr_sample, bl_sample, br_sample)
      };

      // bilinear interpolation
      let top_sample = [
        tl_sample[0] * (1. - normalized_x) + tr_sample[0] * normalized_x,
        tl_sample[1] * (1. - normalized_x) + tr_sample[1] * normalized_x,
        tl_sample[2] * (1. - normalized_x) + tr_sample[2] * normalized_x,
      ];
      let bot_sample = [
        bl_sample[0] * (1. - normalized_x) + br_sample[0] * normalized_x,
        bl_sample[1] * (1. - normalized_x) + br_sample[1] * normalized_x,
        bl_sample[2] * (1. - normalized_x) + br_sample[2] * normalized_x,
      ];
      let sample = [
        top_sample[0] * (1. - normalized_y) + bot_sample[0] * normalized_y,
        top_sample[1] * (1. - normalized_y) + bot_sample[1] * normalized_y,
        top_sample[2] * (1. - normalized_y) + bot_sample[2] * normalized_y,
      ];

      out.extend_from_slice(&[sample[0] as u8, sample[1] as u8, sample[2] as u8, 255]);
    }
  }

  out
}
