import { DefineBitmap } from "swf-tree/tags";
import { Stage } from "./display/stage";

export interface Renderer {
  render(stage: Stage): void;

  addBitmap(tag: DefineBitmap): Promise<void>;
}
