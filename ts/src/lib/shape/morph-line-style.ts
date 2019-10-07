import { MorphFillStyle } from "./morph-fill-style";

export interface MorphLineStyle {
  readonly fill: MorphFillStyle;
  readonly width: [number, number];
}
