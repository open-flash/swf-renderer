use std::collections::HashMap;
use std::mem::ManuallyDrop;

use gfx_hal::command::CommandBuffer;
use gfx_hal::adapter::PhysicalDevice;
use gfx_hal::Backend as GfxBackend;
use gfx_hal::device::Device;
use gfx_hal::image::Extent;
use gfx_hal::pool::CommandPool;
use gfx_hal::queue::CommandQueue;
use gfx_hal::queue::family::QueueFamily;
use nalgebra_glm as glm;

use crate::gfx::{AttachedBuffer, AttachedImage, create_buffer, create_image, create_images, destroy_buffer, destroy_image, get_supported_depth_format};
use crate::renderer::{DisplayItem, GfxSymbol, Image, ImageMetadata, Renderer, ShapeStore};
use std::borrow::Cow;
use crate::swf_renderer::Vertex;

const QUEUE_COUNT: usize = 1;
const VERTEX_SHADER_SOURCE: &'static str = include_str!("shader.vert.glsl");
const FRAGMENT_SHADER_SOURCE: &'static str = include_str!("shader.frag.glsl");


pub struct HeadlessGfxRenderer<B: GfxBackend> {
  pub viewport_extent: Extent,
  pub stage: Option<DisplayItem>,
  pub shape_store: ShapeStore,
  pub shape_meshes: HashMap<usize, ShapeMesh<B>>,

  pub device: B::Device,
  pub queue_group: gfx_hal::queue::QueueGroup<B>,
  pub command_pool: ManuallyDrop<B::CommandPool>,

  pub memories: gfx_hal::adapter::MemoryProperties,
  pub color_format: gfx_hal::format::Format,
  pub depth_format: gfx_hal::format::Format,

  pub color_image: ManuallyDrop<AttachedImage<B>>,
  pub color_image_view: ManuallyDrop<B::ImageView>,
  pub depth_image: ManuallyDrop<AttachedImage<B>>,
  pub depth_image_view: ManuallyDrop<B::ImageView>,

  pub render_pass: ManuallyDrop<B::RenderPass>,
  pub framebuffer: ManuallyDrop<B::Framebuffer>,
}

pub struct ShapeMesh<B: GfxBackend> {
  vertices: ManuallyDrop<AttachedBuffer<B>>,
  indices: ManuallyDrop<AttachedBuffer<B>>,
  index_count: usize,
}

fn is_compatible_queue_familiy<B: GfxBackend>(qf: &B::QueueFamily) -> bool {
  qf.queue_type().supports_graphics() && qf.max_queues() >= QUEUE_COUNT
}

