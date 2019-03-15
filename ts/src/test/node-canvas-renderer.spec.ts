import canvas from "canvas";
import chai from "chai";
import fs from "fs";
import { fromSysPath, join, toSysPath } from "furi";
import { JsonReader } from "kryo/readers/json";
import sysPath from "path";
import pixelmatch from "pixelmatch";
import { Sfixed16P16 } from "swf-tree/fixed-point/sfixed16p16";
import { $DefineShape, DefineShape } from "swf-tree/tags";
import url from "url";
import { DisplayObjectType } from "../lib/display/display-object-type";
import { Stage } from "../lib/display/stage";
import { NodeCanvasRenderer } from "../lib/renderers/node-canvas-renderer";
import meta from "./meta.js";

const PROJECT_ROOT: string = sysPath.join(meta.dirname, "..", "..", "..");
const TEST_SAMPLES_ROOT: string = sysPath.join(PROJECT_ROOT, "..", "tests", "decode-shape");

const JSON_READER: JsonReader = new JsonReader();

describe("render", function () {
  for (const sample of getSamples()) {
    it(sample.name, async function () {
      const inputJson: string = fs.readFileSync(
        sysPath.join(TEST_SAMPLES_ROOT, `${sample.name}.ast.json`),
        {encoding: "UTF-8"},
      );
      const inputTag: DefineShape = $DefineShape.read(JSON_READER, inputJson);

      const width: number = Math.ceil((inputTag.bounds.xMax - inputTag.bounds.xMin) / 20);
      const height: number = Math.ceil((inputTag.bounds.yMax - inputTag.bounds.yMin) / 20);

      const input: Stage = {
        width,
        height,
        backgroundColor: {r: 0, g: 0, b: 0, a: 0},
        children: [
          {
            type: DisplayObjectType.Shape,
            definition: inputTag,
            matrix: {
              scaleX: Sfixed16P16.fromValue(1),
              scaleY: Sfixed16P16.fromValue(1),
              rotateSkew0: Sfixed16P16.fromValue(0),
              rotateSkew1: Sfixed16P16.fromValue(0),
              translateX: -inputTag.bounds.xMin,
              translateY: -inputTag.bounds.yMin,
            },
          },
        ],
      };

      const ncr: NodeCanvasRenderer = new NodeCanvasRenderer(width, height);
      ncr.render(input);

      const actualCanvas: canvas.Canvas = ncr.canvas;
      const actualPngBuffer: Buffer = await toPngBuffer(actualCanvas);
      await writeFile(join(fromSysPath(TEST_SAMPLES_ROOT), [`${sample.name}.ts-out.png`]), actualPngBuffer);
      const expectedUri: url.URL = join(fromSysPath(TEST_SAMPLES_ROOT), [`${sample.name}.png`]);
      const expectedCanvas: canvas.Image = await loadImage(expectedUri);
      const comparison: ImageComparison = await compareImages(actualCanvas, expectedCanvas);
      if (comparison.sameSize) {
        const diffPngBuffer: Buffer = await toPngBuffer(comparison.diffImage);
        await writeFile(join(fromSysPath(TEST_SAMPLES_ROOT), [`${sample.name}.ts-diff.png`]), diffPngBuffer);
      }
      assertSimilarImages(comparison);
    });
  }
});

async function toPngBuffer(cvs: canvas.Canvas): Promise<Buffer> {
  return new Promise<Buffer>((resolve, reject) => {
    cvs.toBuffer(
      (err: Error | null, buffer: Buffer): void => {
        if (err !== null) {
          reject(err);
        } else {
          resolve(buffer);
        }
      },
      "image/png",
    );
  });
}

type ImageLike = canvas.Canvas | canvas.Image;

async function asImageData(input: ImageLike): Promise<canvas.ImageData> {
  const ctx: canvas.CanvasRenderingContext2D = canvas.createCanvas(input.width, input.height).getContext("2d");
  ctx.drawImage(input, 0, 0);
  return ctx.getImageData(0, 0, input.width, input.height);
}

type ImageComparison = ImageComparisonDifferentSize | ImageComparisonSameSize;

interface ImageComparisonDifferentSize {
  actual: ImageLike;
  expected: ImageLike;
  sameSize: false;
}

interface ImageComparisonSameSize {
  actual: ImageLike;
  expected: ImageLike;
  sameSize: true;
  diffCount: number;
  diffImage: canvas.Canvas;
}

async function compareImages(actual: ImageLike, expected: ImageLike): Promise<ImageComparison> {
  if (actual.width !== expected.width || actual.height !== expected.height) {
    return {actual, expected, sameSize: false};
  }
  const actualImageData: canvas.ImageData = await asImageData(actual);
  const expectedImageData: canvas.ImageData = await asImageData(expected);
  const diffImage: canvas.Canvas = canvas.createCanvas(expected.width, expected.height);
  const diffCtx: canvas.CanvasRenderingContext2D = diffImage.getContext("2d");
  const diffData: canvas.ImageData = diffCtx.getImageData(0, 0, expected.width, expected.height);
  const diffCount: number = pixelmatch(
    actualImageData.data as any as Uint8Array,
    expectedImageData.data as any as Uint8Array,
    diffData.data as any as Uint8Array,
    expected.width,
    expected.height,
    {threshold: 0.05},
  );
  diffCtx.putImageData(diffData, 0, 0);
  // console.warn(diffCount);
  return {actual, expected, sameSize: true, diffCount, diffImage};
}

function assertSimilarImages(comparison: ImageComparison): void | never {
  if (!comparison.sameSize) {
    throw new chai.AssertionError("Images do not have the same size");
  }
  const pixelCount: number = comparison.expected.width * comparison.expected.height;
  const ratio: number = comparison.diffCount / pixelCount;
  const THRESHOLD: number = 0.0001;
  if (ratio > THRESHOLD) {
    throw new chai.AssertionError(
      `Image difference above threshold: ${comparison.diffCount} / ${pixelCount} = ${ratio} > ${THRESHOLD}`,
    );
  }
}

async function writeFile(filePath: fs.PathLike, data: Buffer): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    fs.writeFile(filePath, data, (err: NodeJS.ErrnoException | null) => {
      if (err !== null) {
        reject(err);
      } else {
        resolve();
      }
    });
  });
}

export async function loadImage(uri: url.URL): Promise<canvas.Image> {
  return new Promise<canvas.Image>((resolve, reject): void => {
    const img: canvas.Image = new canvas.Image();

    // TODO: Do not restore the old values, simply set to `undefined`.
    const oldOnError: typeof img.onerror = img.onerror;
    const oldOnLoad: typeof img.onload = img.onload;

    img.onerror = onError;
    img.onload = onLoad;
    img.src = toSysPath(uri.toString());

    // return teardown;

    function onLoad() {
      teardown();
      resolve(img);
      // subscriber.next(img);
      // subscriber.complete();
    }

    function onError(err: Error): void {
      teardown();
      reject(err);
      // subscriber.error(err);
    }

    function teardown() {
      img.onerror = oldOnError;
      img.onload = oldOnLoad;
    }
  }); // .pipe(rxOp.shareReplay(1));
}

interface Sample {
  name: string;
}

function* getSamples(): IterableIterator<Sample> {
  yield {name: "homestuck-beta-1"};
  yield {name: "squares"};
  yield {name: "triangle"};
}
