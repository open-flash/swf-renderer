#![allow(dead_code)]

use crate::asset::{ClientAssetStore, MorphShapeId, ShapeId};
use crate::stage::Stage;
use crate::swf_renderer::SwfRenderer;
use gfx_hal::adapter::{Adapter, Gpu, PhysicalDevice};
use gfx_hal::command::CommandBuffer;
use gfx_hal::command;
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
use gfx_hal::queue::{CommandQueue, QueueGroup, Submission};
use gfx_hal::window::PresentationSurface;
use gfx_hal::window::{Extent2D, PresentMode, SurfaceCapabilities, SwapImageIndex};
use gfx_hal::window::{Surface, SwapchainConfig};
use gfx_hal::Backend;
use gfx_hal::Instance;
use log::{debug, info, warn};
use std::borrow::Borrow;
use std::mem::ManuallyDrop;
use swf_tree::tags::{DefineMorphShape, DefineShape};
use std::convert::TryFrom;
use core::iter;

const QUEUE_COUNT: usize = 1;
const DEFAULT_EXTENT: Extent2D = Extent2D {
  width: 640,
  height: 480,
};
const DEFAULT_COLOR_FORMAT: Format = Format::Rgba8Srgb;

struct FrameState<B: Backend> {
  submission_complete_semaphore: B::Semaphore,
  submission_complete_fence: B::Fence,
  command_pool: B::CommandPool,
  // Primary command buffer
  command_buffer: B::CommandBuffer,
}

pub struct GfxRenderer<B: Backend> {
  pub stage: Option<Stage>,

  pub device: B::Device,
  pub queue_group: QueueGroup<B>,
  pub surface: B::Surface,
  swapchain: SwapchainState,
  frames: Vec<FrameState<B>>,

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

/// Create or recreate the swapchain attached to the provided surface.
unsafe fn create_swapchain<B: Backend>(
  device: &B::Device,
  physical_device: &B::PhysicalDevice,
  surface: &mut B::Surface,
) -> SwapchainState {
  let (caps, formats, _supported_present_modes): (SurfaceCapabilities, Option<Vec<Format>>, Vec<PresentMode>) =
    surface.compatibility(physical_device);

  let format = formats.map_or(DEFAULT_COLOR_FORMAT, |formats| {
    formats
      .iter()
      .find(|format| format.base_format().1 == ChannelType::Srgb)
      .map(|format| *format)
      .unwrap_or(formats[0])
  });

  let extent: Extent2D = caps.current_extent.unwrap_or(DEFAULT_EXTENT);

  let config = SwapchainConfig::from_caps(&caps, format, extent);
  debug!("{:?}", config);

  let preferred_frames_in_flight: SwapImageIndex = if config.present_mode == PresentMode::Mailbox { 3 } else { 2 };
  let frames_in_flight = SwapImageIndex::min(
    *caps.image_count.end(),
    SwapImageIndex::max(*caps.image_count.start(), preferred_frames_in_flight),
  );

  surface
    .configure_swapchain(&device, config)
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

    let swapchain: SwapchainState = unsafe { create_swapchain::<B>(&device, &adapter.physical_device, &mut surface) };

    let mut frames: Vec<FrameState<B>> = Vec::with_capacity(usize::try_from(swapchain.frames_in_flight).unwrap());
    for _ in 0..swapchain.frames_in_flight {
      let submission_complete_semaphore: B::Semaphore = device.create_semaphore().expect("Failed to create semaphore");
      let submission_complete_fence: B::Fence = device.create_fence(true).expect("Failed to create fence");
      let mut command_pool: B::CommandPool = unsafe {
        device
          .create_command_pool(queue_group.family, gfx_hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
          .expect("Failed to create command pool")
      };
      let command_buffer: B::CommandBuffer = command_pool.allocate_one(command::Level::Primary);
      frames.push(FrameState {
        submission_complete_semaphore,
        submission_complete_fence,
        command_pool,
        command_buffer,
      });
    }

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
      frames,
      surface,
      swapchain,
      memories,
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

    let surface_image = unsafe {
      match self.surface.acquire_image(core::u64::MAX) {
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
          iter::once(surface_image.borrow()),
          self.swapchain.extent.to_extent(),
        )
        .expect("Failed to create framebuffer");

      framebuffer
    };

    // Compute index into frame resource ring buffer.
    // TODO Refactor conversion
    let frame_resource_idx: SwapImageIndex = SwapImageIndex::try_from(self.frame).unwrap() % self.swapchain.frames_in_flight;
    let frame: &mut FrameState<B> = &mut self.frames[usize::try_from(frame_resource_idx).unwrap()];

    unsafe {
      self.device.wait_for_fence(&frame.submission_complete_fence, core::u64::MAX).expect("Failed to wait for fence");
      self.device.reset_fence(&frame.submission_complete_fence).expect("Failed to reset fence");
      frame.command_pool.reset(false);

      frame.command_buffer.begin_primary(gfx_hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);

      frame.command_buffer.set_viewports(
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
      ];
      frame.command_buffer.begin_render_pass(
        &self.render_pass,
        &framebuffer,
        self.swapchain.extent.to_extent().rect(),
        clear_values.iter(),
        gfx_hal::command::SubpassContents::Inline,
      );

      frame.command_buffer.finish();

      let cmd_queue: &mut B::CommandQueue = &mut self.queue_group.queues[0];
      let submission = Submission {
        command_buffers: iter::once(&frame.command_buffer),
        wait_semaphores: None,
        signal_semaphores: iter::once(&frame.submission_complete_semaphore),
      };
      cmd_queue.submit(
        submission,
        Some(&frame.submission_complete_fence),
      );
      cmd_queue
        .present_surface(&mut self.surface, surface_image, Some(&frame.submission_complete_semaphore))
        .unwrap();
      self
        .device
        .wait_for_fence(&frame.submission_complete_fence, core::u64::MAX)
        .expect("Failed to wait for fence");
    }

    unsafe {
      self.device.destroy_framebuffer(framebuffer);
    }
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

      for frame in self.frames.drain(..) {
        self.device.destroy_command_pool(frame.command_pool);
        self.device.destroy_fence(frame.submission_complete_fence);
        self.device.destroy_semaphore(frame.submission_complete_semaphore);
      }

      self.surface.unconfigure_swapchain(&self.device);
    }
  }
}
