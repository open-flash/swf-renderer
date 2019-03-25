import chai from "chai";
import fs from "fs";
import { JsonReader } from "kryo/readers/json";
import sysPath from "path";
import { $DefineMorphShape, DefineMorphShape } from "swf-tree/tags";
import { decodeSwfMorphShape } from "../lib/shape/decode-swf-morph-shape";
import { MorphShape } from "../lib/shape/morph-shape";
import { Shape } from "../lib/shape/shape";
import meta from "./meta.js";

const PROJECT_ROOT: string = sysPath.join(meta.dirname, "..", "..", "..");
const TEST_SAMPLES_ROOT: string = sysPath.join(PROJECT_ROOT, "..", "tests", "morph-shape");

const JSON_READER: JsonReader = new JsonReader();
// const JSON_VALUE_WRITER: JsonValueWriter = new JsonValueWriter();

describe("decodeMorphShape", function () {
  for (const sample of getSamples()) {
    it(sample.name, async function () {
      const expectedJson: string = fs.readFileSync(
        sysPath.join(TEST_SAMPLES_ROOT, `${sample.name}.decoded.json`),
        {encoding: "UTF-8"},
      );
      const inputJson: string = fs.readFileSync(
        sysPath.join(TEST_SAMPLES_ROOT, `${sample.name}.ast.json`),
        {encoding: "UTF-8"},
      );
      const input: DefineMorphShape = $DefineMorphShape.read(JSON_READER, inputJson);
      const expected: Shape = JSON.parse(expectedJson);
      const actual: MorphShape = decodeSwfMorphShape(input);
      chai.assert.strictEqual(
        JSON.stringify(actual, null, 2),
        JSON.stringify(expected, null, 2),
      );
    });
  }
});

interface Sample {
  name: string;
}

function* getSamples(): IterableIterator<Sample> {
  yield {name: "homestuck-beta-29"};
}
