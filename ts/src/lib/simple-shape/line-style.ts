import {StraightSRgba} from "semantic-types";

export enum LineType {
  Solid,
}

export interface SolidLine {
  readonly type: LineType.Solid;
  readonly color: Readonly<StraightSRgba<number>>;
  readonly width: number;
}

export type LineStyle = SolidLine;
