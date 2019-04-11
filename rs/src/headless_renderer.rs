use std::mem::ManuallyDrop;

use gfx_hal::{Backend as GfxBackend, Graphics};
use gfx_hal::adapter::PhysicalDevice;
use gfx_hal::device::Device;
use gfx_hal::image::Extent;
use gfx_hal::queue::family::QueueFamily;
use swf_tree::Shape as SwfShape;

use crate::gfx::{AttachedImage, create_images, destroy_image, get_supported_depth_format};
use crate::renderer::{Image, Renderer};

const GFX_APP_NAME: &'static str = "ofl-renderer";
const GFX_BACKEND_VERSION: u32 = 1;
const QUEUE_COUNT: usize = 1;

pub struct HeadlessGfxRenderer<B: GfxBackend> {
  pub viewport_extent: Extent,
  pub stage: Option<SwfShape>,

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

    let (device, mut queue_group): (<I::Backend as GfxBackend>::Device, _) = adapter
      .open_with::<_, gfx_hal::queue::capability::Graphics>(QUEUE_COUNT, |_qf| true)
      .map_err(|_| "Failed to open GPU device")?;


    let memories = adapter.physical_device.memory_properties();
    let color_format = gfx_hal::format::Format::Rgba8Unorm;
    let depth_format = get_supported_depth_format::<I::Backend>(&adapter.physical_device)
      .ok_or("Failed to find supported depth format")?;

    let mut command_pool = unsafe {
      device
        .create_command_pool_typed(&queue_group, gfx_hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
        .map_err(|_| "Failed to create command pool")?
    };

    let cmd_queue = &mut queue_group.queues[0];

    // Create attachments
    let attachments = unsafe {
      create_images::<I::Backend>(&device, color_format, depth_format, &memories)
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

  pub fn get_image() -> Image {
    unimplemented!()
  }
}

impl<B: GfxBackend> Drop for HeadlessGfxRenderer<B> {
  fn drop(&mut self) -> () {
    unsafe {
      use core::ptr::read;

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
  fn set_stage(&mut self, shape: SwfShape) -> () {
    self.stage = Some(shape);
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
