use std::mem::ManuallyDrop;

use gfx_hal::adapter::PhysicalDevice;
use gfx_hal::Backend as GfxBackend;
use gfx_hal::device::Device;
use gfx_hal::image::Extent;
use gfx_hal::queue::family::QueueFamily;
use lyon::tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use nalgebra_glm as glm;
use swf_tree::FillStyle;
use swf_tree::Shape as SwfShape;

use crate::decoder::shape_decoder::decode_shape;
use crate::gfx::{AttachedImage, create_buffer, create_image, create_images, destroy_buffer, destroy_image, get_supported_depth_format, Vertex};
use crate::renderer::{Image, ImageMetadata, Renderer, DisplayItem};

const QUEUE_COUNT: usize = 1;
const VERTEX_SHADER_SOURCE: &'static str = include_str!("shader.vert.glsl");
const FRAGMENT_SHADER_SOURCE: &'static str = include_str!("shader.frag.glsl");

pub struct HeadlessGfxRenderer<B: GfxBackend> {
  pub viewport_extent: Extent,
  pub stage: Option<DisplayItem>,

  pub device: B::Device,
  pub queue_group: gfx_hal::queue::QueueGroup<B, gfx_hal::queue::capability::Graphics>,
  pub command_pool: ManuallyDrop<gfx_hal::pool::CommandPool<B, gfx_hal::queue::capability::Graphics>>,

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

impl<B: GfxBackend> HeadlessGfxRenderer<B> {
  pub fn new<I: gfx_hal::Instance>(instance: &I, width: usize, height: usize) -> Result<HeadlessGfxRenderer<I::Backend>, &'static str>
  {
    let viewport_extent = Extent { width: width as u32, height: height as u32, depth: 1 };

    let adapter = instance
      .enumerate_adapters()
      .into_iter()
      .find(|a| {
        a.queue_families
          .iter()
          .any(|qf| qf.supports_graphics())
      })
      .ok_or("Failed to find a compatible GPU adapter")?;

    let (device, queue_group): (<I::Backend as GfxBackend>::Device, _) = adapter
      .open_with::<_, gfx_hal::queue::capability::Graphics>(QUEUE_COUNT, |_qf| true)
      .map_err(|_| "Failed to open GPU device")?;


    let memories = adapter.physical_device.memory_properties();
    let color_format = gfx_hal::format::Format::Rgba8Unorm;
    let depth_format = get_supported_depth_format::<I::Backend>(&adapter.physical_device)
      .ok_or("Failed to find supported depth format")?;

    let command_pool = unsafe {
      device
        .create_command_pool_typed(&queue_group, gfx_hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
        .map_err(|_| "Failed to create command pool")?
    };

    // Create attachments
    let attachments = unsafe {
      create_images::<I::Backend>(&device, viewport_extent, color_format, depth_format, &memories)
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

    Ok(HeadlessGfxRenderer::<I::Backend> {
      viewport_extent,
      stage: None,
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

  pub fn get_image(&mut self) -> Result<Image, &'static str> {
    match self.stage.take() {
      None => Err("Failed to render: self.stage is None"),
      Some(stage) => {
        self.render_stage(&stage);
        self.stage = Some(stage);
        Ok(self.download_image())
      }
    }
  }

  fn render_stage(&mut self, stage: &DisplayItem) -> () {
    let (shape, matrix) = match stage {
      DisplayItem::Shape(ref shape, ref matrix) => (shape, matrix),
    };

    type IndexType = u32;

    let cmd_queue = &mut self.queue_group.queues[0];

    let decoded = decode_shape(shape);
    let mut geometry: VertexBuffers<Vertex, IndexType> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();

    {
      let path = &decoded.paths[0];

      let color: [f32; 3] = if let Some(ref fill) = &path.fill {
        match fill {
          FillStyle::Solid(ref style) => [
            (style.color.r as f32) / 255f32,
            (style.color.g as f32) / 255f32,
            (style.color.b as f32) / 255f32,
          ],
          _ => [0.0, 1.0, 0.0],
        }
      } else {
        [1.0, 0.0, 0.0]
      };

      // Compute the tessellation.
      tessellator.tessellate_path(
        &path.path,
        &FillOptions::default(),
        &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
          Vertex {
            position: [vertex.position.x, vertex.position.y, 0.0],
            color,
          }
        }),
      ).unwrap();
    }

    let vertex_buffer_size = ::std::mem::size_of::<Vertex>() * geometry.vertices.len();
    let index_buffer_size = ::std::mem::size_of::<IndexType>() * geometry.indices.len();

    let vertex_buffer = {
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
        let mut staging_mapping: gfx_hal::mapping::Writer<B, Vertex> = self.device
          .acquire_mapping_writer(&staging_buffer.memory, 0..staging_buffer.capacity)
          .expect("Failed to acquire mapping writer");
        staging_mapping[..geometry.vertices.len()].copy_from_slice(&geometry.vertices);
        self.device
          .release_mapping_writer(staging_mapping)
          .expect("Failed to release mapping writer");
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
        let mut copy_cmd = self.command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
        copy_cmd.begin();
        copy_cmd.copy_buffer(
          &staging_buffer.buffer,
          &vertex_buffer.buffer,
          &[gfx_hal::command::BufferCopy { src: 0, dst: 0, size: vertex_buffer_size as u64 }],
        );
        copy_cmd.finish();
        let copy_fence = self.device.create_fence(false).expect("Failed to create fence");
        cmd_queue.submit_nosemaphores(Some(&copy_cmd), Some(&copy_fence));
        self.device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
        self.device.destroy_fence(copy_fence);
      }

      unsafe { destroy_buffer(&self.device, staging_buffer); }

      vertex_buffer
    };

    let index_buffer = {
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
        let mut staging_mapping: gfx_hal::mapping::Writer<B, IndexType> = self.device
          .acquire_mapping_writer(&staging_buffer.memory, 0..staging_buffer.capacity)
          .expect("Failed to acquire mapping writer");
        staging_mapping[..geometry.indices.len()].copy_from_slice(&geometry.indices);
        self.device
          .release_mapping_writer(staging_mapping)
          .expect("Failed to release mapping writer");
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
        let mut copy_cmd = self.command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
        copy_cmd.begin();
        copy_cmd.copy_buffer(
          &staging_buffer.buffer,
          &index_buffer.buffer,
          &[gfx_hal::command::BufferCopy { src: 0, dst: 0, size: index_buffer_size as u64 }],
        );
        copy_cmd.finish();
        let copy_fence = self.device.create_fence(false).expect("Failed to create fence");
        cmd_queue.submit_nosemaphores(Some(&copy_cmd), Some(&copy_fence));
        self.device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
        self.device.destroy_fence(copy_fence);
      }

      unsafe { destroy_buffer(&self.device, staging_buffer); }

      index_buffer
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
        .create_pipeline_cache()
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
          .create_shader_module(vertex_compile_artifact.as_binary_u8())
          .expect("Failed to create shader module")
      };
      let fragment_shader_module = {
        self.device
          .create_shader_module(fragment_compile_artifact.as_binary_u8())
          .expect("Failed to create fragment module")
      };

      let shaders = gfx_hal::pso::GraphicsShaderSet {
        vertex: gfx_hal::pso::EntryPoint {
          entry: "main",
          module: &vertex_shader_module,
          specialization: gfx_hal::pso::Specialization { constants: &[], data: &[] },
        },
        hull: None,
        domain: None,
        geometry: None,
        fragment: Some(gfx_hal::pso::EntryPoint {
          entry: "main",
          module: &fragment_shader_module,
          specialization: gfx_hal::pso::Specialization { constants: &[], data: &[] },
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
        rate: 0,
      }];
      let attributes: Vec<gfx_hal::pso::AttributeDesc> = vec![
        // position
        gfx_hal::pso::AttributeDesc {
          binding: 0,
          location: 0,
          element: gfx_hal::pso::Element { format: gfx_hal::format::Format::Rgb32Float, offset: offset_of!(Vertex, position) as u32 },
        },
        // color
        gfx_hal::pso::AttributeDesc {
          binding: 0,
          location: 1,
          element: gfx_hal::pso::Element { format: gfx_hal::format::Format::Rgb32Float, offset: offset_of!(Vertex, color) as u32 },
        },
      ];

      let input_assembler: gfx_hal::pso::InputAssemblerDesc = gfx_hal::pso::InputAssemblerDesc::new(gfx_hal::Primitive::TriangleList);

      let blender = {
        let blend_state = gfx_hal::pso::BlendState::On {
          color: gfx_hal::pso::BlendOp::Add {
            src: gfx_hal::pso::Factor::One,
            dst: gfx_hal::pso::Factor::Zero,
          },
          alpha: gfx_hal::pso::BlendOp::Add {
            src: gfx_hal::pso::Factor::One,
            dst: gfx_hal::pso::Factor::Zero,
          },
        };
        gfx_hal::pso::BlendDesc {
          logic_op: Some(gfx_hal::pso::LogicOp::Copy),
          targets: vec![gfx_hal::pso::ColorBlendDesc(gfx_hal::pso::ColorMask::ALL, blend_state)],
        }
      };

      let depth_stencil = gfx_hal::pso::DepthStencilDesc {
        depth: gfx_hal::pso::DepthTest::On { fun: gfx_hal::pso::Comparison::LessEqual, write: true },
        depth_bounds: false,
        stencil: gfx_hal::pso::StencilTest::Off,
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
      let mut command_buffer = self.command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();

      command_buffer.begin();

      {
        let clear_values = [
          gfx_hal::command::ClearValue::Color(gfx_hal::command::ClearColor::Float([0.0, 0.0, 0.0, 0.0])),
          gfx_hal::command::ClearValue::DepthStencil(gfx_hal::command::ClearDepthStencil(1.0, 0)),
        ];
        let mut encoder: gfx_hal::command::RenderPassInlineEncoder<_> = command_buffer.begin_render_pass_inline(
          &self.render_pass,
          &self.framebuffer,
          self.viewport_extent.rect(),
          clear_values.iter(),
        );

        let viewports = vec![gfx_hal::pso::Viewport { rect: self.viewport_extent.rect(), depth: (0.0..1.0) }];
        encoder.set_viewports(0, viewports);

        let scissors = vec![self.viewport_extent.rect()];
        encoder.set_scissors(0, scissors);

        encoder.bind_graphics_pipeline(&pipeline);

        encoder.bind_vertex_buffers(0, vec![(&vertex_buffer.buffer, 0)]);
        encoder.bind_index_buffer(gfx_hal::buffer::IndexBufferView {
          buffer: &index_buffer.buffer,
          offset: 0,
          index_type: gfx_hal::IndexType::U32,
        });

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

          encoder.push_graphics_constants(
            &pipeline_layout,
            gfx_hal::pso::ShaderStageFlags::VERTEX,
            0,
            &mvp_matrix_bits[..],
          );

          encoder.draw_indexed(0..(geometry.indices.len() as u32), 0, 0..1);
//        }
      }

      command_buffer.finish();

      let cmd_fence = self.device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_nosemaphores(Some(&command_buffer), Some(&cmd_fence));
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
      destroy_buffer(&self.device, index_buffer);
      destroy_buffer(&self.device, vertex_buffer);
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
        let mut copy_cmd = self.command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
        copy_cmd.begin();

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
        cmd_queue.submit_nosemaphores(Some(&copy_cmd), Some(&copy_fence));
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
        let mapping: gfx_hal::mapping::Reader<B, u8> = self.device
          .acquire_mapping_reader(&gfx_image.memory, image_footprint.slice)
          .expect("Failed to acquire mapping reader");

        let data: Vec<u8> = Vec::from(&*mapping);

        self.device
          .release_mapping_reader(mapping);

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

      self.device.destroy_framebuffer(ManuallyDrop::into_inner(read(&self.framebuffer)));
      self.device.destroy_render_pass(ManuallyDrop::into_inner(read(&self.render_pass)));

      self.device.destroy_image_view(ManuallyDrop::into_inner(read(&self.depth_image_view)));
      destroy_image(&self.device, ManuallyDrop::into_inner(read(&self.depth_image)));
      self.device.destroy_image_view(ManuallyDrop::into_inner(read(&self.color_image_view)));
      destroy_image(&self.device, ManuallyDrop::into_inner(read(&self.color_image)));

      self
        .device
        .destroy_command_pool(ManuallyDrop::into_inner(read(&self.command_pool)).into_raw());
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
