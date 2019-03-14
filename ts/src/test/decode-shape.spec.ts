import chai from "chai";
import fs from "fs";
import { JsonReader } from "kryo/readers/json";
import sysPath from "path";
import { $DefineShape, DefineShape } from "swf-tree/tags";
import { decodeSwfShape } from "../lib/shape/decode-swf-shape";
import { Shape } from "../lib/shape/shape";
import meta from "./meta.js";

const PROJECT_ROOT: string = sysPath.join(meta.dirname, "..", "..", "..");
const TEST_SAMPLES_ROOT: string = sysPath.join(PROJECT_ROOT, "..", "tests");

const JSON_READER: JsonReader = new JsonReader();
// const JSON_VALUE_WRITER: JsonValueWriter = new JsonValueWriter();

describe("decodeShape", function () {
  for (const sample of getSamples()) {
    it(sample.name, async function () {
      const expectedJson: string = fs.readFileSync(
        sysPath.join(TEST_SAMPLES_ROOT, "decode-shape", `${sample.name}.decoded.json`),
        {encoding: "UTF-8"},
      );
      const inputJson: string = fs.readFileSync(
        sysPath.join(TEST_SAMPLES_ROOT, "decode-shape", `${sample.name}.ast.json`),
        {encoding: "UTF-8"},
      );
      const input: DefineShape = $DefineShape.read(JSON_READER, inputJson);
      const expected: Shape = JSON.parse(expectedJson);
      const actual: Shape = decodeSwfShape(input);
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
  yield {name: "homestuck-beta-1"};
  yield {name: "squares"};
  yield {name: "triangle"};
}
