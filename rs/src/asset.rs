use swf_tree::tags::{DefineMorphShape, DefineShape};

#[derive(Debug, Clone, Copy)]
pub struct ShapeId(pub usize);

#[derive(Debug, Clone, Copy)]
pub struct MorphShapeId(pub usize);

pub trait ClientAssetStore {
  fn register_shape(&mut self, tag: &DefineShape) -> ShapeId;
  fn register_morph_shape(&mut self, tag: &DefineMorphShape) -> MorphShapeId;
}

pub trait ServerAssetStore {
  type Shape;
  type MorphShape;

  fn get_shape(&mut self, id: ShapeId) -> Option<Self::Shape>;
  fn get_morph_shape(&mut self, id: MorphShapeId) -> Option<Self::MorphShape>;
}
