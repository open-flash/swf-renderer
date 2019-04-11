use nalgebra_glm as glm;

const VERTEX_SHADER_SOURCE: &'static str = include_str!("shader.vert.glsl");
const FRAGMENT_SHADER_SOURCE: &'static str = include_str!("shader.frag.glsl");

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
  pub position: [f32; 3],
  pub color: [f32; 3],
}

/// Returns the offset of the field `field` in the struct `ty`
macro_rules! offset_of {
  ($ty:ty, $field:ident) => {
    {
      // TODO: Replace `let` with `const`
      #[allow(unused_unsafe)]
      let offset: usize = unsafe { &(*(0 as *const $ty)).$field as *const _ as usize };
      offset
    }
  }
}

pub struct AttachedBuffer<B: gfx_hal::Backend> {
  /// Buffer attached to memory
  pub buffer: B::Buffer,

  /// Memory for the buffer
  pub memory: B::Memory,

  /// Capacity of the memory
  pub capacity: u64,
}

pub unsafe fn create_buffer<B: gfx_hal::Backend>(
  device: &B::Device,
  usage: gfx_hal::buffer::Usage,
  memory_properties: gfx_hal::memory::Properties,
  size: u64,
  memories: &gfx_hal::adapter::MemoryProperties,
) -> Result<AttachedBuffer<B>, &'static str> {
  use gfx_hal::device::Device;

  let mut buffer = device
    .create_buffer(size, usage)
    .map_err(|_| "Failed to create buffer")?;

  let requirements: gfx_hal::memory::Requirements = device
    .get_buffer_requirements(&buffer);

  let mem_type: gfx_hal::MemoryTypeId = get_memory_type_id(&memories.memory_types, memory_properties, requirements.type_mask);

  match device.allocate_memory(mem_type, requirements.size) {
    Err(_) => {
      device.destroy_buffer(buffer);
      Err("Failed to allocate buffer memory")
    }
    Ok(memory) => {
      match device.bind_buffer_memory(&memory, 0, &mut buffer) {
        Err(_) => {
          device.free_memory(memory);
          device.destroy_buffer(buffer);
          Err("Failed to bind buffer to memory")
        }
        Ok(_) => Ok(AttachedBuffer { buffer, memory, capacity: requirements.size }),
      }
    }
  }
}

pub unsafe fn destroy_buffer<B: gfx_hal::Backend>(device: &B::Device, buffer: AttachedBuffer<B>) -> () {
  use gfx_hal::device::Device;

  device.free_memory(buffer.memory);
  device.destroy_buffer(buffer.buffer);
}

pub struct AttachedImage<B: gfx_hal::Backend> {
  /// Image attached to memory
  pub image: B::Image,

  /// Image for the buffer
  pub memory: B::Memory,
}

pub unsafe fn create_image<B: gfx_hal::Backend>(
  device: &B::Device,
  kind: ::gfx_hal::image::Kind,
  mip_levels: ::gfx_hal::image::Level,
  format: ::gfx_hal::format::Format,
  tiling: ::gfx_hal::image::Tiling,
  usage: ::gfx_hal::image::Usage,
  view_caps: ::gfx_hal::image::ViewCapabilities,
  memory_properties: gfx_hal::memory::Properties,
  memories: &gfx_hal::adapter::MemoryProperties,
) -> Result<AttachedImage<B>, &'static str> {
  use gfx_hal::device::Device;

  let mut image = device
    .create_image(
      kind,
      mip_levels,
      format,
      tiling,
      usage,
      view_caps,
    )
    .map_err(|_| "Failed to create image")?;

  let image_requirements = device.get_image_requirements(&image);
  let image_memory_type_id = get_memory_type_id(
    &memories.memory_types,
    memory_properties,
    image_requirements.type_mask,
  );

  match device.allocate_memory(image_memory_type_id, image_requirements.size) {
    Err(_) => {
      device.destroy_image(image);
      Err("Failed to allocate image memory")
    }
    Ok(memory) => {
      match device.bind_image_memory(&memory, 0, &mut image) {
        Err(_) => {
          device.free_memory(memory);
          device.destroy_image(image);
          Err("Failed to bind image to memory")
        }
        Ok(_) => Ok(AttachedImage { image, memory }),
      }
    }
  }
}

pub unsafe fn destroy_image<B: gfx_hal::Backend>(device: &B::Device, image: AttachedImage<B>) -> () {
  use gfx_hal::device::Device;

  device.free_memory(image.memory);
  device.destroy_image(image.image);
}

