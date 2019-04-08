pub mod gfx;
pub mod pam;

pub(crate) mod decoder {
  pub(crate) mod shape_decoder;
}

pub use decoder::shape_decoder::{decode_shape, Shape, StyledPath};


#[cfg(test)]
mod renderer_tests {
  use std::path::Path;

  use ::swf_tree::tags::DefineShape;

  use ::test_generator::test_expand_paths;

  use crate::decode_shape;
  use std::io::Write;

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
}