impl<B: GfxBackend> HeadlessGfxRenderer<B> {
  pub fn new<I: gfx_hal::Instance<Backend=B>>(instance: &I, width: usize, height: usize) -> Result<HeadlessGfxRenderer<B>, &'static str>
  {
    let viewport_extent = Extent { width: width as u32, height: height as u32, depth: 1 };

    let adapter = instance
      .enumerate_adapters()
      .into_iter()
      .find(|a| {
        a.queue_families
          .iter()
          .any(is_compatible_queue_familiy::<B>)
      })
      .ok_or("Failed to find a compatible GPU adapter")?;

    let (device, queue_group): (B::Device, gfx_hal::queue::QueueGroup<B>) = {
      let family: &B::QueueFamily = adapter
        .queue_families
        .iter()
        .find(|qf| is_compatible_queue_familiy::<B>(qf))
        .expect("Failed to find queue family with graphics support");

      let mut gpu: gfx_hal::adapter::Gpu<B> = unsafe {
        adapter
          .physical_device
          .open(&[(family, &[1.0])], gfx_hal::Features::empty())
          .expect("Failed to open GPU")
      };

      (gpu.device, gpu.queue_groups.pop().unwrap())
    };

    let memories = adapter.physical_device.memory_properties();
    let color_format = gfx_hal::format::Format::Rgba8Unorm;
    let depth_format = get_supported_depth_format::<I::Backend>(&adapter.physical_device)
      .ok_or("Failed to find supported depth format")?;

    let command_pool = unsafe {
      device
        .create_command_pool(queue_group.family, gfx_hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
        .map_err(|_| "Failed to create command pool")?
    };

    // Create attachments
    let attachments = unsafe {
      create_images::<B>(&device, viewport_extent, color_format, depth_format, &memories)
    };

    let ((color_image, color_image_view), (depth_image, depth_image_view)) = attachments.unwrap();

    let render_pass = unsafe {
      let color_attachment: gfx_hal::pass::Attachment = gfx_hal::pass::Attachment {
        format: Some(color_format),
        samples: 1,
        ops: gfx_hal::pass::AttachmentOps {
          load: gfx_hal::pass::AttachmentLoadOp::Clear,
          store: gfx_hal::pass::AttachmentStoreOp::Store,
        },
        stencil_ops: gfx_hal::pass::AttachmentOps {
          load: gfx_hal::pass::AttachmentLoadOp::DontCare,
          store: gfx_hal::pass::AttachmentStoreOp::DontCare,
        },
        layouts: std::ops::Range { start: gfx_hal::image::Layout::Undefined, end: gfx_hal::image::Layout::TransferSrcOptimal },
      };
      let depth_attachment: gfx_hal::pass::Attachment = gfx_hal::pass::Attachment {
        format: Some(depth_format),
        samples: 1,
        ops: gfx_hal::pass::AttachmentOps {
          load: gfx_hal::pass::AttachmentLoadOp::Clear,
          store: gfx_hal::pass::AttachmentStoreOp::DontCare,
        },
        stencil_ops: gfx_hal::pass::AttachmentOps {
          load: gfx_hal::pass::AttachmentLoadOp::DontCare,
          store: gfx_hal::pass::AttachmentStoreOp::DontCare,
        },
        layouts: std::ops::Range { start: gfx_hal::image::Layout::Undefined, end: gfx_hal::image::Layout::DepthStencilAttachmentOptimal },
      };
      let attachments = [color_attachment, depth_attachment];

      let color_ref: gfx_hal::pass::AttachmentRef = (0, gfx_hal::image::Layout::ColorAttachmentOptimal);
      let depth_ref: gfx_hal::pass::AttachmentRef = (1, gfx_hal::image::Layout::DepthStencilAttachmentOptimal);

      let subpass_desc: gfx_hal::pass::SubpassDesc = gfx_hal::pass::SubpassDesc {
        colors: &[color_ref],
        depth_stencil: Some(&depth_ref),
        inputs: &[],
        resolves: &[],
        preserves: &[],
      };

      let dependencies = [
        gfx_hal::pass::SubpassDependency {
          passes: std::ops::Range { start: gfx_hal::pass::SubpassRef::External, end: gfx_hal::pass::SubpassRef::Pass(0) },
          stages: std::ops::Range { start: gfx_hal::pso::PipelineStage::BOTTOM_OF_PIPE, end: gfx_hal::pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT },
          accesses: std::ops::Range { start: gfx_hal::image::Access::MEMORY_READ, end: gfx_hal::image::Access::COLOR_ATTACHMENT_READ | gfx_hal::image::Access::COLOR_ATTACHMENT_WRITE },
        },
        gfx_hal::pass::SubpassDependency {
          passes: std::ops::Range { start: gfx_hal::pass::SubpassRef::Pass(0), end: gfx_hal::pass::SubpassRef::External },
          stages: std::ops::Range { start: gfx_hal::pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT, end: gfx_hal::pso::PipelineStage::BOTTOM_OF_PIPE },
          accesses: std::ops::Range { start: gfx_hal::image::Access::COLOR_ATTACHMENT_READ | gfx_hal::image::Access::COLOR_ATTACHMENT_WRITE, end: gfx_hal::image::Access::MEMORY_READ },
        },
      ];

      let render_pass = device
        .create_render_pass(
          &attachments,
          &[subpass_desc],
          &dependencies,
        )
        .expect("Failed to create render pass");

      render_pass
    };

    let framebuffer = unsafe {
      let image_views = vec![&color_image_view, &depth_image_view];

      let framebuffer = device
        .create_framebuffer(
          &render_pass,
          image_views.into_iter(),
          viewport_extent,
        )
        .expect("Failed to create frame buffer");

      framebuffer
    };

    Ok(HeadlessGfxRenderer::<B> {
      viewport_extent,
      stage: None,
      shape_store: ShapeStore::new(),
      shape_meshes: HashMap::new(),
      device,
      queue_group,
      command_pool: ManuallyDrop::new(command_pool),
      memories,
      color_format,
      depth_format,
      color_image: ManuallyDrop::new(color_image),
      color_image_view: ManuallyDrop::new(color_image_view),
      depth_image: ManuallyDrop::new(depth_image),
      depth_image_view: ManuallyDrop::new(depth_image_view),
      render_pass: ManuallyDrop::new(render_pass),
      framebuffer: ManuallyDrop::new(framebuffer),
    })
  }

  pub fn define_shape(&mut self, tag: &swf_tree::tags::DefineShape) -> usize {
    self.shape_store.define_shape(tag)
  }

  pub fn get_image(&mut self) -> Result<Image, &'static str> {
    match self.stage.take() {
      None => Err("Failed to render: self.stage is None"),
      Some(stage) => {
        let display_list = [stage];
        self.render_stage(&display_list);
        let [old_stage] = display_list;
        self.stage = Some(old_stage);
        Ok(self.download_image())
      }
    }
  }

