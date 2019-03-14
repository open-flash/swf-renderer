import { StraightSRgba8 } from "swf-tree/straight-s-rgba8";
import { DisplayObject } from "./display-object";

/**
 * A stage represent a drawing area. It holds the root of its display tree.
 */
export interface Stage {
  backgroundColor?: StraightSRgba8;
  /**
   * Size in pixels
   */
  width: number;
  /**
   * Size in pixels
   */
  height: number;
  children: DisplayObject[];
}
