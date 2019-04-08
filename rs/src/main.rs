use gfx_backend_vulkan as gfx_backend;
use gfx_hal::adapter::PhysicalDevice;
use gfx_hal::device::Device;
use gfx_hal::Instance;
use gfx_hal::queue::family::QueueFamily;

use env_logger;
use swf_renderer::gfx;
use swf_renderer::pam;

const GFX_APP_NAME: &'static str = "ofl-renderer";
const GFX_BACKEND_VERSION: u32 = 1;
const QUEUE_COUNT: usize = 1;
const VIEWPORT_WIDTH: u32 = 1024;
const VIEWPORT_HEIGHT: u32 = 1024;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Vertex {
  pub position: [f32; 3],
  pub color: [f32; 3],
}

fn main() {
  env_logger::init();

  let instance: gfx_backend::Instance = gfx_backend::Instance::create(GFX_APP_NAME, GFX_BACKEND_VERSION);

  let adapter: gfx_hal::Adapter<gfx_backend::Backend> = instance
    .enumerate_adapters()
    .into_iter()
    .find(|a| {
      a.queue_families
        .iter()
        .any(|qf| qf.supports_graphics())
    })
    .expect("Failed to find a compatible GPU adapter!");

  let physical_device: &gfx_backend::PhysicalDevice = &adapter.physical_device;

  let (device, mut queue_group): (gfx_backend::Device, gfx_hal::QueueGroup<gfx_backend::Backend, gfx_hal::queue::capability::Graphics>) = adapter
    .open_with::<_, gfx_hal::queue::capability::Graphics>(QUEUE_COUNT, |_qf| true)
    .expect("Failed to open GPU device");

  let mut command_pool = unsafe {
    device
      .create_command_pool_typed(&queue_group, gfx_hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
      .expect("Failed to create command pool")
  };

  let cmd_queue = &mut queue_group.queues[0];

  let memory_types = physical_device
    .memory_properties()
    .memory_types;
  // Prepare vertex and index buffers

  let extent = gfx_hal::image::Extent { width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT, depth: 1 };

  let color_format = gfx_hal::format::Format::Rgba8Unorm;
  let depth_format = gfx::get_supported_depth_format::<gfx_backend::Backend>(physical_device).expect("Failed to find supported depth format");

  // Create attachments
  let ((color_image, color_image_view), (depth_image, depth_image_view)) = unsafe {
    gfx::create_images::<gfx_backend::Backend>(&device, color_format, depth_format, &memory_types)
  };

  //  Create renderpass
  let (frame_buffer, render_pass) = unsafe {
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

    let dep0: gfx_hal::pass::SubpassDependency = gfx_hal::pass::SubpassDependency {
      passes: std::ops::Range { start: gfx_hal::pass::SubpassRef::External, end: gfx_hal::pass::SubpassRef::Pass(0) },
      stages: std::ops::Range { start: gfx_hal::pso::PipelineStage::BOTTOM_OF_PIPE, end: gfx_hal::pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT },
      accesses: std::ops::Range { start: gfx_hal::image::Access::MEMORY_READ, end: gfx_hal::image::Access::COLOR_ATTACHMENT_READ | gfx_hal::image::Access::COLOR_ATTACHMENT_WRITE },
    };

    let dep1: gfx_hal::pass::SubpassDependency = gfx_hal::pass::SubpassDependency {
      passes: std::ops::Range { start: gfx_hal::pass::SubpassRef::Pass(0), end: gfx_hal::pass::SubpassRef::External },
      stages: std::ops::Range { start: gfx_hal::pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT, end: gfx_hal::pso::PipelineStage::BOTTOM_OF_PIPE },
      accesses: std::ops::Range { start: gfx_hal::image::Access::COLOR_ATTACHMENT_READ | gfx_hal::image::Access::COLOR_ATTACHMENT_WRITE, end: gfx_hal::image::Access::MEMORY_READ },
    };

    let dependencies = [dep0, dep1];

    let render_pass = device
      .create_render_pass(
        &attachments,
        &[subpass_desc],
        &dependencies,
      )
      .expect("Failed to create render pass");

    let image_views = vec![&color_image_view, &depth_image_view];

    let frame_buffer = device
      .create_framebuffer(
        &render_pass,
        image_views.into_iter(),
        gfx_hal::image::Extent { width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT, depth: 1 },
      )
      .expect("Failed to create frame buffer");

    (frame_buffer, render_pass)
  };

  unsafe {
    gfx::do_the_render(&device, &mut command_pool, cmd_queue, &frame_buffer, &render_pass, &memory_types);
  }

  let (dst_image, dst_image_data) = unsafe {
    let dst_image = gfx::create_image::<gfx_backend::Backend>(
      &device,
      gfx_hal::image::Kind::D2(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1, 1),
      1,
      color_format,
      gfx_hal::image::Tiling::Linear,
      gfx_hal::image::Usage::TRANSFER_DST,
      gfx_hal::image::ViewCapabilities::empty(),
      gfx_hal::memory::Properties::CPU_VISIBLE | gfx_hal::memory::Properties::COHERENT,
      &memory_types,
    ).unwrap();
    {
      let mut copy_cmd = command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
      copy_cmd.begin();

      {
        let src_state: gfx_hal::image::State = (gfx_hal::image::Access::empty(), gfx_hal::image::Layout::Undefined);
        let dst_state: gfx_hal::image::State = (gfx_hal::image::Access::TRANSFER_WRITE, gfx_hal::image::Layout::TransferDstOptimal);
        let barrier: gfx_hal::memory::Barrier<gfx_backend::Backend> = gfx_hal::memory::Barrier::Image {
          states: (src_state..dst_state),
          target: &dst_image.image,
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
        extent: extent.clone(),
      };
      copy_cmd.copy_image(
        &color_image.image,
        gfx_hal::image::Layout::TransferSrcOptimal,
        &dst_image.image,
        gfx_hal::image::Layout::TransferDstOptimal,
        Some(&image_copy_regions),
      );

      {
        let src_state: gfx_hal::image::State = (gfx_hal::image::Access::TRANSFER_WRITE, gfx_hal::image::Layout::TransferDstOptimal);
        let dst_state: gfx_hal::image::State = (gfx_hal::image::Access::MEMORY_READ, gfx_hal::image::Layout::General);
        let barrier: gfx_hal::memory::Barrier<gfx_backend::Backend> = gfx_hal::memory::Barrier::Image {
          states: (src_state..dst_state),
          target: &dst_image.image,
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

      let copy_fence = device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_nosemaphores(Some(&copy_cmd), Some(&copy_fence));
      device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
      device.destroy_fence(copy_fence);
    }

    let dst_image_footprint = device.get_image_subresource_footprint(
      &dst_image.image,
      gfx_hal::image::Subresource {
        aspects: gfx_hal::format::Aspects::COLOR,
        level: 0,
        layer: 0,
      },
    );

    let dst_image_data = {
      let dst_mapping: gfx_hal::mapping::Reader<gfx_backend::Backend, u8> = device
        .acquire_mapping_reader(&dst_image.memory, dst_image_footprint.slice)
        .expect("Failed to acquire mapping reader");

      let mut dst_image_data: Vec<u8> = Vec::new();

      for y in 0..(VIEWPORT_HEIGHT as usize) {
        let row_idx: usize = y * dst_image_footprint.row_pitch as usize;
        for x in 0..(VIEWPORT_WIDTH as usize) {
          let idx: usize = row_idx + 4 * x;
          dst_image_data.push(dst_mapping[idx + 0]);
          dst_image_data.push(dst_mapping[idx + 1]);
          dst_image_data.push(dst_mapping[idx + 2]);
          dst_image_data.push(dst_mapping[idx + 3]);
        }
      }

      device
        .release_mapping_reader(dst_mapping);

      dst_image_data
    };

    (dst_image, dst_image_data)
  };

  {
    let pam_file = ::std::fs::File::create("out.pam").expect("Failed to create actual AST file");
    let mut pam_writer = ::std::io::BufWriter::new(pam_file);
    pam::write_pam(&mut pam_writer, VIEWPORT_WIDTH as usize, VIEWPORT_HEIGHT as usize, &dst_image_data).expect("Failed to write PAM");
  }

  unsafe {
    gfx::destroy_image(&device, dst_image);

    device.destroy_framebuffer(frame_buffer);
    device.destroy_render_pass(render_pass);

    device.destroy_image_view(color_image_view);
    gfx::destroy_image(&device, color_image);
    device.destroy_image_view(depth_image_view);
    gfx::destroy_image(&device, depth_image);

    device.destroy_command_pool(command_pool.into_raw());
  }

  dbg!("done");
}