  fn get_shape_mesh(&mut self, shape_id: usize) -> &ShapeMesh<B> {
    match self.shape_store.get(shape_id) {
      Some(GfxSymbol::Shape(symbol)) => {
        let cmd_queue = &mut self.queue_group.queues[0];

        type IndexType = u32;

        let index_count: usize = symbol.mesh.indices.len();
        let vertex_buffer_size = ::std::mem::size_of::<Vertex>() * symbol.mesh.vertices.len();
        let index_buffer_size = ::std::mem::size_of::<IndexType>() * index_count;

        let vertices = {
          let staging_buffer = unsafe {
            create_buffer::<B>(
              &self.device,
              gfx_hal::buffer::Usage::TRANSFER_SRC,
              gfx_hal::memory::Properties::CPU_VISIBLE | gfx_hal::memory::Properties::COHERENT,
              vertex_buffer_size as u64,
              &self.memories,
            ).unwrap()
          };

          unsafe {
            let mapping = self.device.map_memory(&staging_buffer.memory, 0..staging_buffer.capacity)
              .expect("Failed to map staging memory (for mesh upload)");

            std::ptr::copy_nonoverlapping(symbol.mesh.vertices.as_ptr(), mapping as *mut Vertex, symbol.mesh.vertices.len());

            self.device.unmap_memory(&staging_buffer.memory);
          }

          let vertex_buffer = unsafe {
            create_buffer::<B>(
              &self.device,
              gfx_hal::buffer::Usage::VERTEX | gfx_hal::buffer::Usage::TRANSFER_DST,
              gfx_hal::memory::Properties::DEVICE_LOCAL,
              vertex_buffer_size as u64,
              &self.memories,
            ).unwrap()
          };

          unsafe {
            let mut copy_cmd = self.command_pool.allocate_one(gfx_hal::command::Level::Primary);
            copy_cmd.begin_primary(gfx_hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
            copy_cmd.copy_buffer(
              &staging_buffer.buffer,
              &vertex_buffer.buffer,
              &[gfx_hal::command::BufferCopy { src: 0, dst: 0, size: vertex_buffer_size as u64 }],
            );
            copy_cmd.finish();
            let copy_fence = self.device.create_fence(false).expect("Failed to create fence");
            cmd_queue.submit_without_semaphores(Some(&copy_cmd), Some(&copy_fence));
            self.device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
            self.device.destroy_fence(copy_fence);
          }

          unsafe { destroy_buffer(&self.device, staging_buffer); }

          vertex_buffer
        };


        let indices = {
          let staging_buffer = unsafe {
            create_buffer::<B>(
              &self.device,
              gfx_hal::buffer::Usage::TRANSFER_SRC,
              gfx_hal::memory::Properties::CPU_VISIBLE | gfx_hal::memory::Properties::COHERENT,
              index_buffer_size as u64,
              &self.memories,
            ).unwrap()
          };

          unsafe {
            let mapping = self.device.map_memory(&staging_buffer.memory, 0..staging_buffer.capacity)
              .expect("Failed to map staging memory (for indices upload)");

            std::ptr::copy_nonoverlapping(symbol.mesh.indices.as_ptr(), mapping as *mut u32, symbol.mesh.indices.len());

            self.device.unmap_memory(&staging_buffer.memory);
          }

          let index_buffer = unsafe {
            create_buffer::<B>(
              &self.device,
              gfx_hal::buffer::Usage::INDEX | gfx_hal::buffer::Usage::TRANSFER_DST,
              gfx_hal::memory::Properties::DEVICE_LOCAL,
              index_buffer_size as u64,
              &self.memories,
            ).unwrap()
          };

          unsafe {
            let mut copy_cmd = self.command_pool.allocate_one(gfx_hal::command::Level::Primary);
            copy_cmd.begin_primary(gfx_hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
            copy_cmd.copy_buffer(
              &staging_buffer.buffer,
              &index_buffer.buffer,
              &[gfx_hal::command::BufferCopy { src: 0, dst: 0, size: index_buffer_size as u64 }],
            );
            copy_cmd.finish();
            let copy_fence = self.device.create_fence(false).expect("Failed to create fence");
            cmd_queue.submit_without_semaphores(Some(&copy_cmd), Some(&copy_fence));
            self.device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
            self.device.destroy_fence(copy_fence);
          }

          unsafe { destroy_buffer(&self.device, staging_buffer); }

          index_buffer
        };

        let shape_mesh = ShapeMesh {
          vertices: ManuallyDrop::new(vertices),
          indices: ManuallyDrop::new(indices),
          index_count,
        };
        self.shape_meshes.entry(shape_id).or_insert(shape_mesh)
      }
      _ => panic!("ShapeNotFound"),
    }
  }

  fn render_stage(&mut self, display_list: &[DisplayItem]) -> () {
    let (shape_id, matrix) = match display_list[0] {
      DisplayItem::Shape(ref id, ref matrix) => (*id, matrix),
    };

    let (vertex_shader_module, fragment_shader_module, descriptor_set_layout, pipeline_layout, pipeline_cache, pipeline) = unsafe {
      let descriptor_set_layout = self.device
        .create_descriptor_set_layout(&[], &[])
        .expect("Failed to create descriptor set layout");

      let constant_size: usize = ::std::mem::size_of::<glm::TMat4<f32>>();
      let push_constants: Vec<(gfx_hal::pso::ShaderStageFlags, core::ops::Range<u32>)> = vec![
        (gfx_hal::pso::ShaderStageFlags::VERTEX, 0..((constant_size / ::std::mem::size_of::<f32>()) as u32)),
      ];

      let pipeline_layout = self.device
        .create_pipeline_layout(
          &[],
          push_constants,
        )
        .expect("Failed to create pipeline layout");

      let pipeline_cache = self.device
        .create_pipeline_cache(Option::None)
        .expect("Failed to create pipeline cache");


      let mut shader_compiler: shaderc::Compiler = shaderc::Compiler::new().expect("Failed to create shader");
      let vertex_compile_artifact: shaderc::CompilationArtifact = shader_compiler
        .compile_into_spirv(
          VERTEX_SHADER_SOURCE,
          shaderc::ShaderKind::Vertex,
          "shader.vert",
          "main",
          None,
        )
        .expect("Failed to compile vertex shader");
      let fragment_compile_artifact: shaderc::CompilationArtifact = shader_compiler
        .compile_into_spirv(
          FRAGMENT_SHADER_SOURCE,
          shaderc::ShaderKind::Fragment,
          "shader.frag",
          "main",
          None,
        )
        .expect("Failed to compile fragment shader");
      let vertex_shader_module = {
        self.device
          .create_shader_module(vertex_compile_artifact.as_binary())
          .expect("Failed to create shader module")
      };
      let fragment_shader_module = {
        self.device
          .create_shader_module(fragment_compile_artifact.as_binary())
          .expect("Failed to create fragment module")
      };

      let shaders = gfx_hal::pso::GraphicsShaderSet {
        vertex: gfx_hal::pso::EntryPoint {
          entry: "main",
          module: &vertex_shader_module,
          specialization: gfx_hal::pso::Specialization { constants: Cow::Owned(Vec::new()), data: Cow::Owned(Vec::new()) },
        },
        hull: None,
        domain: None,
        geometry: None,
        fragment: Some(gfx_hal::pso::EntryPoint {
          entry: "main",
          module: &fragment_shader_module,
          specialization: gfx_hal::pso::Specialization { constants: Cow::Owned(Vec::new()), data: Cow::Owned(Vec::new()) },
        }),
      };

      let rasterizer = gfx_hal::pso::Rasterizer {
        depth_clamping: false,
        polygon_mode: gfx_hal::pso::PolygonMode::Fill,
        cull_face: gfx_hal::pso::Face::NONE,
        front_face: gfx_hal::pso::FrontFace::Clockwise,
        depth_bias: None,
        conservative: false,
      };

      let vertex_buffers: Vec<gfx_hal::pso::VertexBufferDesc> = vec![gfx_hal::pso::VertexBufferDesc {
        binding: 0,
        stride: (::std::mem::size_of::<Vertex>()) as u32,
        rate: ::gfx_hal::pso::VertexInputRate::Vertex,
      }];
      let attributes: Vec<gfx_hal::pso::AttributeDesc> = vec![
        // position
        gfx_hal::pso::AttributeDesc {
          binding: 0,
          location: 0,
          element: gfx_hal::pso::Element { format: gfx_hal::format::Format::Rgb32Sfloat, offset: offset_of!(Vertex, position) as u32 },
        },
        // color
        gfx_hal::pso::AttributeDesc {
          binding: 0,
          location: 1,
          element: gfx_hal::pso::Element { format: gfx_hal::format::Format::Rgb32Sfloat, offset: offset_of!(Vertex, color) as u32 },
        },
      ];

      let input_assembler: gfx_hal::pso::InputAssemblerDesc = gfx_hal::pso::InputAssemblerDesc::new(gfx_hal::Primitive::TriangleList);

      let blender = {
        let blend_state: Option<gfx_hal::pso::BlendState> = Some(gfx_hal::pso::BlendState {
          color: gfx_hal::pso::BlendOp::Add {
            src: gfx_hal::pso::Factor::One,
            dst: gfx_hal::pso::Factor::Zero,
          },
          alpha: gfx_hal::pso::BlendOp::Add {
            src: gfx_hal::pso::Factor::One,
            dst: gfx_hal::pso::Factor::Zero,
          },
        });
        gfx_hal::pso::BlendDesc {
          logic_op: Some(gfx_hal::pso::LogicOp::Copy),
          targets: vec![gfx_hal::pso::ColorBlendDesc { mask: gfx_hal::pso::ColorMask::ALL, blend: blend_state }],
        }
      };

      let depth_stencil = gfx_hal::pso::DepthStencilDesc {
        depth: Some(gfx_hal::pso::DepthTest { fun: gfx_hal::pso::Comparison::LessEqual, write: true }),
        depth_bounds: false,
        stencil: None,
      };

      let multisampling: Option<gfx_hal::pso::Multisampling> = None;

      let baked_states = gfx_hal::pso::BakedStates {
        viewport: Some(gfx_hal::pso::Viewport {
          rect: self.viewport_extent.rect(),
          depth: (0.0..1.0),
        }),
        scissor: Some(self.viewport_extent.rect()),
        blend_color: None,
        depth_bounds: None,
      };

      let pipeline_flags: gfx_hal::pso::PipelineCreationFlags = gfx_hal::pso::PipelineCreationFlags::empty();

      let pipeline_desc = gfx_hal::pso::GraphicsPipelineDesc {
        shaders,
        rasterizer,
        vertex_buffers,
        attributes,
        input_assembler,
        blender,
        depth_stencil,
        multisampling,
        baked_states,
        layout: &pipeline_layout,
        subpass: gfx_hal::pass::Subpass {
          index: 0,
          main_pass: &*self.render_pass,
        },
        flags: pipeline_flags,
        parent: gfx_hal::pso::BasePipeline::None,
      };

      let pipeline = self.device
        .create_graphics_pipeline(&pipeline_desc, Some(&pipeline_cache))
        .expect("Failed to create pipeline");

      (vertex_shader_module, fragment_shader_module, descriptor_set_layout, pipeline_layout, pipeline_cache, pipeline)
    };

    unsafe {
      let mut command_buffer: B::CommandBuffer = self.command_pool.allocate_one(gfx_hal::command::Level::Primary);
      command_buffer.begin_primary(gfx_hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);

      {
        let clear_values = [
          gfx_hal::command::ClearValue { color: gfx_hal::command::ClearColor { float32: [0.0, 0.0, 0.0, 0.0] } },
          gfx_hal::command::ClearValue { depth_stencil: gfx_hal::command::ClearDepthStencil { depth: 1.0, stencil: 0 } },
        ];

        // Start of render pass
        command_buffer.begin_render_pass(
          &self.render_pass,
          &self.framebuffer,
          self.viewport_extent.rect(),
          clear_values.iter(),
          gfx_hal::command::SubpassContents::Inline,
        );

        let viewports = vec![gfx_hal::pso::Viewport { rect: self.viewport_extent.rect(), depth: (0.0..1.0) }];
        command_buffer.set_viewports(0, viewports);

        let scissors = vec![self.viewport_extent.rect()];
        command_buffer.set_scissors(0, scissors);

        command_buffer.bind_graphics_pipeline(&pipeline);

        let index_count: usize = {
          let mesh = self.get_shape_mesh(shape_id);

          command_buffer.bind_vertex_buffers(0, vec![(&mesh.vertices.buffer, 0)]);
          command_buffer.bind_index_buffer(gfx_hal::buffer::IndexBufferView {
            buffer: &mesh.indices.buffer,
            offset: 0,
            index_type: gfx_hal::IndexType::U32,
          });

          mesh.index_count
        };

//        let pos = vec![
//          glm::vec3(0.0f32, 0.0f32, 0.0f32),
//        ];

//        for v in pos {
        let eye_matrix = glm::ortho(
          0f32,
          (self.viewport_extent.width * 20) as f32,
          0f32,
          (self.viewport_extent.height * 20) as f32,
          -10f32,
          10f32,
        );

        let world_matrix = glm::make_mat4x4(
          &[
            f64::from(matrix.scale_x) as f32, f64::from(matrix.rotate_skew0) as f32, 0.0, 0.0,
            f64::from(matrix.rotate_skew1) as f32, f64::from(matrix.scale_y) as f32, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            matrix.translate_x as f32, matrix.translate_y as f32, 0.0, 1.0,
          ]
        );

        let mvp_matrix_bits: Vec<u32> = (eye_matrix * world_matrix).data.iter().map(|x| x.to_bits()).collect();

        command_buffer.push_graphics_constants(
          &pipeline_layout,
          gfx_hal::pso::ShaderStageFlags::VERTEX,
          0,
          &mvp_matrix_bits[..],
        );

        command_buffer.draw_indexed(0..(index_count as u32), 0, 0..1);
        // End of render pass
//        }
      }

      command_buffer.finish();

      let cmd_queue = &mut self.queue_group.queues[0];
      let cmd_fence = self.device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_without_semaphores(Some(&command_buffer), Some(&cmd_fence));
      self.device.wait_for_fence(&cmd_fence, core::u64::MAX).expect("Failed to wait for fence");
      self.device.destroy_fence(cmd_fence);

      self.device
        .wait_idle()
        .expect("Failed to wait for device to be idle");
    }

    unsafe {
      self.device.destroy_graphics_pipeline(pipeline);
      self.device.destroy_pipeline_cache(pipeline_cache);
      self.device.destroy_pipeline_layout(pipeline_layout);
      self.device.destroy_descriptor_set_layout(descriptor_set_layout);
      self.device.destroy_shader_module(fragment_shader_module);
      self.device.destroy_shader_module(vertex_shader_module);
    }
  }

  fn download_image(&mut self) -> Image {
    let cmd_queue = &mut self.queue_group.queues[0];

    let gfx_image = unsafe {
      create_image::<B>(
        &self.device,
        gfx_hal::image::Kind::D2(self.viewport_extent.width, self.viewport_extent.height, 1, 1),
        1,
        self.color_format,
        gfx_hal::image::Tiling::Linear,
        gfx_hal::image::Usage::TRANSFER_DST,
        gfx_hal::image::ViewCapabilities::empty(),
        gfx_hal::memory::Properties::CPU_VISIBLE | gfx_hal::memory::Properties::COHERENT,
        &self.memories,
      ).unwrap()
    };

    let image = unsafe {
      {
        let mut copy_cmd = self.command_pool.allocate_one(gfx_hal::command::Level::Primary);
        copy_cmd.begin_primary(gfx_hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);

        {
          let src_state: gfx_hal::image::State = (gfx_hal::image::Access::empty(), gfx_hal::image::Layout::Undefined);
          let dst_state: gfx_hal::image::State = (gfx_hal::image::Access::TRANSFER_WRITE, gfx_hal::image::Layout::TransferDstOptimal);
          let barrier: gfx_hal::memory::Barrier<B> = gfx_hal::memory::Barrier::Image {
            states: (src_state..dst_state),
            target: &gfx_image.image,
            families: None,
            range: gfx_hal::image::SubresourceRange {
              aspects: gfx_hal::format::Aspects::COLOR,
              layers: 0..1,
              levels: 0..1,
            },
          };
          copy_cmd.pipeline_barrier(
            gfx_hal::pso::PipelineStage::TRANSFER..gfx_hal::pso::PipelineStage::TRANSFER,
            gfx_hal::memory::Dependencies::empty(),
            Some(barrier),
          );
        }

        {
          let image_copy_regions: gfx_hal::command::ImageCopy = gfx_hal::command::ImageCopy {
            src_subresource: gfx_hal::image::SubresourceLayers {
              aspects: gfx_hal::format::Aspects::COLOR,
              level: 0,
              layers: 0..1,
            },
            src_offset: gfx_hal::image::Offset { x: 0, y: 0, z: 0 },
            dst_subresource: gfx_hal::image::SubresourceLayers {
              aspects: gfx_hal::format::Aspects::COLOR,
              level: 0,
              layers: 0..1,
            },
            dst_offset: gfx_hal::image::Offset { x: 0, y: 0, z: 0 },
            extent: self.viewport_extent,
          };
          copy_cmd.copy_image(
            &self.color_image.image,
            gfx_hal::image::Layout::TransferSrcOptimal,
            &gfx_image.image,
            gfx_hal::image::Layout::TransferDstOptimal,
            Some(&image_copy_regions),
          );
        }

        {
          let src_state: gfx_hal::image::State = (gfx_hal::image::Access::TRANSFER_WRITE, gfx_hal::image::Layout::TransferDstOptimal);
          let dst_state: gfx_hal::image::State = (gfx_hal::image::Access::MEMORY_READ, gfx_hal::image::Layout::General);
          let barrier: gfx_hal::memory::Barrier<B> = gfx_hal::memory::Barrier::Image {
            states: (src_state..dst_state),
            target: &gfx_image.image,
            families: None,
            range: gfx_hal::image::SubresourceRange {
              aspects: gfx_hal::format::Aspects::COLOR,
              layers: 0..1,
              levels: 0..1,
            },
          };
          copy_cmd.pipeline_barrier(
            gfx_hal::pso::PipelineStage::TRANSFER..gfx_hal::pso::PipelineStage::TRANSFER,
            gfx_hal::memory::Dependencies::empty(),
            Some(barrier),
          );
        }

        copy_cmd.finish();

        let copy_fence = self.device.create_fence(false).expect("Failed to create fence");
        cmd_queue.submit_without_semaphores(Some(&copy_cmd), Some(&copy_fence));
        self.device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
        self.device.destroy_fence(copy_fence);
      }

      let image_footprint = self.device.get_image_subresource_footprint(
        &gfx_image.image,
        gfx_hal::image::Subresource {
          aspects: gfx_hal::format::Aspects::COLOR,
          level: 0,
          layer: 0,
        },
      );

      let meta = ImageMetadata {
        width: self.viewport_extent.width as usize,
        height: self.viewport_extent.height as usize,
        stride: image_footprint.row_pitch as usize,
      };

      let data = {
        let count = ((image_footprint.slice.end - image_footprint.slice.start) as usize) / std::mem::size_of::<u8>();
        let mapping = self.device.map_memory(&gfx_image.memory, image_footprint.slice)
          .expect("Failed to map image memory (for read)");
        let data = std::slice::from_raw_parts::<u8>(mapping as *const u8, count);

        let data: Vec<u8> = Vec::from(data);

        self.device.unmap_memory(&gfx_image.memory);

        data
      };

      Image { meta, data }
    };

    unsafe { destroy_image(&self.device, gfx_image); }

    image
  }
}

impl<B: GfxBackend> Drop for HeadlessGfxRenderer<B> {
  fn drop(&mut self) -> () {
    unsafe {
      use core::ptr::read;

      self.device
        .wait_idle()
        .expect("Failed to wait for device to be idle");

      for (_, mesh) in self.shape_meshes.drain() {
        destroy_buffer(&self.device, ManuallyDrop::into_inner(mesh.indices));
        destroy_buffer(&self.device, ManuallyDrop::into_inner(mesh.vertices));
      }

      self.device.destroy_framebuffer(ManuallyDrop::into_inner(read(&self.framebuffer)));
      self.device.destroy_render_pass(ManuallyDrop::into_inner(read(&self.render_pass)));

      self.device.destroy_image_view(ManuallyDrop::into_inner(read(&self.depth_image_view)));
      destroy_image(&self.device, ManuallyDrop::into_inner(read(&self.depth_image)));
      self.device.destroy_image_view(ManuallyDrop::into_inner(read(&self.color_image_view)));
      destroy_image(&self.device, ManuallyDrop::into_inner(read(&self.color_image)));

      self
        .device
        .destroy_command_pool(ManuallyDrop::take(&mut self.command_pool));
    }
  }
}

impl<B: GfxBackend> Renderer for HeadlessGfxRenderer<B> {
  // TODO: Pass a list instead of a single item
  fn set_stage(&mut self, display_list: DisplayItem) -> () {
    self.stage = Some(display_list);
  }


//  let mut tessellator = FillTessellator::new();
//
//  let mut mesh: VertexBuffers<GpuFillVertex, u16> = VertexBuffers::new();
//
//  tessellator.tessellate_path(
//  &path,
//  &FillOptions::tolerance(0.01),
//  &mut BuffersBuilder::new(&mut mesh, VertexCtor),
//  ).unwrap();
}
