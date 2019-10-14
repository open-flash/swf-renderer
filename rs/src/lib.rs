#![feature(manually_drop_take)]
#![allow(dead_code)]

pub use crate::gfx_renderer::GfxRenderer;
#[cfg(target_arch = "wasm32")]
use crate::swf_renderer::Stage;
pub use decoder::shape_decoder::{decode_shape, Shape, StyledPath};
#[cfg(target_arch = "wasm32")]
use gfx_backend_gl as back;
#[cfg(target_arch = "wasm32")]
use swf_tree::StraightSRgba8;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub mod asset;
pub mod stage;

mod gfx;
mod gfx_renderer;
#[cfg(not(target_arch = "wasm32"))]
pub mod headless_renderer;
pub mod pam;
pub mod renderer;
pub mod swf_renderer;
pub(crate) mod decoder {
  pub(crate) mod shape_decoder;
}

pub use swf_renderer::SwfRenderer;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_start() {
  std::panic::set_hook(Box::new(console_error_panic_hook::hook));
  console_log::init_with_level(log::Level::Debug).unwrap();

  log::info!("Start");
  let window = back::Window;
  let surface = back::Surface::from_window(&window);
  let adapter = GfxRenderer::get_adapter(&surface, &surface).expect("Failed to find a GPU adapter supporting graphics");
  let mut renderer: GfxRenderer<back::Backend> = GfxRenderer::new(adapter, surface);
  log::info!("Created renderer");
  let stage: Stage = Stage {
    background_color: StraightSRgba8 {
      r: 255,
      g: 0,
      b: 0,
      a: 255,
    },
  };
  renderer.render(stage);
  log::info!("End");
}

#[cfg(test)]
mod renderer_tests {
  use crate::decode_shape;
  use crate::headless_renderer::HeadlessGfxRenderer;
  use crate::pam::write_pam;
  use crate::renderer::DisplayItem;
  use ::swf_tree::tags::DefineShape;
  use ::test_generator::test_resources;
  use gfx_hal::adapter::{Adapter, PhysicalDevice};
  use gfx_hal::command::CommandBuffer;
  use gfx_hal::device::Device;
  use gfx_hal::pool::CommandPool;
  use gfx_hal::pso::DescriptorPool;
  use gfx_hal::queue::{CommandQueue, QueueFamily};
  use gfx_hal::window::{Surface, Swapchain};
  use gfx_hal::Backend;
  use gfx_hal::Instance;
  use std::io::Write;
  use std::path::Path;

  #[test_resources("../tests/flat-shapes/*/")]
  fn test_decode_shape(path: &str) {
    let path: &Path = Path::new(path);
    let _name = path
      .components()
      .last()
      .unwrap()
      .as_os_str()
      .to_str()
      .expect("Failed to retrieve sample name");
    let ast_path = path.join("ast.json");
    let ast_file = ::std::fs::File::open(ast_path).expect("Failed to open AST");
    let ast_reader = ::std::io::BufReader::new(ast_file);
    let ast: DefineShape = serde_json::from_reader(ast_reader).unwrap();

    let shape = decode_shape(&ast.shape);
    let shape_info: String = format!("{:#?}\n", &shape);

    let actual_shape_path = path.join("tmp-shape.rs.log");
    {
      let actual_shape_file = ::std::fs::File::create(actual_shape_path).expect("Failed to create actual shape file");
      let mut actual_shape_writer = ::std::io::BufWriter::new(actual_shape_file);
      actual_shape_writer
        .write_all(shape_info.as_bytes())
        .expect("Failed to write actual shape");
    }

    let expected_shape_info_path = path.join("shape.rs.log");
    let expected_shape_info =
      ::std::fs::read_to_string(expected_shape_info_path).expect("Failed to read expected shape file");

    assert_eq!(shape_info, expected_shape_info);
  }

  fn is_whitelisted(name: &str) -> bool {
    match name {
      "squares" | "triangle" => true,
      _ => false,
    }
  }

  #[test_resources("../tests/flat-shapes/*/")]
  fn test_render_flat_shape(path: &str) {
    use crate::renderer::Renderer;
    use gfx_backend_vulkan as gfx_backend;

    const GFX_APP_NAME: &'static str = "ofl-renderer";
    const GFX_BACKEND_VERSION: u32 = 1;

    let path: &Path = Path::new(path);
    let name = path
      .components()
      .last()
      .unwrap()
      .as_os_str()
      .to_str()
      .expect("Failed to retrieve sample name");

    if !is_whitelisted(&name) {
      eprintln!("Skipping: {}", &name);
      return;
    }

    let ast_path = path.join("ast.json");
    let ast_file = ::std::fs::File::open(ast_path).expect("Failed to open AST");
    let ast_reader = ::std::io::BufReader::new(ast_file);
    let ast: swf_tree::tags::DefineShape = serde_json::from_reader(ast_reader).unwrap();

    let instance: gfx_backend::Instance =
      gfx_backend::Instance::create(GFX_APP_NAME, GFX_BACKEND_VERSION).expect("Failed to create Instance");

    let width_twips = ast.bounds.x_max - ast.bounds.x_min;
    let height_twips = ast.bounds.y_max - ast.bounds.y_min;

    // ceil(_ / 20)
    let width_px = (width_twips / 20) + (if width_twips % 20 == 0 { 0 } else { 1 });
    let height_px = (height_twips / 20) + (if height_twips % 20 == 0 { 0 } else { 1 });

    let mut renderer =
      HeadlessGfxRenderer::<gfx_backend::Backend>::new(&instance, width_px as usize, height_px as usize).unwrap();

    let shape_id = renderer.define_shape(&ast);

    let matrix = {
      let mut matrix = swf_tree::Matrix::default();
      matrix.translate_x = -ast.bounds.x_min;
      matrix.translate_y = -ast.bounds.y_min;
      matrix
    };

    renderer.set_stage(DisplayItem::Shape(shape_id, matrix));

    let image = renderer.get_image().unwrap();

    {
      let actual_shape_path = path.join("tmp-shape.rs.pam");
      let actual_shape_file = ::std::fs::File::create(actual_shape_path).expect("Failed to create actual shape file");
      let mut pam_writer = ::std::io::BufWriter::new(actual_shape_file);
      write_pam(&mut pam_writer, &image).expect("Failed to write PAM");
    }

    //    let expected_shape_info_path = path.join("shape.rs.log");
    //    let expected_shape_info = ::std::fs::read_to_string(expected_shape_info_path)
    //      .expect("Failed to read expected shape file");
    //
    //    assert_eq!(shape_info, expected_shape_info);
  }
}
