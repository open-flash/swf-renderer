use swf_tree;

pub trait SwfRenderer {
  fn render(&mut self, stage: Stage) -> ();
}

#[derive(Debug, Clone)]
pub struct Stage {
  pub background_color: swf_tree::StraightSRgba8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
  pub position: [f32; 3],
  pub color: [f32; 3],
}
