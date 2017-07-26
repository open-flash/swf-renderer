import {StraightSRgba, UintSize} from "semantic-types";
import {shapes, StraightSRgba8, tags} from "swf-tree";
import {FillStyle, FillType} from "./fill-style";
import {LineStyle, LineType} from "./line-style";
import {Command, CommandType, PathWithStyle} from "./path";

enum SegmentType {
  Straight,
  Curved,
}

interface StraightSegment {
  type: SegmentType.Straight;
  startX: number;
  startY: number;
  endX: number;
  endY: number;
}

function createStraightSegment(startX: number, startY: number, endX: number, endY: number): StraightSegment {
  return {type: SegmentType.Straight, startX, startY, endX, endY};
}

interface CurvedSegment {
  type: SegmentType.Curved;
  startX: number;
  startY: number;
  controlX: number;
  controlY: number;
  endX: number;
  endY: number;
}

function createCurvedSegment(
  startX: number,
  startY: number,
  controlX: number,
  controlY: number,
  endX: number,
  endY: number,
): CurvedSegment {
  return {type: SegmentType.Curved, startX, startY, controlX, controlY, endX, endY};
}

/**
 * Represents the geometry of the path part produced by a single CurvedEdge or StraightEdge record.
 */
type Segment = StraightSegment | CurvedSegment;

// TODO: Move this to another file
export interface Shape {
  paths: PathWithStyle[];
}

function normalizeStraightSRgba(color: StraightSRgba8): Readonly<StraightSRgba<number>> {
  return {
    r: color.r / 255,
    g: color.g / 255,
    b: color.b / 255,
    a: color.a / 255,
  };
}

function convertFillStyle(old: shapes.FillStyle): FillStyle {
  switch (old.type) {
    case shapes.FillStyleType.Solid:
      return {type: FillType.Solid, color: normalizeStraightSRgba(old.color)};
    default:
      console.warn(old);
      throw new Error("Unknown fill type");
  }
}

function convertLineStyle(old: shapes.LineStyle): LineStyle {
  // TODO...
  return {type: LineType.Solid, color: {r: 0, g: 0, b: 0, a: 1}, width: 50};
}

// const defaultFillStyle: FillStyle = {type: FillType.Solid, color: "transparent"};

/**
 * For a given fill style, the corresponding segments in their order of definition.
 */
interface FillSegmentSet {
  readonly style: FillStyle;
  readonly segments: Segment[];
}

/**
 * For a given line style, the corresponding segments in their order of definition.
 */
interface LineSegmentSet {
  readonly style: LineStyle;
  readonly segments: Segment[];
}

/**
 * Each change of style list creates a new records layer.
 * For each record group, the fills are rendered first, and then the strokes.
 */
interface StyleLayer {
  readonly fills: FillSegmentSet[];
  readonly lines: LineSegmentSet[];
}

/**
 * Create a new layer with the supplied styles.
 */
function createStyleLayer(swfFillStyles: shapes.FillStyle[], swfLineStyles: shapes.LineStyle[]): StyleLayer {
  const fills: FillSegmentSet[] = [];
  for (const swfFillStyle of swfFillStyles) {
    fills.push({
      style: convertFillStyle(swfFillStyle),
      segments: [],
    });
  }
  const lines: LineSegmentSet[] = [];
  for (const swfLineStyle of swfLineStyles) {
    lines.push({
      style: convertLineStyle(swfLineStyle),
      segments: [],
    });
  }
  return {fills, lines};
}

/**
 * Extract a sequence of continuous segments, removes them from openSet.
 */
function extractContinuous(openSet: Segment[]): Segment[] {
  const first: Segment | undefined = openSet.shift();
  if (first === undefined) {
    throw new Error("FailedAssertion: openSet should not be empty");
  }
  const result: Segment[] = [first];
  let startX: UintSize = first.startX;
  let startY: UintSize = first.startY;
  let endX: UintSize = first.endX;
  let endY: UintSize = first.endY;
  for (let i: number = 0, len: number = openSet.length; i < len; i++) {
    const current: Segment = openSet[i];
    if (current.startX === endX && current.startY === endY) {
      openSet.splice(i, 1);
      i--;
      len--;
      endX = current.endX;
      endY = current.endY;
      result.push(current);
    } else if (current.endX === startX && current.endY === startY) {
      openSet.splice(i, 1);
      i--;
      len--;
      startX = current.startX;
      startY = current.startY;
      result.unshift(current);
    }
  }
  // TODO: Repeat until reaching fixed point ?
  return result;
}

/**
 * Converts a list of segments (in their definition order) to a list of path commands.
 */
function segmentsToCommands(segments: Segment[]): Command[] {
  const openSet: Segment[] = segments.map((x) => x);
  const result: Command[] = [];
  while (openSet.length > 0) {
    const sequence: Segment[] = extractContinuous(openSet);
    result.push({
      type: CommandType.MoveTo,
      x: sequence[0].startX,
      y: sequence[0].startY,
    });
    for (const segment of sequence) {
      switch (segment.type) {
        case SegmentType.Straight:
          result.push({
            type: CommandType.LineTo,
            endX: segment.endX,
            endY: segment.endY,
          });
          break;
        case SegmentType.Curved:
          result.push({
            type: CommandType.CurveTo,
            controlX: segment.controlX,
            controlY: segment.controlY,
            endX: segment.endX,
            endY: segment.endY,
          });
          break;
        default:
          throw new Error("Failed assertion: unexpected segment type");
      }
    }
  }
  return result;
}

/**
 * Converts a layer to a list of paths with style
 */
