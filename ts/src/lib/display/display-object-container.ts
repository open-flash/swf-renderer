import { Matrix } from "swf-tree";
import { DisplayObject } from "./display-object";
import { DisplayObjectType } from "./display-object-type";

export interface DisplayObjectContainer {
  readonly type: DisplayObjectType.Container;
  readonly children: ReadonlyArray<DisplayObject>;
  matrix?: Matrix;
}
