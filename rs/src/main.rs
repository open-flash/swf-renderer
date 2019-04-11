use gfx_backend_vulkan as gfx_backend;
use gfx_hal::adapter::PhysicalDevice;
use gfx_hal::device::Device;
use gfx_hal::Instance;
use gfx_hal::queue::family::QueueFamily;

use env_logger;
use swf_renderer::gfx;
use swf_renderer::pam;
use swf_renderer::headless_renderer::HeadlessGfxRenderer;

const GFX_APP_NAME: &'static str = "ofl-renderer";
const GFX_BACKEND_VERSION: u32 = 1;
const QUEUE_COUNT: usize = 1;
const VIEWPORT_WIDTH: u32 = 256;
const VIEWPORT_HEIGHT: u32 = 256;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Vertex {
  pub position: [f32; 3],
  pub color: [f32; 3],
}

fn main() {
  env_logger::init();

  let instance: gfx_backend::Instance = gfx_backend::Instance::create(GFX_APP_NAME, GFX_BACKEND_VERSION);

  let mut renderer = HeadlessGfxRenderer::<gfx_backend::Backend>::new(&instance, VIEWPORT_WIDTH as usize, VIEWPORT_HEIGHT as usize)
    .unwrap();

  let cmd_queue = &mut renderer.queue_group.queues[0];

  unsafe {
    gfx::do_the_render(
      &renderer.device,
      &mut renderer.command_pool,
      cmd_queue,
      &renderer.framebuffer,
      &renderer.render_pass,
      &renderer.memories,
      renderer.viewport_extent,
    );
  }

  let (dst_image, dst_image_data) = unsafe {
    let dst_image = gfx::create_image::<gfx_backend::Backend>(
      &renderer.device,
      gfx_hal::image::Kind::D2(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1, 1),
      1,
      renderer.color_format,
      gfx_hal::image::Tiling::Linear,
      gfx_hal::image::Usage::TRANSFER_DST,
      gfx_hal::image::ViewCapabilities::empty(),
      gfx_hal::memory::Properties::CPU_VISIBLE | gfx_hal::memory::Properties::COHERENT,
      &renderer.memories,
    ).unwrap();
    {
      let mut copy_cmd = renderer.command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
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
        extent: renderer.viewport_extent,
      };
      copy_cmd.copy_image(
        &renderer.color_image.image,
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

      let copy_fence = renderer.device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_nosemaphores(Some(&copy_cmd), Some(&copy_fence));
      renderer.device.wait_for_fence(&copy_fence, core::u64::MAX).expect("Failed to wait for fence");
      renderer.device.destroy_fence(copy_fence);
    }

    let dst_image_footprint = renderer.device.get_image_subresource_footprint(
      &dst_image.image,
      gfx_hal::image::Subresource {
        aspects: gfx_hal::format::Aspects::COLOR,
        level: 0,
        layer: 0,
      },
    );

    let dst_image_data = {
      let dst_mapping: gfx_hal::mapping::Reader<gfx_backend::Backend, u8> = renderer.device
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

      renderer.device
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
    gfx::destroy_image(&renderer.device, dst_image);
  }

  dbg!("done");
}