function layerToPaths(layer: StyleLayer): PathWithStyle[] {
  const paths: PathWithStyle[] = [];
  for (const fillSet of layer.fills) {
    const commands: Command[] = segmentsToCommands(fillSet.segments);
    if (commands.length > 0) {
      paths.push({commands, fill: fillSet.style});
    }
  }
  for (const lineSet of layer.lines) {
    const commands: Command[] = segmentsToCommands(lineSet.segments);
    if (commands.length > 0) {
      paths.push({commands, line: lineSet.style});
    }
  }
  return paths;
}

class SwfShapeConverter {
  /**
   * Each definition of new styles creates a layer.
   */
  private layers: StyleLayer[];

  /**
   * Current style set for the left fill (fill0).
   */
  private leftFill: FillSegmentSet | undefined;
  /**
   * Current style set for the right fill (fill1).
   */
  private rightFill: FillSegmentSet | undefined;
  /**
   * Current style set for the line fill (line).
   */
  private lineFill: LineSegmentSet | undefined;

  /**
   * Current X position
   */
  private x: number;

  /**
   * Current Y position
   */
  private y: number;

  constructor(swfFillStyles: shapes.FillStyle[], swfLineStyles: shapes.LineStyle[]) {
    this.x = 0;
    this.y = 0;
    this.layers = [];
    this.leftFill = undefined;
    this.rightFill = undefined;
    this.lineFill = undefined;
    this.setNewStyles(swfFillStyles, swfLineStyles);
  }

  applyStyleChange(record: shapes.records.StyleChange): void {
    if (record.leftFill !== undefined) {
      this.setLeftFillById(record.leftFill);
    }
    if (record.rightFill !== undefined) {
      this.setRightFillById(record.rightFill);
    }
    // In fact, it's an absolute moveTo (fixed in SWF tree 0.0.4)
    if (record.deltaX !== 0 || record.deltaY !== 0) {
      this.x = record.deltaX || 0;
      this.y = record.deltaY || 0;
    }
  }

  applyStraightEdge(record: shapes.records.StraightEdge): void {
    const endX: number = this.x + record.deltaX;
    const endY: number = this.y + record.deltaY;

    if (this.leftFill !== undefined) {
      this.leftFill.segments.push(createStraightSegment(this.x, this.y, endX, endY));
    }
    if (this.rightFill !== undefined) {
      this.rightFill.segments.push(createStraightSegment(endX, endY, this.x, this.y));
    }
    if (this.lineFill !== undefined) {
      this.lineFill.segments.push(createStraightSegment(this.x, this.y, endX, endY));
    }

    this.x = endX;
    this.y = endY;
  }

  applyCurvedEdge(record: shapes.records.CurvedEdge): void {
    const controlX: number = this.x + record.controlX;
    const controlY: number = this.y + record.controlY;
    const endX: number = this.x + record.deltaX;
    const endY: number = this.y + record.deltaY;

    if (this.leftFill !== undefined) {
      this.leftFill.segments.push(createCurvedSegment(this.x, this.y, controlX, controlY, endX, endY));
    }
    if (this.rightFill !== undefined) {
      this.rightFill.segments.push(createCurvedSegment(endX, endY, controlX, controlY, this.x, this.y));
    }
    if (this.lineFill !== undefined) {
      this.lineFill.segments.push(createCurvedSegment(this.x, this.y, controlX, controlY, endX, endY));
    }

    this.x = endX;
    this.y = endY;
  }

  getShape(): Shape {
    const paths: PathWithStyle[] = [];
    for (const layer of this.layers) {
      for (const path of layerToPaths(layer)) {
        paths.push(path);
      }
    }
    return {paths};
  }

  private setNewStyles(swfFillStyles: shapes.FillStyle[], swfLineStyles: shapes.LineStyle[]): void {
    const layer: StyleLayer = createStyleLayer(swfFillStyles, swfLineStyles);
    this.layers.push(layer);
    this.leftFill = undefined;
    this.rightFill = undefined;
    this.lineFill = undefined;
  }

  private setLeftFillById(fillId: UintSize): void {
    if (fillId === 0) {
      this.leftFill = undefined;
      return;
    }
    const currentLayer: StyleLayer = this.layers[this.layers.length - 1];
    const fillIndex: UintSize = fillId - 1;
    this.leftFill = currentLayer.fills[fillIndex];
    if (this.leftFill === undefined) {
      throw new Error("Invalid fill ID");
    }
  }

  private setRightFillById(fillId: UintSize): void {
    if (fillId === 0) {
      this.rightFill = undefined;
      return;
    }
    const currentLayer: StyleLayer = this.layers[this.layers.length - 1];
    const fillIndex: UintSize = fillId - 1;
    this.rightFill = currentLayer.fills[fillIndex];
    if (this.rightFill === undefined) {
      throw new Error("Invalid fill ID");
    }
  }

  private setLineFillById(fillId: UintSize): void {
    if (fillId === 0) {
      this.lineFill = undefined;
      return;
    }
    const currentLayer: StyleLayer = this.layers[this.layers.length - 1];
    const fillIndex: UintSize = fillId - 1;
    this.lineFill = currentLayer.lines[fillIndex];
    if (this.lineFill === undefined) {
      throw new Error("Invalid fill ID");
    }
  }
}

export function toSimpleShape(tag: tags.DefineShape): Shape {
  // console.log(tag.shape.records);
  const converter: SwfShapeConverter = new SwfShapeConverter(tag.shape.fillStyles, tag.shape.lineStyles);

  for (const record of tag.shape.records) {
    switch (record.type) {
      case shapes.ShapeRecordType.CurvedEdge:
        converter.applyCurvedEdge(record);
        break;
      case shapes.ShapeRecordType.StraightEdge:
        converter.applyStraightEdge(record);
        break;
      case shapes.ShapeRecordType.StyleChange:
        converter.applyStyleChange(record);
        break;
    }
  }

  return converter.getShape();
}