pub fn get_supported_depth_format<B: gfx_hal::Backend>(physical_device: &B::PhysicalDevice) -> Option<gfx_hal::format::Format> {
  use gfx_hal::adapter::PhysicalDevice;

  let depth_formats = [
    gfx_hal::format::Format::D32FloatS8Uint,
    gfx_hal::format::Format::D32Float,
    gfx_hal::format::Format::D24UnormS8Uint,
    gfx_hal::format::Format::D16UnormS8Uint,
    gfx_hal::format::Format::D16Unorm,
  ];

  for format in depth_formats.into_iter() {
    let format_properties = physical_device.format_properties(Some(*format));
    if format_properties.optimal_tiling.contains(gfx_hal::format::ImageFeature::DEPTH_STENCIL_ATTACHMENT) {
      return Some(*format);
    }
  }

  Option::None
}

pub fn get_memory_type_id(
  memory_types: &[gfx_hal::MemoryType],
  memory_properties: gfx_hal::memory::Properties,
  mem_type_mask: u64,
) -> gfx_hal::MemoryTypeId {
  memory_types
    .into_iter()
    .enumerate()
    .position(|(id, memory_type)| {
      // Typemask is a bitset where the bit `2^id` indicates compatibility with the memory type with
      // the corresponding `id`.
      (mem_type_mask & (1 << id) != 0) & &memory_type.properties.contains(memory_properties)
    })
    .expect("Failed to find compatible memory type")
    .into()
}

/// Creates the images backing the framebuffer
pub unsafe fn create_images<B: gfx_hal::Backend>(
  device: &B::Device,
  extent: gfx_hal::image::Extent,
  color_format: gfx_hal::format::Format,
  depth_format: gfx_hal::format::Format,
  memories: &gfx_hal::adapter::MemoryProperties,
) -> Result<((AttachedImage<B>, B::ImageView), (AttachedImage<B>, B::ImageView)), &'static str> {
  use gfx_hal::device::Device;

  let color_image = create_image::<B>(
    &device,
    gfx_hal::image::Kind::D2(extent.width, extent.height, 1, 1),
    1,
    color_format,
    gfx_hal::image::Tiling::Optimal,
    gfx_hal::image::Usage::COLOR_ATTACHMENT | gfx_hal::image::Usage::TRANSFER_SRC,
    gfx_hal::image::ViewCapabilities::empty(),
    gfx_hal::memory::Properties::DEVICE_LOCAL,
    memories,
  ).map_err(|_| "Failed to create color image")?;

  let color_image_view = device
    .create_image_view(
      &color_image.image,
      gfx_hal::image::ViewKind::D2,
      color_format,
      gfx_hal::format::Swizzle::NO,
      gfx_hal::image::SubresourceRange {
        aspects: gfx_hal::format::Aspects::COLOR,
        layers: std::ops::Range { start: 0, end: 1 },
        levels: std::ops::Range { start: 0, end: 1 },
      },
    );

  match color_image_view {
    Err(_) => {
      destroy_image(device, color_image);
      Err("Failed to create color image view")
    }
    Ok(color_image_view) => {
      let depth_image = create_image::<B>(
        &device,
        gfx_hal::image::Kind::D2(extent.width, extent.height, 1, 1),
        1,
        depth_format,
        gfx_hal::image::Tiling::Optimal,
        gfx_hal::image::Usage::DEPTH_STENCIL_ATTACHMENT,
        gfx_hal::image::ViewCapabilities::empty(),
        gfx_hal::memory::Properties::DEVICE_LOCAL,
        &memories,
      );

      match depth_image {
        Err(_) => {
          device.destroy_image_view(color_image_view);
          destroy_image(device, color_image);
          Err("Failed to create depth image")
        }
        Ok(depth_image) => {
          let depth_image_view = device
            .create_image_view(
              &depth_image.image,
              gfx_hal::image::ViewKind::D2,
              depth_format,
              gfx_hal::format::Swizzle::NO,
              gfx_hal::image::SubresourceRange {
                aspects: gfx_hal::format::Aspects::DEPTH | gfx_hal::format::Aspects::STENCIL,
                layers: std::ops::Range { start: 0, end: 1 },
                levels: std::ops::Range { start: 0, end: 1 },
              },
            );

          match depth_image_view {
            Err(_) => {
              destroy_image(device, depth_image);
              device.destroy_image_view(color_image_view);
              destroy_image(device, color_image);
              Err("Failed to create depth image view")
            }
            Ok(depth_image_view) => Ok(((color_image, color_image_view), (depth_image, depth_image_view))),
          }
        },
      }
    }
  }
}

