import { ReadableByteStream, ReadableStream } from "@open-flash/stream";
import * as canvas from "canvas";
import { Uint32, Uint8, UintSize } from "semantic-types";
import * as zlib from "zlib";

const RGB_SIZE: UintSize = 3;
const RGBA_SIZE: UintSize = 4;

export function decodeXSwfBmpSync(bytes: Uint8Array): canvas.ImageData {
  const stream: ReadableByteStream = new ReadableStream(bytes);
  const formatId: Uint8 = stream.readUint8();
  if (formatId !== 3) {
    throw new Error(`UnsupportedXSwfBmpFormatId: ${formatId}`);
  }
  const width: UintSize = stream.readUint16LE();
  const height: UintSize = stream.readUint16LE();
  const paddedWidth: UintSize = width + ((4 - (width % 4)) % 4);
  const colorCount: UintSize = stream.readUint8() + 1;
  const colors: Uint32[] = [];
  const compressedData: Uint8Array = stream.tailBytes();
  // `new Buffer` is a workaround to get zlib to work in the browser: TODO, make `zlib` accept Uint8Array
  const srcData: Buffer = zlib.inflateSync(new Buffer(compressedData));
  const data: Uint8ClampedArray = new Uint8ClampedArray(width * height * RGBA_SIZE);
  const dataView: DataView = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const colorTableSize: UintSize = RGB_SIZE * colorCount;
  for (let i: UintSize = 0; i < colorTableSize; i += 3) {
    const r: Uint8 = srcData[i];
    const g: Uint8 = srcData[i + 1];
    const b: Uint8 = srcData[i + 2];
    colors.push((r * 2 ** 24) + (g << 16) + (b << 8) + 0xff);
  }
  for (let y: UintSize = 0; y < height; y++) {
    for (let x: UintSize = 0; x < width; x++) {
      const ci: Uint8 = srcData[colorTableSize + y * paddedWidth + x];
      // TODO: Check how to handle out-of-bounds color indexes (currently we default to opaque black)
      const c: Uint32 = ci < colors.length ? colors[ci] : 0x000000ff;
      dataView.setUint32(RGBA_SIZE * (y * width + x), c, false);
    }
  }
  return canvas.createImageData(data, width, height);
}
