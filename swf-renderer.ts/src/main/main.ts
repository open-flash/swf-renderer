import * as fs from "fs";
import * as sysPath from "path";
import {SwfFile, TagType} from "swf-tree";
import {imageDataToPam} from "../lib/image-data-to-pam";
import {NodeCanvasRenderer} from "../lib/node-canvas/node-canvas-renderer";
import {Renderer, RendererFactory} from "../lib/renderer";
import {Shape, toSimpleShape} from "../lib/simple-shape/shape";

const testDir: string = sysPath.resolve(__dirname, "..", "..", "..", "..", "test");
const squaresAstPath: string = sysPath.resolve(testDir, "samples", "squares.ast.json");
const squaresJson: string = fs.readFileSync(squaresAstPath).toString("utf8");
const squares: SwfFile = SwfFile.type.read("json", JSON.parse(squaresJson));

async function main(rendererFactory: RendererFactory): Promise<void> {
  const width: number = Math.floor((squares.header.frameSize.xMax - squares.header.frameSize.xMin) / 20);
  const height: number = Math.floor((squares.header.frameSize.yMax - squares.header.frameSize.yMin) / 20);

  const renderer: Renderer = rendererFactory.create(width, height);

  const dictionary: Map<number, any> = new Map();
  let objects: any[] = [];

  for (const tag of squares.tags) {
    switch (tag.type) {
      case TagType.DefineShape:
        dictionary.set(tag.id, toSimpleShape(tag));
        break;
      case TagType.PlaceObject:
        objects.push(dictionary.get(tag.characterId!));
        break;
      case TagType.ShowFrame:
        renderer.clear();
        for (const obj of objects) {
          renderer.drawShape(obj);
        }
        objects = [];
        break;
      default:
        // console.warn(tag);
    }
  }

  const pam: Buffer = imageDataToPam(renderer.exportImageData());
  fs.writeFileSync("out.pam", pam);
}

main(NodeCanvasRenderer)
  .catch((err: Error): never => {
    console.error(err.stack);
    process.exit(1);
    return undefined as never;
  });
