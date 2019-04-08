import chai from "chai";
import { JsonReader } from "kryo/readers/json";
import sysPath from "path";
import { $DefineShape, DefineShape } from "swf-tree/tags";
import { decodeSwfShape } from "../lib/shape/decode-swf-shape";
import { Shape } from "../lib/shape/shape";
import { readTextFile, TEST_SAMPLES_ROOT, writeTextFile } from "./utils";

const JSON_READER: JsonReader = new JsonReader();

describe("decodeShape", function () {
  for (const sample of getSamples()) {
    it(sample.name, async function () {
      const inputJson: string = await readTextFile(sysPath.join(TEST_SAMPLES_ROOT, sample.name, "ast.json"));
      const input: DefineShape = $DefineShape.read(JSON_READER, inputJson);

      const actual: Shape = decodeSwfShape(input);
      const actualJson: string = `${JSON.stringify(actual, null, 2)}\n`;
      await writeTextFile(sysPath.join(TEST_SAMPLES_ROOT, sample.name, "tmp-shape.ts.json"), actualJson);

      const expectedJson: string = await readTextFile(sysPath.join(TEST_SAMPLES_ROOT, sample.name, "shape.ts.json"));
      chai.assert.strictEqual(actualJson, expectedJson);
    });
  }
});

interface Sample {
  name: string;
}

function* getSamples(): IterableIterator<Sample> {
  yield {name: "flat-shapes/homestuck-beta-1"};
  yield {name: "flat-shapes/squares"};
  yield {name: "flat-shapes/triangle"};
  yield {name: "textured-shapes/homestuck-beta-4"};
}
