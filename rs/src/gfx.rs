#![allow(dead_code)]
#![macro_use]

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
    gfx_hal::format::Format::D32SfloatS8Uint,
    gfx_hal::format::Format::D32Sfloat,
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
  memory_types: &[gfx_hal::adapter::MemoryType],
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
