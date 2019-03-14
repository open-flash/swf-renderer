import { DisplayObjectContainer } from "./display-object-container";
import { MorphShape } from "./morph-shape";
import { Shape } from "./shape";

export type DisplayObject = DisplayObjectContainer | MorphShape | Shape;
