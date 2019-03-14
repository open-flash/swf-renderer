import { Matrix } from "swf-tree/matrix";
import { DefineShape } from "swf-tree/tags";
import { DisplayObjectType } from "./display-object-type";

export interface Shape {
  readonly type: DisplayObjectType.Shape;
  readonly definition: DefineShape;
  matrix?: Matrix;
}
