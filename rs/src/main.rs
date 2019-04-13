fn main() {
//  env_logger::init();
//
//  let instance: gfx_backend::Instance = gfx_backend::Instance::create(GFX_APP_NAME, GFX_BACKEND_VERSION);
//
//  let mut renderer = HeadlessGfxRenderer::<gfx_backend::Backend>::new(&instance, VIEWPORT_WIDTH as usize, VIEWPORT_HEIGHT as usize)
//    .unwrap();
//
//  let stage: SwfShape = SwfShape {
//    initial_styles: swf_tree::ShapeStyles {
//      fill: Vec::new(),
//      line: Vec::new(),
//    },
//    records: Vec::new(),
//  };
//
//  let shape_id = renderer.define_shape();
//
//  renderer.set_stage(DisplayItem::Shape(stage, swf_tree::Matrix::default()));
//
//  let image = renderer.get_image().unwrap();
//
//  {
//    let pam_file = ::std::fs::File::create("out.pam").expect("Failed to create actual AST file");
//    let mut pam_writer = ::std::io::BufWriter::new(pam_file);
//    pam::write_pam(&mut pam_writer, &image).expect("Failed to write PAM");
//  }

  dbg!("NotImplemented: Read a shape definition and display it");
}
