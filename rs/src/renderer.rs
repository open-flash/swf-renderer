use swf_tree::Shape as SwfShape;

pub enum DisplayItem {
  Shape(SwfShape, swf_tree::Matrix),
}

pub trait Renderer {
  fn set_stage(&mut self, shape: DisplayItem) -> ();
}

/// Image metadata, format is always standard RGB with alpha (8 bits per channel).
pub struct ImageMetadata {
  /// Width in pixels
  pub width: usize,
  /// Height in pixels
  pub height: usize,
  /// Bytes per row (stride >= width * bytes_per_pixel)
  pub stride: usize,
}

pub struct Image {
  pub meta: ImageMetadata,
  pub data: Vec<u8>,
}
