use swf_tree::StraightSRgba8;
use crate::asset::{ShapeId, MorphShapeId};

/// Represents a stage state
#[derive(Debug, Clone)]
pub struct Stage {
  pub background_color: StraightSRgba8,
  pub display_root: Vec<DisplayPrimitive>,
}

/// Represents a 2D transformation matrix.
///
/// The coefficients are ordered as in `swf_tree::Matrix` and represent the following matrix:
/// ```txt
/// [c0 c3 c4]
/// [c2 c1 c5]
/// [0  0  1 ]
/// ```
#[derive(Debug, Clone)]
pub struct Matrix2D(pub [f32; 6]);

/// Represents the interpolation ratio of a morph shape.
///
/// A value of `0` indicates that the shape is in its start state.
/// A value of `core::16::MAX` indicates that the shape is its end state.
/// Intermediate values correspond to a linear interpolation between these two states.
#[derive(Debug, Clone)]
pub struct MorphRatio(pub u16);

/// Represents a static shape retrieved from the asset store.
///
/// The shape must first be registered with `register_shape`
#[derive(Debug, Clone)]
pub struct StoredShape {
  pub id: ShapeId,
  pub matrix: Matrix2D,
}

/// Represents a morph shape retrieved from the asset store.
///
/// The shape must first be registered with `register_morph_shape`
#[derive(Debug, Clone)]
pub struct StoredMorphShape {
  pub id: MorphShapeId,
  pub matrix: Matrix2D,
  pub ratio: MorphRatio,
}

#[derive(Debug, Clone)]
pub enum DisplayPrimitive {
  Shape(StoredShape),
  MorphShape(StoredMorphShape),
}
