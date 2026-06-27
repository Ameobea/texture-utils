use imageproc::edges::canny;
use imageproc::hough::{LineDetectionOptions, PolarLine, detect_lines};
use imageproc::image::GrayImage;
use nalgebra::{Matrix3, SymmetricEigen, Vector3};
use wasm_bindgen::prelude::*;

static mut SRC_PIXELS: Vec<u8> = Vec::new();
static mut SRC_W: usize = 0;
static mut SRC_H: usize = 0;

/// Store the source image to be corrected. Subsequent [`warp_perspective`] / [`detect_perspective`]
/// calls operate on it, avoiding re-transferring the (potentially large) pixel buffer each call.
#[wasm_bindgen]
pub fn warp_set_source(pixels: &[u8], width: u32, height: u32) {
  console_error_panic_hook::set_once();
  let w = width as usize;
  let h = height as usize;
  assert_eq!(pixels.len(), w * h * 4, "Pixel data must be RGBA");
  unsafe {
    SRC_PIXELS = pixels.to_vec();
    SRC_W = w;
    SRC_H = h;
  }
}

fn source() -> (&'static [u8], usize, usize) {
  unsafe {
    assert!(!SRC_PIXELS.is_empty(), "Source image not set");
    (&SRC_PIXELS, SRC_W, SRC_H)
  }
}

