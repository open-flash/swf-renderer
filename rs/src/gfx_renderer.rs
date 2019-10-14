#![allow(dead_code)]

use crate::asset::{ClientAssetStore, MorphShapeId, ShapeId};
use crate::stage::Stage;
use crate::swf_renderer::SwfRenderer;
use gfx_hal::adapter::{Adapter, Gpu, PhysicalDevice};
use gfx_hal::command::CommandBuffer;
use gfx_hal::device::Device;
use gfx_hal::format::{ChannelType, Format};
use gfx_hal::image::Access as ImageAccess;
use gfx_hal::image::Layout;
use gfx_hal::pass;
use gfx_hal::pool::CommandPool;
#[allow(unused_imports)]
use gfx_hal::pso;
use gfx_hal::pso::{PipelineStage, Rect, Viewport};
use gfx_hal::queue::family::QueueFamily;
use gfx_hal::queue::{CommandQueue, QueueGroup};
use gfx_hal::window::PresentationSurface;
use gfx_hal::window::{Extent2D, PresentMode, SurfaceCapabilities, SwapImageIndex};
use gfx_hal::window::{Surface, SwapchainConfig};
use gfx_hal::Backend;
use gfx_hal::Instance;
use log::{debug, info, warn};
use std::borrow::Borrow;
use std::mem::ManuallyDrop;
use swf_tree::tags::{DefineMorphShape, DefineShape};

const QUEUE_COUNT: usize = 1;
const DEFAULT_EXTENT: Extent2D = Extent2D {
  width: 640,
  height: 480,
};
const DEFAULT_COLOR_FORMAT: Format = Format::Rgba8Srgb;

pub struct GfxRenderer<B: Backend> {
  pub stage: Option<Stage>,

  pub device: B::Device,
  pub queue_group: QueueGroup<B>,
  pub command_pool: ManuallyDrop<B::CommandPool>,
  pub surface: B::Surface,
  swapchain: SwapchainState,

  pub memories: gfx_hal::adapter::MemoryProperties,

  pub render_pass: ManuallyDrop<B::RenderPass>,
  // Current frame count
  pub frame: u64,
}

//fn is_graphics_family<B: Backend>(qf: &B::QueueFamily) -> bool {
//  qf.queue_type().supports_graphics() && qf.max_queues() >= QUEUE_COUNT
//}

fn find_graphics_queue_family<'a, B: Backend>(
  adapter: &'a Adapter<B>,
  surface: &B::Surface,
) -> Option<&'a B::QueueFamily> {
  adapter.queue_families.iter().find(|qf| {
    let surf: bool = surface.supports_queue_family(qf);
    let graph: bool = qf.queue_type().supports_graphics() && qf.max_queues() >= QUEUE_COUNT;
    surf && graph
  })
}

struct SwapchainState {
  format: Format,
  extent: Extent2D,
  frames_in_flight: SwapImageIndex,
}

/// If the swapchain is not created, create it. Otherwise reset it.
unsafe fn create_swapchain<B: Backend>(
  device: &B::Device,
  physical_device: &B::PhysicalDevice,
  surface: &mut B::Surface,
) -> SwapchainState {
  let (caps, formats, supported_present_modes): (SurfaceCapabilities, Option<Vec<Format>>, Vec<PresentMode>) =
    surface.compatibility(physical_device);

  let present_mode: PresentMode = {
    const PREFERRED_MODES: [PresentMode; 2] = [PresentMode::Mailbox, PresentMode::Fifo];
    PREFERRED_MODES
      .iter()
      .cloned()
      .find(|pm| supported_present_modes.contains(pm))
      .expect("Failed to negotiate present mode")
  };

  let format = formats.map_or(DEFAULT_COLOR_FORMAT, |formats| {
    formats
      .iter()
      .find(|format| format.base_format().1 == ChannelType::Srgb)
      .map(|format| *format)
      .unwrap_or(formats[0])
  });

  let extent: Extent2D = caps.current_extent.unwrap_or(DEFAULT_EXTENT);

  let mut swapchain_config = SwapchainConfig::from_caps(&caps, format, extent);
  swapchain_config.present_mode = present_mode;
  debug!("{:?}", swapchain_config);

  let preferred_frames_in_flight: SwapImageIndex = if present_mode == PresentMode::Mailbox { 3 } else { 2 };
  let frames_in_flight = SwapImageIndex::min(
    *caps.image_count.end(),
    SwapImageIndex::max(*caps.image_count.start(), preferred_frames_in_flight),
  );

  surface
    .configure_swapchain(&device, swapchain_config)
    .expect("Failed to configure swapchain");

  SwapchainState {
    format,
    frames_in_flight,
    extent,
  }
}

impl<B: Backend> GfxRenderer<B> {
  pub fn get_adapter<I: Instance<Backend = B>>(instance: &I, surface: &B::Surface) -> Option<Adapter<B>> {
    instance
      .enumerate_adapters()
      .into_iter()
      .find(|a| find_graphics_queue_family::<B>(a, surface).is_some())
  }

