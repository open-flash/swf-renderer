import chai from "chai";
import { JsonReader } from "kryo/readers/json";
import sysPath from "path";
import { $DefineMorphShape, DefineMorphShape } from "swf-tree/tags";
import { decodeSwfMorphShape } from "../lib/shape/decode-swf-morph-shape";
import { MorphShape } from "../lib/shape/morph-shape";
import { readTextFile, TEST_SAMPLES_ROOT, writeTextFile } from "./utils";

const JSON_READER: JsonReader = new JsonReader();

describe("decodeMorphShape", function () {
  for (const sample of getSamples()) {
    it(sample.name, async function () {
      const inputJson: string = await readTextFile(sysPath.join(TEST_SAMPLES_ROOT, sample.name, "ast.json"));
      const input: DefineMorphShape = $DefineMorphShape.read(JSON_READER, inputJson);

      const actual: MorphShape = decodeSwfMorphShape(input);
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
  yield {name: "flat-morph-shapes/homestuck-beta-29"};
}
