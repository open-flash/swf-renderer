use swf_tree::tags::{DefineShape, DefineMorphShape};

#[derive(Debug, Clone)]
pub struct ShapeId(pub usize);

#[derive(Debug, Clone)]
pub struct MorphShapeId(pub usize);

pub trait ClientAssetStore {
  fn register_shape(&mut self, tag: DefineShape) -> ShapeId;
  fn register_morph_shape(&mut self, tag: DefineMorphShape) -> MorphShapeId;
}

pub trait ServerAssetStore<Shape, MorphShape> {
  fn get_shape(&self, id: ShapeId) -> Option<Shape>;
  fn get_morph_shape(&self, id: MorphShapeId) -> Option<MorphShape>;
}
