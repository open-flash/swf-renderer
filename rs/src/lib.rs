pub use decoder::shape_decoder::{decode_shape, Shape, StyledPath};

pub mod gfx;
pub mod pam;
pub mod headless_renderer;
pub mod renderer;

pub(crate) mod decoder {
  pub(crate) mod shape_decoder;
}

#[cfg(test)]
mod renderer_tests {
  use std::io::Write;
  use std::path::Path;

  use ::swf_tree::tags::DefineShape;
  use ::test_generator::test_expand_paths;

  use crate::decode_shape;
  use crate::headless_renderer::HeadlessGfxRenderer;
  use crate::pam::write_pam;
  use crate::renderer::DisplayItem;

  test_expand_paths! { test_decode_shape; "../tests/flat-shapes/*/" }
  fn test_decode_shape(path: &str) {
    let path: &Path = Path::new(path);
    let _name = path.components().last().unwrap().as_os_str().to_str().expect("Failed to retrieve sample name");
    let ast_path = path.join("ast.json");
    let ast_file = ::std::fs::File::open(ast_path).expect("Failed to open AST");
    let ast_reader = ::std::io::BufReader::new(ast_file);
    let ast: DefineShape = serde_json::from_reader(ast_reader).unwrap();

    let shape = decode_shape(&ast.shape);
    let shape_info: String = format!("{:#?}\n", &shape);

    let actual_shape_path = path.join("tmp-shape.rs.log");
    {
      let actual_shape_file = ::std::fs::File::create(actual_shape_path)
        .expect("Failed to create actual shape file");
      let mut actual_shape_writer = ::std::io::BufWriter::new(actual_shape_file);
      actual_shape_writer.write_all(shape_info.as_bytes())
        .expect("Failed to write actual shape");
    }

    let expected_shape_info_path = path.join("shape.rs.log");
    let expected_shape_info = ::std::fs::read_to_string(expected_shape_info_path)
      .expect("Failed to read expected shape file");

    assert_eq!(shape_info, expected_shape_info);
  }

  test_expand_paths! { test_render_flat_shape; "../tests/flat-shapes/*/" }
  fn test_render_flat_shape(path: &str) {
    use gfx_backend_vulkan as gfx_backend;
    use crate::renderer::Renderer;

    const GFX_APP_NAME: &'static str = "ofl-renderer";
    const GFX_BACKEND_VERSION: u32 = 1;

    let path: &Path = Path::new(path);
    let name = path.components().last().unwrap().as_os_str().to_str().expect("Failed to retrieve sample name");

    if name != "triangle" && name != "squares" {
      return;
    }

    let ast_path = path.join("ast.json");
    let ast_file = ::std::fs::File::open(ast_path).expect("Failed to open AST");
    let ast_reader = ::std::io::BufReader::new(ast_file);
    let ast: swf_tree::tags::DefineShape = serde_json::from_reader(ast_reader).unwrap();

    let instance: gfx_backend::Instance = gfx_backend::Instance::create(GFX_APP_NAME, GFX_BACKEND_VERSION);

    let width_twips = ast.bounds.x_max - ast.bounds.x_min;
    let height_twips = ast.bounds.y_max - ast.bounds.y_min;

    // ceil(_ / 20)
    let width_px = (width_twips / 20) + (if width_twips % 20 == 0 { 0 } else { 1 });
    let height_px = (height_twips / 20) + (if height_twips % 20 == 0 { 0 } else { 1 });

    let mut renderer = HeadlessGfxRenderer::<gfx_backend::Backend>::new(&instance, width_px as usize, height_px as usize)
      .unwrap();

    let matrix = {
      let mut matrix = swf_tree::Matrix::default();
      matrix.translate_x = -ast.bounds.x_min;
      matrix.translate_y = -ast.bounds.y_min;
      matrix
    };

    renderer.set_stage(DisplayItem::Shape(ast.shape.clone(), matrix));

    let image = renderer.get_image().unwrap();

    {
      let actual_shape_path = path.join("tmp-shape.rs.pam");
      let actual_shape_file = ::std::fs::File::create(actual_shape_path)
        .expect("Failed to create actual shape file");
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
