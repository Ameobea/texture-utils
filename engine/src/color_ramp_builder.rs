use wasm_bindgen::prelude::*;

#[derive(Debug)]
pub struct ColorRampStep {
  /// Must always be [0, 255].  Stored as f32 to avoid having to cast to float for interpolation
  pub color: [f32; 3],
  /// Must always be [0, 1]
  pub position: f32,
}

#[derive(Debug)]
pub struct ColorRamp {
  pub steps: Vec<ColorRampStep>,
}

impl ColorRamp {
  /// Val must be in [0, 1]
  pub fn apply(&self, val: f32) -> [u8; 3] {
    let mut i = 0;
    while i < self.steps.len() && self.steps[i].position < val {
      i += 1;
    }

    if i == 0 {
      return [
        self.steps[0].color[0] as u8,
        self.steps[0].color[1] as u8,
        self.steps[0].color[2] as u8,
      ];
    } else if i == self.steps.len() {
      return [
        self.steps[self.steps.len() - 1].color[0] as u8,
        self.steps[self.steps.len() - 1].color[1] as u8,
        self.steps[self.steps.len() - 1].color[2] as u8,
      ];
    }

    let prev = &self.steps[i - 1];
    let next = &self.steps[i];
    let t = (val - prev.position) / (next.position - prev.position);

    let r = prev.color[0] + t * (next.color[0] - prev.color[0]);
    let g = prev.color[1] + t * (next.color[1] - prev.color[1]);
    let b = prev.color[2] + t * (next.color[2] - prev.color[2]);

    [r as u8, g as u8, b as u8]
  }
}

struct InputTexture {
  data: Vec<u8>,
  grayscale: Vec<f32>,
}

static mut INPUT_TEXTURE: *mut InputTexture = std::ptr::null_mut();

fn convert_image_to_grayscale(img_pixel_data: &[u8]) -> Vec<f32> {
  let mut grayscale = Vec::with_capacity(img_pixel_data.len() / 4);

  for i in 0..img_pixel_data.len() / 4 {
    let r = img_pixel_data[i * 4] as f32;
    let g = img_pixel_data[i * 4 + 1] as f32;
    let b = img_pixel_data[i * 4 + 2] as f32;
    grayscale.push(0.2126 * r + 0.7152 * g + 0.0722 * b);
  }

  grayscale
}

#[wasm_bindgen]
pub fn color_ramp_set_input_texture(input_texture: Vec<u8>) {
  console_error_panic_hook::set_once();

  // expects RGBA
  assert_eq!(input_texture.len() % 4, 0, "Pixel data must be RGBA");

  let state = InputTexture {
    data: input_texture.clone(),
    grayscale: convert_image_to_grayscale(&input_texture),
  };
  unsafe {
    if !INPUT_TEXTURE.is_null() {
      drop(Box::from_raw(INPUT_TEXTURE));
    }
    INPUT_TEXTURE = Box::into_raw(Box::new(state));
  }
}

/// Ramp def format: [r, g, b, position, r, g, b, position, ...]
///
/// Position must be in [0, 1]
/// R, G, B must be in [0, 255]
fn parse_color_ramp(ramp_def: Vec<f32>) -> ColorRamp {
  let mut steps = Vec::with_capacity(ramp_def.len() / 4);

  for i in 0..ramp_def.len() / 4 {
    let color = [ramp_def[i * 4], ramp_def[i * 4 + 1], ramp_def[i * 4 + 2]];
    let position = ramp_def[i * 4 + 3];

    steps.push(ColorRampStep { color, position });
  }

  ColorRamp { steps }
}

#[wasm_bindgen]
pub fn color_ramp_apply_ramp(ramp_def: Vec<f32>) -> Vec<u8> {
  if unsafe { INPUT_TEXTURE.is_null() } {
    panic!("Input texture not set");
  }
  let input_texture = unsafe { &*INPUT_TEXTURE };

  let ramp = parse_color_ramp(ramp_def);
  let mut output = Vec::with_capacity(input_texture.data.len());

  for i in 0..input_texture.grayscale.len() {
    let val = input_texture.grayscale[i] / 255.;
    let color = ramp.apply(val);
    output.push(color[0]);
    output.push(color[1]);
    output.push(color[2]);
    output.push(255);
  }

  output
}

#[wasm_bindgen]
pub fn color_ramp_get_grayscale_image_data() -> Vec<u8> {
  if unsafe { INPUT_TEXTURE.is_null() } {
    panic!("Input texture not set");
  }
  let input_texture = unsafe { &*INPUT_TEXTURE };

  input_texture.data.clone()
}
