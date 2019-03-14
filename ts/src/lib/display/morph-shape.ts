import { Matrix } from "swf-tree/matrix";
import { DefineMorphShape } from "swf-tree/tags";
import { DisplayObjectType } from "./display-object-type";

export interface MorphShape {
  readonly type: DisplayObjectType.MorphShape;
  readonly definition: DefineMorphShape;
  matrix?: Matrix;
  ratio: number;
}
