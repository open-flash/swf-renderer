use crate::stage::Stage;
use crate::swf_renderer::SwfRenderer;
use crate::GfxRenderer;
use gfx_backend_gl as back;
use log::{error, info};
use std::collections::HashMap;
use swf_tree::StraightSRgba8;
use wasm_bindgen::prelude::*;
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
  static ref GLOBAL_RENDERER_STORE: Mutex<RendererStore> = Mutex::new(RendererStore::new());
}

struct RendererStore {
  next: u64,
  handles: Option<HashMap<RendererHandle, GfxRenderer<back::Backend>>>,
}

impl RendererStore {
  const fn new() -> Self {
    RendererStore { next: 0, handles: None }
  }

  fn add(&mut self, renderer: GfxRenderer<back::Backend>) -> RendererHandle {
    let handle = RendererHandle(self.next);
    self.next += 1;
    let old: Option<GfxRenderer<back::Backend>> = self.handles
      .get_or_insert_with(HashMap::new)
      .insert(handle, renderer);
    assert!(old.is_none(), "Adding the same handle multiple times");
    handle
  }

  fn get_mut(&mut self, handle: RendererHandle) -> Option<&mut GfxRenderer<back::Backend>> {
    match &mut self.handles {
      None => None,
      Some(ref mut handles) => handles.get_mut(&handle),
    }
  }

  fn remove(&mut self, handle: RendererHandle) -> () {
    let old: Option<GfxRenderer<back::Backend>> = match &mut self.handles {
      None => None,
      Some(ref mut handles) => handles.remove(&handle),
    };
    assert!(old.is_some(), "Destroying the same handle multiple times");
  }
}

#[wasm_bindgen(start)]
pub fn wasm_start() {
  std::panic::set_hook(Box::new(console_error_panic_hook::hook));
  console_log::init_with_level(log::Level::Info).unwrap();
}

/// Creates a new renderer and returns its handle.
///
/// Remember to call `destroyRenderer()` to free the associated resources.
#[wasm_bindgen(js_name = createRenderer)]
pub fn create_renderer() -> RendererHandle {
  let window = back::Window;
  let surface = back::Surface::from_window(&window);
  let adapter = GfxRenderer::get_adapter(&surface, &surface).expect("Failed to find a GPU adapter supporting graphics");
  let renderer: GfxRenderer<back::Backend> = GfxRenderer::new(adapter, surface);
  let mut store = GLOBAL_RENDERER_STORE.lock().expect("Failed to acquire global store");
  store.add(renderer)
}

/// Destroys a previously created renderer and frees its resources.
///
/// The handle becomes invalid and should no longer be used.
#[wasm_bindgen(js_name = destroyRenderer)]
pub fn destroy_renderer(handle: RendererHandle) -> () {
  let mut store = GLOBAL_RENDERER_STORE.lock().expect("Failed to acquire global store");
  store.remove(handle)
}

#[wasm_bindgen]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RendererHandle(u64);

#[wasm_bindgen]
impl RendererHandle {
  #[wasm_bindgen(js_name = render)]
  pub fn render(self) -> () {
    self.with_renderer(|r| {
      let stage: Stage = Stage {
        background_color: StraightSRgba8 {
          r: 255,
          g: 0,
          b: 255,
          a: 255,
        },
        display_root: Vec::new(),
      };
      r.render(stage)
    })
  }
}

impl RendererHandle {
  pub fn with_renderer<F: FnOnce(&mut GfxRenderer<back::Backend>) -> ()>(self, f: F) -> () {
    let mut store = GLOBAL_RENDERER_STORE.lock().expect("Failed to acquire global store");
    match store.get_mut(self) {
      None => error!("InvalidRendererHandle"),
      Some(ref mut renderer) => f(renderer)
    }
  }
}