fn bilinear_sample(pixels: &[u8], w: usize, h: usize, sx: f64, sy: f64, out: &mut [u8]) {
  let x0 = sx.floor();
  let y0 = sy.floor();
  let fx = (sx - x0) as f32;
  let fy = (sy - y0) as f32;

  let clampx = |x: f64| (x as isize).clamp(0, w as isize - 1) as usize;
  let clampy = |y: f64| (y as isize).clamp(0, h as isize - 1) as usize;
  let x0c = clampx(x0);
  let x1c = clampx(x0 + 1.0);
  let y0c = clampy(y0);
  let y1c = clampy(y0 + 1.0);

  for c in 0..4 {
    let p00 = pixels[(y0c * w + x0c) * 4 + c] as f32;
    let p10 = pixels[(y0c * w + x1c) * 4 + c] as f32;
    let p01 = pixels[(y1c * w + x0c) * 4 + c] as f32;
    let p11 = pixels[(y1c * w + x1c) * 4 + c] as f32;
    let top = p00 + (p10 - p00) * fx;
    let bot = p01 + (p11 - p01) * fx;
    out[c] = (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8;
  }
}

/// Inverse-warp the stored source image through a homography that maps output pixel coordinates to
/// source pixel coordinates (row-major 3x3). Output pixels whose source preimage falls outside the
/// source image are left fully transparent.
#[wasm_bindgen]
pub fn warp_perspective(hom: &[f64], out_width: u32, out_height: u32) -> Vec<u8> {
  console_error_panic_hook::set_once();

  let (pixels, w, h) = source();
  let ow = out_width as usize;
  let oh = out_height as usize;
  assert_eq!(hom.len(), 9, "Homography must have 9 entries");

  let m = Matrix3::new(
    hom[0], hom[1], hom[2], hom[3], hom[4], hom[5], hom[6], hom[7], hom[8],
  );

  let mut out = vec![0u8; ow * oh * 4];
  for oy in 0..oh {
    for ox in 0..ow {
      let p = m * Vector3::new(ox as f64 + 0.5, oy as f64 + 0.5, 1.0);
      if p.z.abs() < 1e-9 {
        continue;
      }
      let sx = p.x / p.z - 0.5;
      let sy = p.y / p.z - 0.5;
      if sx < -0.5 || sy < -0.5 || sx > w as f64 - 0.5 || sy > h as f64 - 0.5 {
        continue;
      }
      let oi = (oy * ow + ox) * 4;
      bilinear_sample(pixels, w, h, sx, sy, &mut out[oi..oi + 4]);
    }
  }
  out
}

struct Norm {
  cx: f64,
  cy: f64,
  s: f64,
}

impl Norm {
  fn new(w: usize, h: usize) -> Self {
    Norm {
      cx: w as f64 / 2.0,
      cy: h as f64 / 2.0,
      s: 2.0 / w.max(h) as f64,
    }
  }

  fn to_norm(&self, px: f64, py: f64) -> Vector3<f64> {
    Vector3::new((px - self.cx) * self.s, (py - self.cy) * self.s, 1.0)
  }

  fn to_px(&self, p: Vector3<f64>) -> (f32, f32) {
    let x = p.x / p.z;
    let y = p.y / p.z;
    ((x / self.s + self.cx) as f32, (y / self.s + self.cy) as f32)
  }
}

fn polar_to_norm_line(line: &PolarLine, n: &Norm) -> Vector3<f64> {
  let theta = (line.angle_in_degrees as f64).to_radians();
  let (lx, ly, lw) = (theta.cos(), theta.sin(), -(line.r as f64));
  let l = Vector3::new(lx, ly, n.s * (lx * n.cx + ly * n.cy + lw));
  l.normalize()
}

fn endpoints_to_norm_line(x1: f64, y1: f64, x2: f64, y2: f64, n: &Norm) -> Vector3<f64> {
  n.to_norm(x1, y1).cross(&n.to_norm(x2, y2)).normalize()
}

/// Vanishing point of a set of (ideally concurrent) lines: the unit vector minimizing the sum of
/// squared point-line incidences, i.e. the eigenvector of the smallest eigenvalue of `Σ lᵢ lᵢᵀ`.
fn estimate_vp(lines: &[Vector3<f64>]) -> Option<Vector3<f64>> {
  if lines.len() < 2 {
    return None;
  }
  let mut m = Matrix3::zeros();
  for l in lines {
    m += l * l.transpose();
  }
  let eig = SymmetricEigen::new(m);
  let mut min_ix = 0;
  for i in 1..3 {
    if eig.eigenvalues[i] < eig.eigenvalues[min_ix] {
      min_ix = i;
    }
  }
  Some(eig.eigenvectors.column(min_ix).into_owned())
}

/// Build the source quad (8 floats, TL/TR/BR/BL pixel coords) whose rectifying homography sends
/// the horizontal vanishing point to horizontal infinity and the vertical one to vertical infinity.
/// Returns `None` if the homography is degenerate or folds the image across the horizon.
fn rectify_quad(mut vp_h: Vector3<f64>, mut vp_v: Vector3<f64>, w: usize, h: usize, n: &Norm) -> Option<[f32; 8]> {
  // `estimate_vp`'s eigenvector has an arbitrary sign; align each VP with its axis so the
  // rectifying homography preserves orientation rather than mirror-flipping at random.
  if vp_h.x < 0.0 {
    vp_h = -vp_h;
  }
  if vp_v.y < 0.0 {
    vp_v = -vp_v;
  }
  let c = Matrix3::from_columns(&[vp_h, vp_v, Vector3::new(0.0, 0.0, 1.0)]);
  if c.determinant() <= 0.0 {
    return None;
  }
  let h_rect = c.try_inverse()?;

  let img_corners = [(0.0, 0.0), (w as f64, 0.0), (w as f64, h as f64), (0.0, h as f64)];
  let mut rect_pts = [(0.0f64, 0.0f64); 4];
  for (i, &(px, py)) in img_corners.iter().enumerate() {
    let q = h_rect * n.to_norm(px, py);
    if q.z <= 0.0 {
      return None;
    }
    rect_pts[i] = (q.x / q.z, q.y / q.z);
  }

  let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
  for &(x, y) in &rect_pts {
    min_x = min_x.min(x);
    min_y = min_y.min(y);
    max_x = max_x.max(x);
    max_y = max_y.max(y);
  }

  let bbox = [(min_x, min_y), (max_x, min_y), (max_x, max_y), (min_x, max_y)];
  let mut quad = [0.0f32; 8];
  for (i, &(bx, by)) in bbox.iter().enumerate() {
    let src = c * Vector3::new(bx, by, 1.0);
    if src.z.abs() < 1e-9 {
      return None;
    }
    let (px, py) = n.to_px(src);
    if !px.is_finite() || !py.is_finite() || px.abs() > 10.0 * w as f32 || py.abs() > 10.0 * h as f32 {
      return None;
    }
    quad[i * 2] = px;
    quad[i * 2 + 1] = py;
  }
  Some(quad)
}

fn to_grayscale(pixels: &[u8], w: usize, h: usize) -> GrayImage {
  let mut gray = Vec::with_capacity(w * h);
  for i in 0..w * h {
    let r = pixels[i * 4] as f32;
    let g = pixels[i * 4 + 1] as f32;
    let b = pixels[i * 4 + 2] as f32;
    gray.push((0.299 * r + 0.587 * g + 0.114 * b).round().clamp(0.0, 255.0) as u8);
  }
  GrayImage::from_raw(w as u32, h as u32, gray).unwrap()
}

/// Detect perspective distortion via Canny + Hough line detection. Lines are split into
/// near-vertical and near-horizontal groups (within `angle_tol_deg` of each axis); each group
/// with 2+ lines yields a vanishing point. Returns `[found, x0,y0,x1,y1,x2,y2,x3,y3]` where the
/// quad (TL/TR/BR/BL, source pixels) seeds the correction handles; `found == 0.0` on failure.
#[wasm_bindgen]
pub fn detect_perspective(
  canny_low: f32,
  canny_high: f32,
  vote_threshold: u32,
  angle_tol_deg: f32,
) -> Vec<f32> {
  console_error_panic_hook::set_once();

  let (pixels, w, h) = source();
  let gray = to_grayscale(pixels, w, h);
  let edges = canny(&gray, canny_low, canny_high);
  let suppression_radius = ((w.max(h) as f32) * 0.02).max(5.0) as u32;
  let lines = detect_lines(
    &edges,
    LineDetectionOptions { vote_threshold, suppression_radius },
  );

  let n = Norm::new(w, h);
  let tol = angle_tol_deg as f64;
  let mut vert = Vec::new();
  let mut horiz = Vec::new();
  for line in &lines {
    let a = line.angle_in_degrees as f64;
    if a <= tol || a >= 180.0 - tol {
      vert.push(polar_to_norm_line(line, &n));
    } else if (a - 90.0).abs() <= tol {
      horiz.push(polar_to_norm_line(line, &n));
    }
  }

  if vert.len() < 2 && horiz.len() < 2 {
    return vec![0.0];
  }

  let vp_v = estimate_vp(&vert).unwrap_or_else(|| Vector3::new(0.0, 1.0, 0.0));
  let vp_h = estimate_vp(&horiz).unwrap_or_else(|| Vector3::new(1.0, 0.0, 0.0));

  match rectify_quad(vp_h, vp_v, w, h, &n) {
    Some(quad) => {
      let mut out = Vec::with_capacity(9);
      out.push(1.0);
      out.extend_from_slice(&quad);
      out
    },
    None => vec![0.0],
  }
}

/// Compute a correction quad from user-drawn guide lines. `endpoints` holds `[x1,y1,x2,y2]` per
/// guide (source pixels); `is_vertical[i] != 0` marks guides that should become vertical (the rest
/// become horizontal). Returns `[found, x0,y0,...,x3,y3]` like [`detect_perspective`].
#[wasm_bindgen]
pub fn solve_guides(endpoints: &[f32], is_vertical: &[u8], width: u32, height: u32) -> Vec<f32> {
  console_error_panic_hook::set_once();

  let w = width as usize;
  let h = height as usize;
  assert_eq!(endpoints.len(), is_vertical.len() * 4, "Each guide needs 4 endpoint coords");

  let n = Norm::new(w, h);
  let mut vert = Vec::new();
  let mut horiz = Vec::new();
  for (i, &v) in is_vertical.iter().enumerate() {
    let l = endpoints_to_norm_line(
      endpoints[i * 4] as f64,
      endpoints[i * 4 + 1] as f64,
      endpoints[i * 4 + 2] as f64,
      endpoints[i * 4 + 3] as f64,
      &n,
    );
    if v != 0 {
      vert.push(l);
    } else {
      horiz.push(l);
    }
  }

  if vert.len() < 2 && horiz.len() < 2 {
    return vec![0.0];
  }

  let vp_v = estimate_vp(&vert).unwrap_or_else(|| Vector3::new(0.0, 1.0, 0.0));
  let vp_h = estimate_vp(&horiz).unwrap_or_else(|| Vector3::new(1.0, 0.0, 0.0));

  match rectify_quad(vp_h, vp_v, w, h, &n) {
    Some(quad) => {
      let mut out = Vec::with_capacity(9);
      out.push(1.0);
      out.extend_from_slice(&quad);
      out
    },
    None => vec![0.0],
  }
}
