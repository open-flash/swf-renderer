use std::collections::HashMap;

use lyon::tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use swf_tree::FillStyle;

use crate::{decode_shape};
use crate::swf_renderer::Vertex;

/// Structure holding all the shape and morph-shape definitions in a
/// format optimized for the renderer.
pub struct ShapeStore {
  shapes: HashMap<usize, GfxSymbol>,
}

impl ShapeStore {
  pub fn new() -> Self {
    Self { shapes: HashMap::new() }
  }

  pub fn get(&self, id: usize) -> Option<&GfxSymbol> {
    self.shapes.get(&id)
  }

  pub fn define_shape(&mut self, tag: &swf_tree::tags::DefineShape) -> usize {
    let id: usize = tag.id.into();
    let shape = decode_shape(&tag.shape);
    let mut mesh: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();

    for path in shape.paths.iter() {
      let color: [f32; 3] = if let Some(ref fill) = &path.fill {
        match fill {
          FillStyle::Solid(ref style) => [
            (style.color.r as f32) / 255f32,
            (style.color.g as f32) / 255f32,
            (style.color.b as f32) / 255f32,
          ],
          _ => [0.0, 1.0, 0.0],
        }
      } else {
        [1.0, 0.0, 0.0]
      };

      // Compute the tessellation.
      tessellator.tessellate_path(
        &path.path,
        &FillOptions::default(),
        &mut BuffersBuilder::new(&mut mesh, |vertex: FillVertex| {
          Vertex {
            position: [vertex.position.x, vertex.position.y, 0.0],
            color,
          }
        }),
      ).unwrap();
    }

    let shape_symbol = GfxShapeSymbol {bounds: tag.bounds, mesh};
    let old = self.shapes.insert(id, GfxSymbol::Shape(shape_symbol));
    debug_assert!(old.is_none());
    id
  }
}

pub enum GfxSymbol {
  Shape(GfxShapeSymbol),
  MorphShape(GfxMorphShapeSymbol),
}

pub struct GfxShapeSymbol {
  pub bounds: swf_tree::Rect,
  pub mesh: VertexBuffers<Vertex, u32>,
}

pub struct GfxMorphShapeSymbol {
  // TODO
}

pub enum DisplayItem {
  Shape(usize, swf_tree::Matrix),
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
