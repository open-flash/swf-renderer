use ::gfx_backend_vulkan as back;
use swf_renderer::stage::{Stage, DisplayPrimitive, StoredShape, Matrix2D};
use swf_renderer::asset::ClientAssetStore;
use swf_renderer::SwfRenderer;
use swf_renderer::WebRenderer;
use swf_tree::StraightSRgba8;
use swf_tree::tags::DefineShape;

fn main() {
  let args: Vec<String> = std::env::args().collect();
  if args.len() < 2 {
    eprintln!("Missing input path");
    return;
  }

  let file_path = &args[1];
  let json_file = ::std::fs::File::open(file_path).expect("Failed to open file");
  let reader = ::std::io::BufReader::new(json_file);
  let tag: DefineShape = serde_json::from_reader(reader).expect("Failed to read tag");

  println!("{:?}", &tag);

  env_logger::init();
  let event_loop = winit::event_loop::EventLoop::new();
  let dpi = event_loop.primary_monitor().hidpi_factor();
  let wb = winit::window::WindowBuilder::new()
    .with_min_inner_size(winit::dpi::LogicalSize::new(1.0, 1.0))
    .with_inner_size(winit::dpi::LogicalSize::from_physical(
      winit::dpi::PhysicalSize::new(1024.0, 768.0),
      dpi,
    ))
    .with_title("swf-renderer".to_string());

  let (_window, adapter, surface) = {
    let window = wb.build(&event_loop).unwrap();
    let instance = back::Instance::create("ofl-swf-renderer", 1).expect("Failed to create instance");
    let surface = instance.create_surface(&window).expect("Failed to create surface");
    let adapter = WebRenderer::get_adapter(&instance, &surface).expect("Failed to find adapter with graphics support");
    // Return `window` so it is not dropped: dropping it invalidates `surface`.
    (window, adapter, surface)
  };
  let mut renderer: WebRenderer<back::Backend> = WebRenderer::new(adapter, surface);
  let shape_id = renderer.register_shape(&tag);

  event_loop.run(move |event, _, control_flow| {
    *control_flow = winit::event_loop::ControlFlow::Wait;

    match event {
      winit::event::Event::WindowEvent { event, .. } => match event {
        winit::event::WindowEvent::CloseRequested => *control_flow = winit::event_loop::ControlFlow::Exit,
        winit::event::WindowEvent::KeyboardInput {
          input:
          winit::event::KeyboardInput {
            virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
            ..
          },
          ..
        } => *control_flow = winit::event_loop::ControlFlow::Exit,
        winit::event::WindowEvent::Resized(dims) => {
          println!("resized to {:?}", dims);
        }
        _ => {}
      },
      winit::event::Event::EventsCleared => {
        let stage: Stage = Stage {
          background_color: StraightSRgba8 {
            r: 255,
            g: 255,
            b: 0,
            a: 255,
          },
          display_root: vec![
            DisplayPrimitive::Shape(StoredShape {
              id: shape_id,
              matrix: Matrix2D::default()
            })
          ],
        };
        renderer.render(stage);
      }
      _ => {}
    }
  });
}
