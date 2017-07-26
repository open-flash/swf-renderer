export function exportGl(ctx: WebGLRenderingContext): ImageData {
  const width: number = ctx.drawingBufferWidth;
  const height: number = ctx.drawingBufferHeight;
  const data: Uint8Array = new Uint8Array(width * height * 4);
  ctx.readPixels(0, 0, width, height, ctx.RGBA, ctx.UNSIGNED_BYTE, data);
  return {width, height, data: Uint8ClampedArray.from(data)};
}
