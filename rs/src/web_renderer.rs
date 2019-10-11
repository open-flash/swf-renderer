#![allow(dead_code)]

use gfx_hal::Backend;
use gfx_hal::Instance;
use gfx_hal::command::CommandBuffer;
use gfx_hal::device::Device;
use gfx_hal::adapter::{Adapter, Gpu, PhysicalDevice};
use gfx_hal::queue::family::QueueFamily;
use gfx_hal::pool::CommandPool;
use gfx_hal::image::{Layout, Extent};
use gfx_hal::image::Access as ImageAccess;
use gfx_hal::pass;
#[allow(unused_imports)]
use gfx_hal::pso;
use gfx_hal::queue::{CommandQueue, QueueGroup};
use gfx_hal::window::{Surface, SwapchainConfig};
use std::borrow::Borrow;
use gfx_hal::window::PresentationSurface;
use log::{debug, info, warn};
use crate::swf_renderer::{SwfRenderer, Stage};
use std::mem::ManuallyDrop;
use gfx_hal::pso::{PipelineStage, Viewport, Rect};
use gfx_hal::window::Extent2D;
use gfx_hal::format::{Format, ChannelType};

const QUEUE_COUNT: usize = 1;
const DEFAULT_EXTENT2D: Extent2D = Extent2D { width: 640, height: 480 };
const DEFAULT_EXTENT: Extent = Extent { width: DEFAULT_EXTENT2D.width, height: DEFAULT_EXTENT2D.height, depth: 1 };
const DEFAULT_COLOR_FORMAT: Format = Format::Rgba8Srgb;

pub struct WebRenderer<B: Backend> {
  pub stage: Option<Stage>,

  pub device: B::Device,
  pub queue_group: QueueGroup<B>,
  pub command_pool: ManuallyDrop<B::CommandPool>,
  pub surface: B::Surface,

  pub memories: gfx_hal::adapter::MemoryProperties,
  pub color_format: gfx_hal::format::Format,

  pub render_pass: ManuallyDrop<B::RenderPass>,
  // Current frame count
  pub frame: u64,
}

fn is_graphics_family<B: Backend>(qf: &B::QueueFamily) -> bool {
  qf.queue_type().supports_graphics() && qf.max_queues() >= QUEUE_COUNT
}

impl<B: Backend> WebRenderer<B> {
  pub fn get_adapter<I: Instance<Backend=B>>(instance: &I) -> Option<Adapter<B>> {
    instance.enumerate_adapters().into_iter()
      .find(|a| {
        a.queue_families
          .iter()
          .any(is_graphics_family::<B>)
      })
  }

