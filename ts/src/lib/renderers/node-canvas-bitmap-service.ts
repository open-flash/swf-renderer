import canvas from "canvas";
import { UintSize } from "semantic-types";
import { DefineBitmap } from "swf-tree/tags";
import { Bitmap, BitmapConsumer, BitmapProvider } from "../bitmap-service";
import { decodeXSwfBmpSync } from "../decode-x-swf-bmp";

export class NodeCanvasBitmapService implements BitmapConsumer, BitmapProvider<canvas.Canvas | canvas.Image> {
  private readonly bitmaps: Map<UintSize, Bitmap<canvas.Canvas | canvas.Image>>;

  constructor() {
    this.bitmaps = new Map();
  }

  addBitmap(tag: DefineBitmap): void {
    let bitmap: Bitmap<canvas.Canvas | canvas.Image>;

    switch (tag.mediaType) {
      case "image/x-swf-bmp": {
        const decoded: canvas.ImageData = decodeXSwfBmpSync(tag.data);
        const cvs: canvas.Canvas = canvas.createCanvas(decoded.width, decoded.height);
        const ctx: canvas.CanvasRenderingContext2D = cvs.getContext("2d");

        ctx.putImageData(decoded, 0, 0);
        bitmap = {
          width: decoded.width,
          height: decoded.height,
          bitmap: cvs,
          bitmap$: Promise.resolve(cvs),
        };
        break;
      }
      default:
        throw new Error(`NotImplemented: Support for ${tag.mediaType} images`);
    }

    this.bitmaps.set(tag.id, bitmap);
  }

  getById(id: UintSize): Bitmap<canvas.Canvas | canvas.Image> {
    const bitmap: Bitmap<canvas.Canvas | canvas.Image> | undefined = this.bitmaps.get(id);
    if (bitmap === undefined) {
      throw new Error(`BitmapNotFound: ${id}`);
    }
    return bitmap;
  }
}
