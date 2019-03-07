/**
 * Export an image data object to the PAM format.
 *
 * @see http://netpbm.sourceforge.net/doc/pam.html
 * @param imageData Image data to export
 * @return The export PAM buffer
 */
export function imageDataToPam(imageData: ImageData): Buffer {
  const headerParts: string[] = [];
  headerParts.push("P7");
  headerParts.push(`WIDTH ${imageData.width.toString(10)}`);
  headerParts.push(`HEIGHT ${imageData.height.toString(10)}`);
  headerParts.push("DEPTH 4");
  headerParts.push("MAXVAL 255");
  headerParts.push("TUPLTYPE RGB_ALPHA");
  headerParts.push("ENDHDR");
  headerParts.push("");
  const header: string = headerParts.join("\n");

  const headerBuffer: Buffer = Buffer.from(header, "ascii");
  const dataBuffer: Buffer = Buffer.from(imageData.data as ArrayLike<number> as number[]);
  const result: Buffer = Buffer.allocUnsafe(headerBuffer.length + dataBuffer.length);

  headerBuffer.copy(result);
  dataBuffer.copy(result, headerBuffer.length);

  return result;
}