pub unsafe fn do_the_render<B: gfx_hal::Backend>(
  device: &B::Device,
  command_pool: &mut gfx_hal::pool::CommandPool<B, gfx_hal::queue::Graphics>,
  cmd_queue: &mut gfx_hal::queue::CommandQueue<B, gfx_hal::queue::Graphics>,
  framebuffer: &B::Framebuffer,
  render_pass: &B::RenderPass,
  memories: &gfx_hal::adapter::MemoryProperties,
  extent: gfx_hal::image::Extent,
) -> () {
  use gfx_hal::device::Device;

  // Prepare vertex and index buffers
  let vertices: [Vertex; 3] = [
    Vertex { position: [1.0, 1.0, 0.0], color: [1.0, 0.0, 0.0] },
    Vertex { position: [-1.0, 1.0, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.0, -1.0, 0.0], color: [0.0, 0.0, 1.0] },
  ];
  let indices: [u32; 3] = [0, 1, 2];

  let vertex_buffer_size = ::std::mem::size_of::<[Vertex; 3]>();
  let index_buffer_size = ::std::mem::size_of::<[u32; 3]>();

  // WRITE THE TRIANGLE DATA
  let (vertex_buffer, index_buffer) = {
    let staging_buffer = {
      create_buffer::<B>(
        &device,
        gfx_hal::buffer::Usage::TRANSFER_SRC,
        gfx_hal::memory::Properties::CPU_VISIBLE | gfx_hal::memory::Properties::COHERENT,
        vertex_buffer_size as u64,
        &memories,
      ).unwrap()
    };

    {
      let mut staging_mapping: gfx_hal::mapping::Writer<B, Vertex> = device
        .acquire_mapping_writer(&staging_buffer.memory, 0..staging_buffer.capacity)
        .expect("Failed to acquire mapping writer");
      staging_mapping[..vertices.len()].copy_from_slice(&vertices);
      device
        .release_mapping_writer(staging_mapping)
        .expect("Failed to release mapping writer");
    }

    let vertex_buffer = {
      create_buffer::<B>(
        &device,
        gfx_hal::buffer::Usage::VERTEX | gfx_hal::buffer::Usage::TRANSFER_DST,
        gfx_hal::memory::Properties::DEVICE_LOCAL,
        vertex_buffer_size as u64,
        &memories,
      ).unwrap()
    };

    {
      let mut copy_cmd = command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
      copy_cmd.begin();
      copy_cmd.copy_buffer(
        &staging_buffer.buffer,
        &vertex_buffer.buffer,
        &[gfx_hal::command::BufferCopy { src: 0, dst: 0, size: vertex_buffer_size as u64 }],
      );
      copy_cmd.finish();
      let copy_fence = device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_nosemaphores(Some(&copy_cmd), Some(&copy_fence));
      device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
      device.destroy_fence(copy_fence);
    }

    {
      destroy_buffer(device, staging_buffer);
    }

    let staging_buffer = {
      create_buffer::<B>(
        &device,
        gfx_hal::buffer::Usage::TRANSFER_SRC,
        gfx_hal::memory::Properties::CPU_VISIBLE | gfx_hal::memory::Properties::COHERENT,
        index_buffer_size as u64,
        &memories,
      ).unwrap()
    };

    {
      let mut staging_mapping: gfx_hal::mapping::Writer<B, u32> = device
        .acquire_mapping_writer(&staging_buffer.memory, 0..staging_buffer.capacity)
        .expect("Failed to acquire mapping writer");
      staging_mapping[..indices.len()].copy_from_slice(&indices);
      device
        .release_mapping_writer(staging_mapping)
        .expect("Failed to release mapping writer");
    }

    let index_buffer = {
      create_buffer::<B>(
        &device,
        gfx_hal::buffer::Usage::INDEX | gfx_hal::buffer::Usage::TRANSFER_DST,
        gfx_hal::memory::Properties::DEVICE_LOCAL,
        index_buffer_size as u64,
        &memories,
      ).unwrap()
    };

    {
      let mut copy_cmd = command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
      copy_cmd.begin();
      copy_cmd.copy_buffer(
        &staging_buffer.buffer,
        &index_buffer.buffer,
        &[gfx_hal::command::BufferCopy { src: 0, dst: 0, size: index_buffer_size as u64 }],
      );
      copy_cmd.finish();
      let copy_fence = device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_nosemaphores(Some(&copy_cmd), Some(&copy_fence));
      device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
      device.destroy_fence(copy_fence);
    }

    {
      destroy_buffer(device, staging_buffer);
    }

    (vertex_buffer, index_buffer)
  };

  let (vertex_shader_module, fragment_shader_module, descriptor_set_layout, pipeline_layout, pipeline_cache, pipeline) = {
    let descriptor_set_layout = device
      .create_descriptor_set_layout(&[], &[])
      .expect("Failed to create descriptor set layout");

    let constant_size: usize = ::std::mem::size_of::<glm::TMat4<f32>>();
    let push_constants: Vec<(gfx_hal::pso::ShaderStageFlags, core::ops::Range<u32>)> = vec![
      (gfx_hal::pso::ShaderStageFlags::VERTEX, 0..((constant_size / ::std::mem::size_of::<f32>()) as u32)),
    ];

    let pipeline_layout = device
      .create_pipeline_layout(
        &[],
        push_constants,
      )
      .expect("Failed to create pipeline layout");

    let pipeline_cache = device
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
      device
        .create_shader_module(vertex_compile_artifact.as_binary_u8())
        .expect("Failed to create shader module")
    };
    let fragment_shader_module = {
      device
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
      cull_face: gfx_hal::pso::Face::BACK,
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
        rect: extent.rect(),
        depth: (0.0..1.0),
      }),
      scissor: Some(extent.rect()),
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
        main_pass: render_pass,
      },
      flags: pipeline_flags,
      parent: gfx_hal::pso::BasePipeline::None,
    };

    let pipeline = device
      .create_graphics_pipeline(&pipeline_desc, Some(&pipeline_cache))
      .expect("Failed to create pipeline");

    (vertex_shader_module, fragment_shader_module, descriptor_set_layout, pipeline_layout, pipeline_cache, pipeline)
  };

  {
    let mut command_buffer = command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();

    command_buffer.begin();

    {
      let clear_values = [
        gfx_hal::command::ClearValue::Color(gfx_hal::command::ClearColor::Float([0.0, 0.0, 0.2, 1.0])),
        gfx_hal::command::ClearValue::DepthStencil(gfx_hal::command::ClearDepthStencil(1.0, 0)),
      ];
      let mut encoder: gfx_hal::command::RenderPassInlineEncoder<_> = command_buffer.begin_render_pass_inline(
        &render_pass,
        &framebuffer,
        extent.rect(),
        clear_values.iter(),
      );

      let viewports = vec![gfx_hal::pso::Viewport { rect: extent.rect(), depth: (0.0..1.0) }];
      encoder.set_viewports(0, viewports);

      let scissors = vec![extent.rect()];
      encoder.set_scissors(0, scissors);

      encoder.bind_graphics_pipeline(&pipeline);

      encoder.bind_vertex_buffers(0, vec![(&vertex_buffer.buffer, 0)]);
      encoder.bind_index_buffer(gfx_hal::buffer::IndexBufferView {
        buffer: &index_buffer.buffer,
        offset: 0,
        index_type: gfx_hal::IndexType::U32,
      });

      let pos = vec![
        glm::vec3(-1.5f32, 0.0f32, -4.0f32),
        glm::vec3(0.0f32, 0.0f32, -2.5f32),
        glm::vec3(1.5f32, 0.0f32, -4.0f32),
      ];

      for v in pos {
        let perspective = glm::perspective(
          1.0, // glm::radians(60.0f32),
          (extent.width as f32) / (extent.height as f32),
          0.1f32,
          256.0f32,
        );

        let identiy4: glm::TMat4<f32> = glm::identity();

        let mvp_matrix: glm::TMat4<f32> = perspective * glm::translate(&identiy4, &v);

        let mvp_matrix_bits: Vec<u32> = mvp_matrix.data.iter().map(|x| x.to_bits()).collect();

        encoder.push_graphics_constants(
          &pipeline_layout,
          gfx_hal::pso::ShaderStageFlags::VERTEX,
          0,
          &mvp_matrix_bits[..],
        );
        encoder.draw_indexed(0..3, 0, 0..1);
      }
    }

    command_buffer.finish();

    let cmd_fence = device.create_fence(false).expect("Failed to create fence");
    cmd_queue.submit_nosemaphores(Some(&command_buffer), Some(&cmd_fence));
    device.wait_for_fence(&cmd_fence, core::u64::MAX).expect("Failed to wait for fence");
    device.destroy_fence(cmd_fence);

    device
      .wait_idle()
      .expect("Failed to wait for device to be idle");
  }

  {
    device.destroy_graphics_pipeline(pipeline);
    device.destroy_pipeline_cache(pipeline_cache);
    device.destroy_pipeline_layout(pipeline_layout);
    device.destroy_descriptor_set_layout(descriptor_set_layout);
    device.destroy_shader_module(fragment_shader_module);
    device.destroy_shader_module(vertex_shader_module);
    destroy_buffer(device, index_buffer);
    destroy_buffer(device, vertex_buffer);
  }
}
