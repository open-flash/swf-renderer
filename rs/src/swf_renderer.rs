use crate::stage::Stage;

pub trait SwfRenderer {
  fn render(&mut self, stage: Stage) -> ();
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
  pub position: [f32; 3],
  pub color: [f32; 3],
}