  pub fn new(mut adapter: Adapter<B>, mut surface: B::Surface) -> WebRenderer<B> {
//    let memory_types = adapter.physical_device.memory_properties().memory_types;
//    let limits = adapter.physical_device.limits();

    let memories = adapter.physical_device.memory_properties();
    debug!("{:?}", memories);
    let limits = adapter.physical_device.limits();
    debug!("{:?}", limits);

    let family: &B::QueueFamily = adapter
      .queue_families
      .iter()
      .find(|qf| surface.supports_queue_family(qf) && is_graphics_family::<B>(qf))
      .expect("Failed to find queue family with graphics support");

    let gpu: Gpu<B> = unsafe {
      adapter
        .physical_device
        .open(&[(family, &[1.0])], gfx_hal::Features::empty())
        .expect("Failed to open GPU")
    };
    let device: B::Device = gpu.device;
    let mut queue_groups: Vec<QueueGroup<B>> = gpu.queue_groups;
    let queue_group: QueueGroup<B> = queue_groups.pop().unwrap();

    let command_pool = unsafe {
      device
        .create_command_pool(queue_group.family, gfx_hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
        .expect("Failed to create command pool")
    };

//    let set_layout = unsafe {
//      device
//        .create_descriptor_set_layout(
//          &[
//            pso::DescriptorSetLayoutBinding {
//              binding: 0,
//              ty: pso::DescriptorType::SampledImage,
//              count: 1,
//              stage_flags: ShaderStageFlags::FRAGMENT,
//              immutable_samplers: false,
//            },
//            pso::DescriptorSetLayoutBinding {
//              binding: 1,
//              ty: pso::DescriptorType::Sampler,
//              count: 1,
//              stage_flags: ShaderStageFlags::FRAGMENT,
//              immutable_samplers: false,
//            },
//          ],
//          &[],
//        )
//        .expect("Can't create descriptor set layout")
//    };

    let (caps, formats, _present_modes) = surface.compatibility(&mut adapter.physical_device);
    info!("formats: {:?}", formats);

    let color_format = formats.map_or(DEFAULT_COLOR_FORMAT, |formats| {
      formats
        .iter()
        .find(|format| format.base_format().1 == ChannelType::Srgb)
        .map(|format| *format)
        .unwrap_or(formats[0])
    });

    let swap_config = SwapchainConfig::from_caps(&caps, color_format, DEFAULT_EXTENT2D);
    info!("{:?}", swap_config);

    unsafe {
      surface
        .configure_swapchain(&device, swap_config)
        .expect("Can't configure swapchain");
    };

    let render_pass: B::RenderPass = unsafe {
      let attachment: pass::Attachment = pass::Attachment {
        format: Some(color_format),
        samples: 1,
        ops: pass::AttachmentOps {
          load: pass::AttachmentLoadOp::Clear,
          store: pass::AttachmentStoreOp::Store,
        },
        stencil_ops: pass::AttachmentOps::DONT_CARE,
        layouts: Layout::Undefined..Layout::Present,
      };
      let attachments = [attachment];

      let subpass: pass::SubpassDesc = pass::SubpassDesc {
        colors: &[(0, Layout::ColorAttachmentOptimal)],
        depth_stencil: None,
        inputs: &[],
        resolves: &[],
        preserves: &[],
      };

      let dependencies = [
        pass::SubpassDependency {
          passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
          stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
          accesses: ImageAccess::empty()..(ImageAccess::COLOR_ATTACHMENT_READ | ImageAccess::COLOR_ATTACHMENT_WRITE),
        },
      ];

      let render_pass = device
        .create_render_pass(
          &attachments,
          &[subpass],
          &dependencies,
        )
        .expect("Failed to create render pass");

      render_pass
    };

    WebRenderer {
      stage: None,
      device,
      queue_group,
      command_pool: ManuallyDrop::new(command_pool),
      surface,
      memories,
      color_format,
      render_pass: ManuallyDrop::new(render_pass),
      frame: 0,
    }
  }

  fn draw(&mut self) -> () {
    let stage: &Stage = match &self.stage {
      Some(ref stage) => stage,
      None => {
        warn!("Skipping draw: no stage set");
        return;
      }
    };

    info!("Has stage: {:?}", &stage);

    let surface_image = unsafe {
      match self.surface.acquire_image(std::u64::MAX) {
        Ok((image, _)) => image,
        Err(_) => {
          warn!("Failed to acquire image");
          return;
        }
      }
    };

    info!("Got surface image");

    let framebuffer: B::Framebuffer = unsafe {
      let framebuffer = self.device
        .create_framebuffer(
          &self.render_pass,
          std::iter::once(surface_image.borrow()),
          DEFAULT_EXTENT,
        )
        .expect("Failed to create framebuffer");

      framebuffer
    };

    unsafe {
      let mut command_buffer: B::CommandBuffer = self.command_pool.allocate_one(gfx_hal::command::Level::Primary);
      command_buffer.begin_primary(gfx_hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);

      command_buffer.set_viewports(0, &[Viewport {
        rect: Rect { x: 0, y: 0, w: 640, h: 480 },
        depth: 0.0..1.0,
      }]);

      let clear_values = [
        gfx_hal::command::ClearValue { color: gfx_hal::command::ClearColor { float32: [0.0, 1.0, 0.0, 1.0] } },
//        gfx_hal::command::ClearValue { depth_stencil: gfx_hal::command::ClearDepthStencil { depth: 1.0, stencil: 0 } },
      ];
      command_buffer.begin_render_pass(
        &self.render_pass,
        &framebuffer,
        DEFAULT_EXTENT.rect(),
        clear_values.iter(),
        gfx_hal::command::SubpassContents::Inline,
      );

      command_buffer.finish();

      let cmd_queue: &mut B::CommandQueue = &mut self.queue_group.queues[0];
      let cmd_fence = self.device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_without_semaphores(Some(&command_buffer), Some(&cmd_fence));
      self.device.wait_for_fence(&cmd_fence, core::u64::MAX).expect("Failed to wait for fence");
      self.device.destroy_fence(cmd_fence);
    }

    unsafe {
      self.device.destroy_framebuffer(framebuffer);
    }

    warn!("NotImplemented: Draw");
  }
}

impl<B: Backend> SwfRenderer for WebRenderer<B> {
  fn render(&mut self, stage: Stage) -> () {
    info!("Set stage: {:?}", &stage);
    self.stage = Some(stage);
    self.draw();
  }
}

impl<B: Backend> Drop for WebRenderer<B> {
  fn drop(&mut self) -> () {
    unsafe {
      self.device
        .wait_idle()
        .expect("Failed to wait for device to be idle");

//      for (_, mesh) in self.shape_meshes.drain() {
//        destroy_buffer(&self.device, ManuallyDrop::into_inner(mesh.indices));
//        destroy_buffer(&self.device, ManuallyDrop::into_inner(mesh.vertices));
//      }
//
//      self.device.destroy_framebuffer(ManuallyDrop::into_inner(read(&self.framebuffer)));
//      self.device.destroy_render_pass(ManuallyDrop::into_inner(read(&self.render_pass)));
//
//      self.device.destroy_image_view(ManuallyDrop::into_inner(read(&self.depth_image_view)));
//      destroy_image(&self.device, ManuallyDrop::into_inner(read(&self.depth_image)));
//      self.device.destroy_image_view(ManuallyDrop::into_inner(read(&self.color_image_view)));
//      destroy_image(&self.device, ManuallyDrop::into_inner(read(&self.color_image)));

      self.device
        .destroy_command_pool(ManuallyDrop::take(&mut self.command_pool));
    }
  }
}
