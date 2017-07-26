import {StraightSRgba} from "semantic-types";

export enum FillType {
  Solid,
}

export interface SolidFill {
  readonly type: FillType.Solid;
  readonly color: Readonly<StraightSRgba<number>>;
}

export type FillStyle = SolidFill;
