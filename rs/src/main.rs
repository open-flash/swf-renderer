use env_logger;
use gfx_backend_vulkan as gfx_backend;
use swf_tree::Shape as SwfShape;

use swf_renderer::headless_renderer::HeadlessGfxRenderer;
use swf_renderer::pam;
use swf_renderer::renderer::Renderer;

const GFX_APP_NAME: &'static str = "ofl-renderer";
const GFX_BACKEND_VERSION: u32 = 1;
const VIEWPORT_WIDTH: u32 = 256;
const VIEWPORT_HEIGHT: u32 = 256;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Vertex {
  pub position: [f32; 3],
  pub color: [f32; 3],
}

fn main() {
  env_logger::init();

  let instance: gfx_backend::Instance = gfx_backend::Instance::create(GFX_APP_NAME, GFX_BACKEND_VERSION);

  let mut renderer = HeadlessGfxRenderer::<gfx_backend::Backend>::new(&instance, VIEWPORT_WIDTH as usize, VIEWPORT_HEIGHT as usize)
    .unwrap();

  let stage: SwfShape = SwfShape {
    initial_styles: swf_tree::ShapeStyles {
      fill: Vec::new(),
      line: Vec::new(),
    },
    records: Vec::new(),
  };

  renderer.set_stage(stage);

  let image = renderer.get_image().unwrap();

  let dst_image_data = {
    let mut dst_image_data: Vec<u8> = Vec::new();

    for y in 0..image.meta.height {
      let row_idx: usize = y * image.meta.stride;
      for x in 0..image.meta.width {
        let idx: usize = row_idx + 4 * x;
        dst_image_data.push(image.data[idx + 0]);
        dst_image_data.push(image.data[idx + 1]);
        dst_image_data.push(image.data[idx + 2]);
        dst_image_data.push(image.data[idx + 3]);
      }
    }

    dst_image_data
  };

  {
    let pam_file = ::std::fs::File::create("out.pam").expect("Failed to create actual AST file");
    let mut pam_writer = ::std::io::BufWriter::new(pam_file);
    pam::write_pam(&mut pam_writer, VIEWPORT_WIDTH as usize, VIEWPORT_HEIGHT as usize, &dst_image_data).expect("Failed to write PAM");
  }

  dbg!("done");
}