  pub fn new(adapter: Adapter<B>, mut surface: B::Surface) -> GfxRenderer<B> {
    let memories = adapter.physical_device.memory_properties();
    debug!("{:?}", memories);
    let limits = adapter.physical_device.limits();
    debug!("{:?}", limits);
    let surface_compat = surface.compatibility(&adapter.physical_device);
    debug!("{:?}", surface_compat);

    let family: &B::QueueFamily =
      find_graphics_queue_family(&adapter, &surface).expect("Failed to find queue family with graphics support");

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
        .create_command_pool(
          queue_group.family,
          gfx_hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL,
        )
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

    let swapchain: SwapchainState = unsafe { create_swapchain::<B>(&device, &adapter.physical_device, &mut surface) };

    let render_pass: B::RenderPass = unsafe {
      let attachment: pass::Attachment = pass::Attachment {
        format: Some(swapchain.format),
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

      let dependencies = [pass::SubpassDependency {
        passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
        stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
        accesses: ImageAccess::empty()..(ImageAccess::COLOR_ATTACHMENT_READ | ImageAccess::COLOR_ATTACHMENT_WRITE),
      }];

      let render_pass = device
        .create_render_pass(&attachments, &[subpass], &dependencies)
        .expect("Failed to create render pass");

      render_pass
    };

    GfxRenderer {
      stage: None,
      device,
      queue_group,
      command_pool: ManuallyDrop::new(command_pool),
      surface,
      swapchain,
      memories,
      render_pass: ManuallyDrop::new(render_pass),
      frame: 0,
    }
  }

  fn refresh_swapchain(&mut self) {}

  fn draw(&mut self) -> () {
    let stage: &Stage = match &self.stage {
      Some(ref stage) => stage,
      None => {
        warn!("Skipping draw: no stage set");
        return;
      }
    };

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
      let framebuffer = self
        .device
        .create_framebuffer(
          &self.render_pass,
          std::iter::once(surface_image.borrow()),
          self.swapchain.extent.to_extent(),
        )
        .expect("Failed to create framebuffer");

      framebuffer
    };

    unsafe {
      let mut command_buffer: B::CommandBuffer = self.command_pool.allocate_one(gfx_hal::command::Level::Primary);
      command_buffer.begin_primary(gfx_hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);

      command_buffer.set_viewports(
        0,
        &[Viewport {
          rect: Rect {
            x: 0,
            y: 0,
            w: 640,
            h: 480,
          },
          depth: 0.0..1.0,
        }],
      );

      let color_f32: [f32; 4] = [
        f32::from(stage.background_color.r) / 255.0,
        f32::from(stage.background_color.g) / 255.0,
        f32::from(stage.background_color.b) / 255.0,
        1.0,
      ];

      let clear_values = [
        gfx_hal::command::ClearValue {
          color: gfx_hal::command::ClearColor { float32: color_f32 },
        },
        //        gfx_hal::command::ClearValue { depth_stencil: gfx_hal::command::ClearDepthStencil { depth: 1.0, stencil: 0 } },
      ];
      command_buffer.begin_render_pass(
        &self.render_pass,
        &framebuffer,
        self.swapchain.extent.to_extent().rect(),
        clear_values.iter(),
        gfx_hal::command::SubpassContents::Inline,
      );

      command_buffer.finish();

      let cmd_queue: &mut B::CommandQueue = &mut self.queue_group.queues[0];
      let cmd_fence = self.device.create_fence(false).expect("Failed to create fence");
      cmd_queue.submit_without_semaphores(Some(&command_buffer), Some(&cmd_fence));
      cmd_queue
        .present_surface(&mut self.surface, surface_image, None)
        .unwrap();
      self
        .device
        .wait_for_fence(&cmd_fence, core::u64::MAX)
        .expect("Failed to wait for fence");
      self.device.destroy_fence(cmd_fence);
    }

    unsafe {
      self.device.destroy_framebuffer(framebuffer);
    }

    warn!("NotImplemented: Draw");
  }
}

impl<B: Backend> SwfRenderer for GfxRenderer<B> {
  fn render(&mut self, stage: Stage) -> () {
    self.stage = Some(stage);
    self.draw();
  }
}

impl<B: Backend> ClientAssetStore for GfxRenderer<B> {
  fn register_shape(&mut self, _tag: &DefineShape) -> ShapeId {
    ShapeId(0)
  }

  fn register_morph_shape(&mut self, _tag: &DefineMorphShape) -> MorphShapeId {
    MorphShapeId(0)
  }
}

impl<B: Backend> Drop for GfxRenderer<B> {
  fn drop(&mut self) -> () {
    unsafe {
      self.device.wait_idle().expect("Failed to wait for device to be idle");

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

      self.surface.unconfigure_swapchain(&self.device);

      self
        .device
        .destroy_command_pool(ManuallyDrop::take(&mut self.command_pool));
    }
  }
}
