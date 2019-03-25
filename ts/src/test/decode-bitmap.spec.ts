import canvas from "canvas";
import chai from "chai";
import fs from "fs";
import { fromSysPath, join } from "furi";
import { JsonReader } from "kryo/readers/json";
import sysPath from "path";
import { $DefineBitmap, DefineBitmap } from "swf-tree/tags";
import { decodeXSwfBmpSync } from "../lib/decode-x-swf-bmp";
import { imageDataToPam } from "../lib/image-data-to-pam";
import meta from "./meta.js";

const PROJECT_ROOT: string = sysPath.join(meta.dirname, "..", "..", "..");
const TEST_SAMPLES_ROOT: string = sysPath.join(PROJECT_ROOT, "..", "tests", "bitmap");

const JSON_READER: JsonReader = new JsonReader();
// const JSON_VALUE_WRITER: JsonValueWriter = new JsonValueWriter();

describe("decodeBitmap", function () {
  for (const sample of getSamples()) {
    it(sample.name, async function () {
      const expectedPam: Buffer = fs.readFileSync(
        sysPath.join(TEST_SAMPLES_ROOT, `${sample.name}.pam`),
        {encoding: null},
      );
      const inputJson: string = fs.readFileSync(
        sysPath.join(TEST_SAMPLES_ROOT, `${sample.name}.ast.json`),
        {encoding: "UTF-8"},
      );
      const input: DefineBitmap = $DefineBitmap.read(JSON_READER, inputJson);

      const decoded: canvas.ImageData = decodeXSwfBmpSync(input.data);
      const actualPam: Buffer = imageDataToPam(decoded);

      await writeFile(join(fromSysPath(TEST_SAMPLES_ROOT), [`${sample.name}.ts-out.pam`]), actualPam);

      chai.assert.deepEqual(actualPam, expectedPam);
    });
  }
});

interface Sample {
  name: string;
}

function* getSamples(): IterableIterator<Sample> {
  yield {name: "homestuck-beta-3"};
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
