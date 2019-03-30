/**
 * Copyright 2014 Mozilla Foundation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/// <reference path='references.ts'/>
module Shumway.SWF.Parser {
  import PathCommand = Shumway.PathCommand;
  import GradientType = Shumway.GradientType;
  import GradientSpreadMethod = Shumway.GradientSpreadMethod;
  import GradientInterpolationMethod = Shumway.GradientInterpolationMethod;
  import Bounds = Shumway.Bounds;
  import DataBuffer = Shumway.ArrayUtilities.DataBuffer;
  import ShapeData = Shumway.ShapeData;
  import ShapeMatrix = Shumway.ShapeMatrix;
  import clamp = Shumway.NumberUtilities.clamp;
  import assert = Shumway.Debug.assert;
  import assertUnreachable = Shumway.Debug.assertUnreachable;
  let push = Array.prototype.push;

  enum FillType {
    Solid = 0,
    LinearGradient = 0x10,
    RadialGradient = 0x12,
    FocalRadialGradient = 0x13,
    RepeatingBitmap = 0x40,
    ClippedBitmap = 0x41,
    NonsmoothedRepeatingBitmap = 0x42,
    NonsmoothedClippedBitmap = 0x43,
  }

  /*
   * Applies the current segment to the paths of all styles specified in the last
   * style-change record.
   *
   * For fill0, we have to apply commands and their data in reverse order, to turn
   * left fills into right ones.
   *
   * If we have more than one style, we only recorded commands for the first one
   * and have to duplicate them for the other styles. The order is: fill1, line,
   * fill0. (That means we only ever recorded into fill0 if that's the only style.)
   */
  function applySegmentToStyles(segment: PathSegment, styles: any,
                                linePaths: SegmentedPath[], fillPaths: SegmentedPath[]) {
    if (!segment) {
      return;
    }
    let path: SegmentedPath;
    if (styles.fill0) {
      path = fillPaths[styles.fill0 - 1];
      // If fill0 is the only style, we have pushed the segment to its stack. In
      // that case, just mark it as reversed and move on.
      if (!(styles.fill1 || styles.line)) {
        segment.isReversed = true;
        return;
      } else {
        path.addSegment(segment.toReversed());
      }
    }
    if (styles.line && styles.fill1) {
      path = linePaths[styles.line - 1];
      path.addSegment(segment.clone());
    }
  }

  /*
   * Converts records from the space-optimized format they're stored in to a
   * format that's more amenable to fast rendering.
   *
   * See http://blogs.msdn.com/b/mswanson/archive/2006/02/27/539749.aspx and
   * http://wahlers.com.br/claus/blog/hacking-swf-1-shapes-in-flash/ for details.
   */
  function convertRecordsToShapeData(records: ShapeRecord[], fillPaths: SegmentedPath[],
                                     linePaths: SegmentedPath[], dependencies: number[],
                                     recordsMorph: ShapeRecord[]): ShapeData {
    clearShared();

    let isMorph = recordsMorph !== null;
    let styles = {fill0: 0, fill1: 0, line: 0};
    let segment: PathSegment = null;

    // Fill- and line styles can be added by style change records in the middle of
    // a shape records list. This also causes the previous paths to be treated as
    // a group, so the lines don't get moved on top of any following fills.
    // To support this, we just append all current fill and line paths to a list
    // when new styles are introduced.
    let allPaths: SegmentedPath[];
    // If no style is set for a segment of a path, a 1px transparent line is used.
    let defaultPath: SegmentedPath;

    let numRecords = records.length;
    let x: number = 0;
    let y: number = 0;
    let morphX: number = 0;
    let morphY: number = 0;
    let path: SegmentedPath;
    for (let i = 0, j = 0; i < numRecords; i++) {
      let record = records[i];
      let morphRecord: ShapeRecord;
      if (isMorph) {
        morphRecord = recordsMorph[j++];
      }
      // type 0 is a StyleChange record
      if (record.type === 0) {
        //TODO: make the `has*` fields bitflags
        if (segment) {
          applySegmentToStyles(segment, styles, linePaths, fillPaths);
        }

        if (record.flags & ShapeRecordFlags.HasNewStyles) {
          if (!allPaths) {
            allPaths = [];
          }
          push.apply(allPaths, fillPaths);
          fillPaths = createPathsList(record.fillStyles, false, isMorph, dependencies);
          push.apply(allPaths, linePaths);
          linePaths = createPathsList(record.lineStyles, true, isMorph, dependencies);
          if (defaultPath) {
            allPaths.push(defaultPath);
            defaultPath = null;
          }
          styles = {fill0: 0, fill1: 0, line: 0};
        }

        if ((record.flags & ShapeRecordFlags.HasFillStyle0) !== 0) {
          styles.fill0 = record.fillStyle0;
        }
        if ((record.flags & ShapeRecordFlags.HasFillStyle1) !== 0) {
          styles.fill1 = record.fillStyle1;
        }
        if ((record.flags & ShapeRecordFlags.HasLineStyle) !== 0) {
          styles.line = record.lineStyle;
        }
        if (styles.fill1) {
          path = fillPaths[styles.fill1 - 1];
        } else if (styles.line) {
          path = linePaths[styles.line - 1];
        } else if (styles.fill0) {
          path = fillPaths[styles.fill0 - 1];
        }

        if (record.flags & ShapeRecordFlags.Move) {
          x = record.moveX | 0;
          y = record.moveY | 0;
          // When morphed, StyleChangeRecords/MoveTo might not have a
          // corresponding record in the start or end shape --
          // processing morphRecord below before converting type 1 records.
        }

        // Very first record can be just fill/line-style definition record.
        if (path) {
          segment = PathSegment.FromDefaults(isMorph);
          path.addSegment(segment);

          // Move or not, we want this path segment to start where the last one
          // left off. Even if the last one belonged to a different style.
          // "Huh," you say? Yup.
          if (!isMorph) {
            segment.moveTo(x, y);
          } else {
            if (morphRecord.type === 0) {
              morphX = morphRecord.moveX | 0;
              morphY = morphRecord.moveY | 0;
            } else {
              morphX = x;
              morphY = y;
              // Not all moveTos are reflected in morph data.
              // In that case, decrease morph data index.
              j--;
            }
            segment.morphMoveTo(x, y, morphX, morphY);
          }
        }
      }
      // type 1 is a StraightEdge or CurvedEdge record
      else {
        release || assert(record.type === 1);
        if (!segment) {
          if (!defaultPath) {
            let style = {color: {red: 0, green: 0, blue: 0, alpha: 0}, width: 20};
            defaultPath = new SegmentedPath(null, processStyle(style, true, isMorph, dependencies));
          }
          segment = PathSegment.FromDefaults(isMorph);
          defaultPath.addSegment(segment);
          if (!isMorph) {
            segment.moveTo(x, y);
          } else {
            segment.morphMoveTo(x, y, morphX, morphY);
          }
        }
        if (isMorph) {
          // An invalid SWF might contain a move in the EndEdges list where the
          // StartEdges list contains an edge. The Flash Player seems to skip it,
          // so we do, too.
          while (morphRecord && morphRecord.type === 0) {
            morphRecord = recordsMorph[j++];
          }
          // The EndEdges list might be shorter than the StartEdges list. Reuse
          // start edges as end edges in that case.
          if (!morphRecord) {
            morphRecord = record;
          }
        }

        if (record.flags & ShapeRecordFlags.IsStraight &&
          (!isMorph || (morphRecord.flags & ShapeRecordFlags.IsStraight) !== 0)) {
          x += record.deltaX | 0;
          y += record.deltaY | 0;
          if (!isMorph) {
            segment.lineTo(x, y);
          } else {
            morphX += morphRecord.deltaX | 0;
            morphY += morphRecord.deltaY | 0;
            segment.morphLineTo(x, y, morphX, morphY);
          }
        } else {
          let cx, cy;
          let deltaX, deltaY;
          if ((record.flags & ShapeRecordFlags.IsStraight) === 0) {
            cx = x + record.controlDeltaX | 0;
            cy = y + record.controlDeltaY | 0;
            x = cx + record.anchorDeltaX | 0;
            y = cy + record.anchorDeltaY | 0;
          } else {
            deltaX = record.deltaX | 0;
            deltaY = record.deltaY | 0;
            cx = x + (deltaX >> 1);
            cy = y + (deltaY >> 1);
            x += deltaX;
            y += deltaY;
          }
          if (!isMorph) {
            segment.curveTo(cx, cy, x, y);
          } else {
            let morphCX, morphCY;
            if ((morphRecord.flags & ShapeRecordFlags.IsStraight) === 0) {
              morphCX = morphX + morphRecord.controlDeltaX | 0;
              morphCY = morphY + morphRecord.controlDeltaY | 0;
              morphX = morphCX + morphRecord.anchorDeltaX | 0;
              morphY = morphCY + morphRecord.anchorDeltaY | 0;
            } else {
              deltaX = morphRecord.deltaX | 0;
              deltaY = morphRecord.deltaY | 0;
              morphCX = morphX + (deltaX >> 1);
              morphCY = morphY + (deltaY >> 1);
              morphX += deltaX;
              morphY += deltaY;
            }
            segment.morphCurveTo(cx, cy, x, y, morphCX, morphCY, morphX, morphY);
          }
        }
      }
    }
    applySegmentToStyles(segment, styles, linePaths, fillPaths);

    // All current paths get appended to the allPaths list at the end. First fill,
    // then line paths.
    if (allPaths) {
      push.apply(allPaths, fillPaths);
    } else {
      allPaths = fillPaths;
    }
    push.apply(allPaths, linePaths);
    if (defaultPath) {
      allPaths.push(defaultPath);
    }

    let shape: ShapeData = new ShapeData();
    if (isMorph) {
      shape.morphCoordinates = new Int32Array(shape.coordinates.length);
      shape.morphStyles = new DataBuffer(16);
    }
    for (let i = 0; i < allPaths.length; i++) {
      allPaths[i].serialize(shape);
    }
    return shape;
  }

  interface ShapeStyle {
    type: number;

    fillType?: number;
    width?: number;
    pixelHinting?: boolean;
    noHscale?: boolean;
    noVscale?: boolean;
    endCapsStyle?: number;
    jointStyle?: number;
    miterLimit?: number;

    color?: number;

    transform?: ShapeMatrix;
    colors?: number[];
    ratios?: number[];
    spreadMethod?: number;
    interpolationMode?: number;
    focalPoint?: number;
    bitmapId?: number;
    bitmapIndex?: number;
    repeat?: boolean;
    smooth?: boolean;

    morph: ShapeStyle
  }

  let IDENTITY_MATRIX: ShapeMatrix = {a: 1, b: 0, c: 0, d: 1, tx: 0, ty: 0};

  function processStyle(style: any, isLineStyle: boolean, isMorph: boolean,
                        dependencies: number[]): ShapeStyle {
    let shapeStyle: ShapeStyle = style;
    if (isMorph) {
      shapeStyle.morph = processMorphStyle(style, isLineStyle, dependencies);
    }
    if (isLineStyle) {
      shapeStyle.miterLimit = (style.miterLimitFactor || 1.5) * 2;
      if (!style.color && style.hasFill) {
        let fillStyle = processStyle(style.fillStyle, false, false, dependencies);
        shapeStyle.type = fillStyle.type;
        shapeStyle.transform = fillStyle.transform;
        shapeStyle.colors = fillStyle.colors;
        shapeStyle.ratios = fillStyle.ratios;
        shapeStyle.focalPoint = fillStyle.focalPoint;
        shapeStyle.bitmapId = fillStyle.bitmapId;
        shapeStyle.bitmapIndex = fillStyle.bitmapIndex;
        shapeStyle.repeat = fillStyle.repeat;
        style.fillStyle = null;
        return shapeStyle;
      } else {
        shapeStyle.type = FillType.Solid;
        return shapeStyle;
      }
    }
    if (style.type === undefined || style.type === FillType.Solid) {
      return shapeStyle;
    }
    let scale;
    switch (style.type) {
      case FillType.LinearGradient:
      case FillType.RadialGradient:
      case FillType.FocalRadialGradient:
        let records = style.records;
        let colors: Array<any> = shapeStyle.colors = [];
        let ratios: Array<any> = shapeStyle.ratios = [];
        for (let i = 0; i < records.length; i++) {
          let record = records[i];
          colors.push(record.color);
          ratios.push(record.ratio);
        }
        scale = 819.2;
        break;
      case FillType.RepeatingBitmap:
      case FillType.ClippedBitmap:
      case FillType.NonsmoothedRepeatingBitmap:
      case FillType.NonsmoothedClippedBitmap:
        shapeStyle.smooth = style.type !== FillType.NonsmoothedRepeatingBitmap &&
          style.type !== FillType.NonsmoothedClippedBitmap;
        shapeStyle.repeat = style.type !== FillType.ClippedBitmap &&
          style.type !== FillType.NonsmoothedClippedBitmap;
        let index = dependencies.indexOf(style.bitmapId);
        if (index === -1) {
          index = dependencies.length;
          dependencies.push(style.bitmapId);
        }
        shapeStyle.bitmapIndex = index;
        scale = 0.05;
        break;
      default:
        Debug.warning('shape parser encountered invalid fill style ' + style.type);
    }
    if (!style.matrix) {
      shapeStyle.transform = IDENTITY_MATRIX;
      return shapeStyle;
    }
    let matrix = style.matrix;
    shapeStyle.transform = {
      a: (matrix.a * scale),
      b: (matrix.b * scale),
      c: (matrix.c * scale),
      d: (matrix.d * scale),
      tx: matrix.tx / 20,
      ty: matrix.ty / 20
    };
    // null data that's unused from here on out
    style.matrix = null;
    return shapeStyle;
  }

  function processMorphStyle(style: any, isLineStyle: boolean, dependencies: number[]): ShapeStyle {
    let morphStyle: ShapeStyle = Object.create(style);
    if (isLineStyle) {
      morphStyle.width = style.widthMorph;
      if (!style.color && style.hasFill) {
        let fillStyle = processMorphStyle(style.fillStyle, false, dependencies);
        morphStyle.transform = fillStyle.transform;
        morphStyle.colors = fillStyle.colors;
        morphStyle.ratios = fillStyle.ratios;
        return morphStyle;
      } else {
        morphStyle.color = style.colorMorph;
        return morphStyle;
      }
    }
    if (style.type === undefined) {
      return morphStyle;
    }
    if (style.type === FillType.Solid) {
      morphStyle.color = style.colorMorph;
      return morphStyle;
    }
    let scale;
    switch (style.type) {
      case FillType.LinearGradient:
      case FillType.RadialGradient:
      case FillType.FocalRadialGradient:
        let records = style.records;
        let colors: Array<any> = morphStyle.colors = [];
        let ratios: Array<any> = morphStyle.ratios = [];
        for (let i = 0; i < records.length; i++) {
          let record = records[i];
          colors.push(record.colorMorph);
          ratios.push(record.ratioMorph);
        }
        scale = 819.2;
        break;
      case FillType.RepeatingBitmap:
      case FillType.ClippedBitmap:
      case FillType.NonsmoothedRepeatingBitmap:
      case FillType.NonsmoothedClippedBitmap:
        scale = 0.05;
        break;
      default:
        release || assertUnreachable('shape parser encountered invalid fill style');
    }
    if (!style.matrix) {
      morphStyle.transform = IDENTITY_MATRIX;
      return morphStyle;
    }
    let matrix = style.matrixMorph;
    morphStyle.transform = {
      a: (matrix.a * scale),
      b: (matrix.b * scale),
      c: (matrix.c * scale),
      d: (matrix.d * scale),
      tx: matrix.tx / 20,
      ty: matrix.ty / 20
    };
    return morphStyle;
  }

  /*
   * Paths are stored in 2-dimensional arrays. Each of the inner arrays contains
   * all the paths for a certain fill or line style.
   */
  function createPathsList(styles: any[], isLineStyle: boolean, isMorph: boolean,
                           dependencies: number[]): SegmentedPath[] {
    let paths: SegmentedPath[] = [];
    for (let i = 0; i < styles.length; i++) {
      let style = processStyle(styles[i], isLineStyle, isMorph, dependencies);
      if (!isLineStyle) {
        paths[i] = new SegmentedPath(style, null);
      } else {
        paths[i] = new SegmentedPath(null, style);
      }
    }
    return paths;
  }

  export function defineShape(tag: ShapeTag) {
    let dependencies: Array<any> = [];
    let fillPaths = createPathsList(tag.fillStyles, false, !!tag.recordsMorph, dependencies);
    let linePaths = createPathsList(tag.lineStyles, true, !!tag.recordsMorph, dependencies);
    let shape = convertRecordsToShapeData(tag.records, fillPaths, linePaths,
      dependencies, tag.recordsMorph || null);
    return {
      type: tag.flags & ShapeFlags.IsMorph ? 'morphshape' : 'shape',
      id: tag.id,
      fillBounds: tag.fillBounds,
      lineBounds: tag.lineBounds,
      morphFillBounds: tag.fillBoundsMorph || null,
      morphLineBounds: tag.lineBoundsMorph || null,
      shape: shape.toPlainObject(),
      transferables: shape.buffers,
      require: dependencies.length ? dependencies : null
    };
  }

  let sharedCommands: DataBuffer = null;
  let sharedData: DataBuffer = null;
  let sharedMorphData: DataBuffer = null;
  let segmentsPool: Array<PathSegment> = [];
  let segmentsCounter: number = 0;

  function clearShared() {
    if (!sharedCommands) {
      sharedCommands = new DataBuffer();
      sharedData = new DataBuffer();
      sharedMorphData = new DataBuffer();
      sharedCommands.endian = sharedData.endian = sharedMorphData.endian = 'auto';
    }

    sharedCommands.clear();
    sharedData.clear();
    sharedMorphData.clear();
    segmentsCounter = 0;
  }

  class PathSegment {
    // public startPoint: string;
    // public endPoint: string;
    public startPoint: number;
    public endPoint: number;
    public flag: boolean;

    commandsStart: number;
    commandsEnd: number;
    dataStart: number;
    dataEnd: number;
    morphDataStart: number;
    morphDataEnd: number;

    prev: PathSegment;
    next: PathSegment;
    isMorph: boolean;
    isReversed: boolean;

    reset() {
      this.commandsStart = sharedCommands.position;
      this.commandsEnd = sharedCommands.position;
      this.dataStart = sharedData.position;
      this.dataEnd = sharedData.position;
      this.morphDataStart = sharedMorphData.position;
      this.morphDataEnd = sharedMorphData.position;

      this.isMorph = false;
      this.isReversed = false;
      this.prev = null;
      this.next = null;

      this.startPoint = 0;
      this.endPoint = 0;
      this.flag = false;
    }

    constructor() {
      this.reset();
    }

    static FromDefaults(isMorph: boolean) {
      let segment = segmentsPool[segmentsCounter];
      if (!segment) {
        segment = new PathSegment();
        segmentsPool.push(segment);
      } else {
        segment.reset();
      }
      segmentsCounter++;

      segment.isMorph = isMorph;

      return segment;
    }

    moveTo(x: number, y: number) {
      sharedCommands.writeUnsignedByte(PathCommand.MoveTo);
      sharedData.write2Ints(x, y);
      this.commandsEnd = sharedCommands.position;
      this.dataEnd = sharedData.position;
    }

    morphMoveTo(x: number, y: number, mx: number, my: number) {
      this.moveTo(x, y);
      sharedMorphData.write2Ints(mx, my);
      this.morphDataEnd = sharedMorphData.position;
    }

    lineTo(x: number, y: number) {
      sharedCommands.writeUnsignedByte(PathCommand.LineTo);
      sharedData.write2Ints(x, y);
      this.commandsEnd = sharedCommands.position;
      this.dataEnd = sharedData.position;
    }

    morphLineTo(x: number, y: number, mx: number, my: number) {
      this.lineTo(x, y);
      sharedMorphData.write2Ints(mx, my);
      this.morphDataEnd = sharedMorphData.position;
    }

    curveTo(cpx: number, cpy: number, x: number, y: number) {
      sharedCommands.writeUnsignedByte(PathCommand.CurveTo);
      sharedData.write4Ints(cpx, cpy, x, y);
      ;
      this.commandsEnd = sharedCommands.position;
      this.dataEnd = sharedData.position;
    }

    morphCurveTo(cpx: number, cpy: number, x: number, y: number,
                 mcpx: number, mcpy: number, mx: number, my: number) {
      this.curveTo(cpx, cpy, x, y);
      sharedMorphData.write4Ints(mcpx, mcpy, mx, my);
      this.morphDataEnd = sharedMorphData.position;
    }

    /**
     * Returns a shallow copy of the segment with the "isReversed" flag set.
     * Reversed segments play themselves back in reverse when they're merged into the final
     * non-segmented path.
     * Note: Don't modify the original, or the reversed copy, after this operation!
     */
    toReversed(): PathSegment {
      release || assert(!this.isReversed);

      let segment = this.clone();
      segment.isReversed = true;
      return segment;
    }

    clone(): PathSegment {
      let segment = PathSegment.FromDefaults(this.isMorph);
      segment.commandsStart = this.commandsStart;
      segment.commandsEnd = this.commandsEnd;
      segment.dataStart = this.dataStart;
      segment.dataEnd = this.dataEnd;
      segment.morphDataStart = this.morphDataStart;
      segment.morphDataEnd = this.morphDataEnd;
      segment.isReversed = this.isReversed;
      return segment;
    }

    storeStartAndEnd() {
      let data = sharedData.ints;
      let pos = this.dataStart >> 2;
      let endPoint1 = data[pos] + data[pos + 1] * (1 << 24);
      pos = (this.dataEnd >> 2) - 2;
      let endPoint2 = data[pos] + data[pos + 1] * (1 << 24);
      if (!this.isReversed) {
        this.startPoint = endPoint1;
        this.endPoint = endPoint2;
      } else {
        this.startPoint = endPoint2;
        this.endPoint = endPoint1;
      }
      this.flag = false;
    }

    connectsTo(other: PathSegment): boolean {
      release || assert(other !== this);
      release || assert(this.endPoint);
      release || assert(other.startPoint);
      return this.endPoint === other.startPoint;
    }

    startConnectsTo(other: PathSegment): boolean {
      release || assert(other !== this);
      return this.startPoint === other.startPoint;
    }

    flipDirection() {
      let tempPoint = 0;
      tempPoint = this.startPoint;
      this.startPoint = this.endPoint;
      this.endPoint = tempPoint;
      this.isReversed = !this.isReversed;
    }

    serialize(shape: ShapeData, lastPosition: { x: number; y: number }) {
      if (this.isReversed) {
        this._serializeReversed(shape, lastPosition);
        return;
      }
      let commands = sharedCommands.bytes;
      // Note: this *must* use `this.data.length`, because buffers will have padding.
      let dataEnd = this.dataEnd >> 2;
      let morphData = this.isMorph ? sharedMorphData.ints : null;
      let data = sharedData.ints;
      let cPos = this.commandsStart;
      release || assert(commands[cPos] === PathCommand.MoveTo);
      // If the segment's first moveTo goes to the current coordinates, we have to skip it.
      let dataPosition = this.dataStart >> 2;
      let morphOffset = (this.morphDataStart >> 2) - dataPosition;
      let offset = 0;
      if (data[dataPosition] === lastPosition.x && data[dataPosition + 1] === lastPosition.y) {
        offset++;
      }
      let commandsCount = this.commandsEnd - this.commandsStart;
      dataPosition += offset * 2;
      for (let i = offset; i < commandsCount; i++) {
        dataPosition = this._writeCommand(commands[cPos + i], dataPosition, data, morphOffset, morphData, shape);
      }
      release || assert(dataPosition === dataEnd);
      lastPosition.x = data[dataEnd - 2];
      lastPosition.y = data[dataEnd - 1];
    }

    private _serializeReversed(shape: ShapeData, lastPosition: { x: number; y: number }) {
      // For reversing the fill0 segments, we rely on the fact that each segment
      // starts with a moveTo. We first write a new moveTo with the final drawing command's
      // target coordinates (if we don't skip it, see below). For each of the following
      // commands, we take the coordinates of the command originally *preceding*
      // it as the new target coordinates. The final coordinates we target will be
      // the ones from the original first moveTo.
      // Note: these *must* use `this.{data,commands}.length`, because buffers will have padding.
      let commandsCount = this.commandsEnd - this.commandsStart;
      let dataPosition = (this.dataEnd >> 2) - 2;
      let morphOffset = (this.morphDataEnd - this.dataEnd) >> 2;
      let commands = sharedCommands.bytes;
      let cPos = this.commandsStart;
      release || assert(commands[cPos] === PathCommand.MoveTo);
      let data = sharedData.ints;
      let morphData = this.isMorph ? sharedMorphData.ints : null;

      // Only write the first moveTo if it doesn't go to the current coordinates.
      if (data[dataPosition] !== lastPosition.x || data[dataPosition + 1] !== lastPosition.y) {
        this._writeCommand(PathCommand.MoveTo, dataPosition, data, morphOffset, morphData, shape);
      }
      if (commandsCount === 1) {
        lastPosition.x = data[this.dataStart >> 2];
        lastPosition.y = data[(this.dataStart >> 2) + 1];
        return;
      }
      for (let i = commandsCount; i-- > 1;) {
        dataPosition -= 2;
        let command: PathCommand = commands[cPos + i];
        shape.writeCommandAndCoordinates(command, data[dataPosition], data[dataPosition + 1]);
        if (morphData) {
          shape.writeMorphCoordinates(morphData[dataPosition + morphOffset], morphData[dataPosition + 1 + morphOffset]);
        }
        if (command === PathCommand.CurveTo) {
          dataPosition -= 2;
          shape.writeCoordinates(data[dataPosition], data[dataPosition + 1]);
          if (morphData) {
            shape.writeMorphCoordinates(morphData[dataPosition + morphOffset], morphData[dataPosition + 1 + morphOffset]);
          }
        } else {
        }
      }
      const dataStart = this.dataStart >> 2;
      release || assert(dataPosition === dataStart);
      lastPosition.x = data[dataStart];
      lastPosition.y = data[dataStart + 1];
    }

    private _writeCommand(command: PathCommand, position: number, data: Uint32Array,
                          morphOffset: number, morphData: Uint32Array, shape: ShapeData): number {
      shape.writeCommandAndCoordinates(command, data[position++], data[position++]);
      if (morphData) {
        shape.writeMorphCoordinates(morphData[position - 2 + morphOffset], morphData[position - 1 + morphOffset]);
      }
      if (command === PathCommand.CurveTo) {
        shape.writeCoordinates(data[position++], data[position++]);
        if (morphData) {
          shape.writeMorphCoordinates(morphData[position - 2 + morphOffset], morphData[position - 1 + morphOffset]);
        }
      }
      return position;
    }
  }

  let absCounter = 0;

  class SlowSegmentedPath {
    private _head: PathSegment;
    debugCount: number;

    constructor(public fillStyle: any, public lineStyle: any) {
      this._head = null;
      this.debugCount = 0;
    }

    addSegment(segment: PathSegment) {
      release || assert(segment);
      release || assert(segment.next === null);
      release || assert(segment.prev === null);
      let currentHead = this._head;
      if (currentHead) {
        release || assert(segment !== currentHead);
        currentHead.next = segment;
        segment.prev = currentHead;
      }
      this._head = segment;
      this.debugCount++;
    }

    // Does *not* reset the segment's prev and next pointers!
    removeSegment(segment: PathSegment) {
      if (segment.prev) {
        segment.prev.next = segment.next;
      }
      if (segment.next) {
        segment.next.prev = segment.prev;
      }
      this.debugCount--;
    }

    insertSegment(segment: PathSegment, next: PathSegment) {
      let prev = next.prev;
      segment.prev = prev;
      segment.next = next;
      if (prev) {
        prev.next = segment;
      }
      next.prev = segment;
      this.debugCount++;
    }

    head(): PathSegment {
      return this._head;
    }

    serialize(shape: ShapeData) {
      let segment = this.head();
      if (!segment) {
        // Path is empty.
        return null;
      }

      let debugOn = this.debugCount > 1000;
      let iterCount = 0, joinsCount = 0;

      console.log(`=== SEGMENTED PATH ${absCounter}===`);
      absCounter++;
      let counter = 0;

      while (segment) {
        segment.storeStartAndEnd();
        console.log(`${counter} : ${segment.startPoint} - ${segment.endPoint}`);
        segment = segment.prev;

        counter++;
      }

      let start = this.head();
      let end = start;

      let finalRoot: PathSegment = null;
      let finalHead: PathSegment = null;

      // Path segments for one style can appear in arbitrary order in the tag's list
      // of edge records.
      // Before we linearize them, we have to identify all pairs of segments where
      // one ends at a coordinate the other starts at.
      // The following loop does that, by creating ever-growing runs of matching
      // segments. If no more segments are found that match the current run (either
      // at the beginning, or at the end), the current run is complete, and a new
      // one is started. Rinse, repeat, until no solitary segments remain.
      let current = start.prev;
      while (start) {
        while (current) {
          iterCount++;

          if (current.startConnectsTo(start)) {
            current.flipDirection();
          }

          if (current.connectsTo(start)) {
            if (current.next !== start) {
              joinsCount++;
              this.removeSegment(current);
              this.insertSegment(current, start);
            }
            start = current;
            current = start.prev;
            continue;
          }

          if (current.startConnectsTo(end)) {
            current.flipDirection();
          }

          if (end.connectsTo(current)) {
            this.removeSegment(current);
            end.next = current;
            current = current.prev;
            end.next.prev = end;
            end.next.next = null;
            end = end.next;
            continue;
          }
          current = current.prev;
        }
        // This run of segments is finished. Store and forget it (for this loop).
        current = start.prev;
        if (!finalRoot) {
          finalRoot = start;
          finalHead = end;
        } else {
          finalHead.next = start;
          start.prev = finalHead;
          finalHead = end;
          finalHead.next = null;
        }
        if (!current) {
          break;
        }
        start = end = current;
        current = start.prev;
      }

      if (debugOn) {
        console.log(`Debug SegmentPath serialize segments=${this.debugCount}, iterations=${iterCount}, joins = ${joinsCount}`);
      }

      if (this.fillStyle) {
        let style = this.fillStyle;
        let morph = style.morph;
        switch (style.type) {
          case FillType.Solid:
            shape.beginFill(style.color);
            if (morph) {
              shape.writeMorphFill(morph.color);
            }
            break;
          case FillType.LinearGradient:
          case FillType.RadialGradient:
          case FillType.FocalRadialGradient:
            writeGradient(PathCommand.BeginGradientFill, style, shape);
            if (morph) {
              writeMorphGradient(morph, shape);
            }
            break;
          case FillType.ClippedBitmap:
          case FillType.RepeatingBitmap:
          case FillType.NonsmoothedClippedBitmap:
          case FillType.NonsmoothedRepeatingBitmap:
            writeBitmap(PathCommand.BeginBitmapFill, style, shape);
            if (morph) {
              writeMorphBitmap(morph, shape);
            }
            break;
          default:
            release || assertUnreachable('Invalid fill style type: ' + style.type);
        }
      } else {
        let style = this.lineStyle;
        let morph = style.morph;
        release || assert(style);
        switch (style.type) {
          case FillType.Solid:
            writeLineStyle(style, shape);
            if (morph) {
              writeMorphLineStyle(morph, shape);
            }
            break;
          case FillType.LinearGradient:
          case FillType.RadialGradient:
          case FillType.FocalRadialGradient:
            writeLineStyle(style, shape);
            writeGradient(PathCommand.LineStyleGradient, style, shape);
            if (morph) {
              writeMorphLineStyle(morph, shape);
              writeMorphGradient(morph, shape);
            }
            break;
          case FillType.ClippedBitmap:
          case FillType.RepeatingBitmap:
          case FillType.NonsmoothedClippedBitmap:
          case FillType.NonsmoothedRepeatingBitmap:
            writeLineStyle(style, shape);
            writeBitmap(PathCommand.LineStyleBitmap, style, shape);
            if (morph) {
              writeMorphLineStyle(morph, shape);
              writeMorphBitmap(morph, shape);
            }
            break;
          default:
          //console.error('Line style type not yet supported: ' + style.type);
        }
      }

      let lastPosition = {x: 0, y: 0};
      current = finalRoot;
      counter = 0;
      console.log("--- AFTER SORT");
      while (current) {
        current.serialize(shape, lastPosition);
        console.log(`${counter} : ${current.startPoint} - ${current.endPoint}`);
        current = current.next;
        counter++;
      }
      if (this.fillStyle) {
        shape.endFill();
      } else {
        shape.endLine();
      }
      return shape;
    }
  }

  function writeLineStyle(style: ShapeStyle, shape: ShapeData): void {
    // No scaling == 0, normal == 1, vertical only == 2, horizontal only == 3.
    let scaleMode = style.noHscale ?
      (style.noVscale ? 0 : 2) :
      style.noVscale ? 3 : 1;
    // TODO: Figure out how to handle startCapsStyle
    let thickness = clamp(style.width, 0, 0xff * 20) | 0;
    shape.lineStyle(thickness, style.color,
      style.pixelHinting, scaleMode, style.endCapsStyle,
      style.jointStyle, style.miterLimit);
  }

  function writeMorphLineStyle(style: ShapeStyle, shape: ShapeData): void {
    // TODO: Figure out how to handle startCapsStyle
    let thickness = clamp(style.width, 0, 0xff * 20) | 0;
    shape.writeMorphLineStyle(thickness, style.color);
  }

  function writeGradient(command: PathCommand, style: ShapeStyle, shape: ShapeData): void {
    let gradientType = style.type === FillType.LinearGradient ?
      GradientType.Linear :
      GradientType.Radial;
    shape.beginGradient(command, style.colors, style.ratios,
      gradientType, style.transform, style.spreadMethod,
      style.interpolationMode, style.focalPoint / 2 | 0);
  }

  function writeMorphGradient(style: ShapeStyle, shape: ShapeData) {
    shape.writeMorphGradient(style.colors, style.ratios, style.transform);
  }

  function writeBitmap(command: PathCommand, style: ShapeStyle, shape: ShapeData): void {
    shape.beginBitmap(command, style.bitmapIndex, style.transform, style.repeat, style.smooth);
  }

  function writeMorphBitmap(style: ShapeStyle, shape: ShapeData) {
    shape.writeMorphBitmap(style.transform);
  }


  class SegmentedPath {
    segments: Array<PathSegment>;
    match: Map<number, PathSegment>;

    constructor(public fillStyle: any, public lineStyle: any) {
      this.segments = [];
      this.match = new Map();
    }

    addSegment(segment: PathSegment) {
      this.segments.push(segment);
    }

    checkSegment(segment: PathSegment) {
      const match = this.match;

      segment.storeStartAndEnd();
      segment.prev = null;
      segment.next = null;
      segment.flag = true;

      let p = match.get(segment.startPoint);
      if (p) {
        segment.prev = p;
        if (p.prev) {
          p.next = segment;
        } else {
          p.prev = segment;
        }
        match.delete(segment.startPoint);
      } else {
        match.set(segment.startPoint, segment);
      }

      p = match.get(segment.endPoint);
      if (p) {
        segment.next = p;
        if (p.prev) {
          p.next = segment;
        } else {
          p.prev = segment;
        }
        match.delete(segment.endPoint);
      } else {
        match.set(segment.endPoint, segment);
      }
    }

    serialize(shape: ShapeData) {
      const segments = this.segments;
      if (segments.length === 0) {
        return null;
      }

      this.serializeStyle(shape);

      let lastPosition = {x: 0, y: 0};

      // console.log(`=== SEGMENTED PATH ${absCounter}===`);
      // absCounter++;

      for (let i = segments.length - 1; i >= 0; i--) {
        this.checkSegment(segments[i]);
        // console.log(`${i} : ${segments[i].startPoint} - ${segments[i].endPoint}`);
      }

      // let counter = 0;
      // console.log("--- AFTER SORT");

      for (let i = segments.length - 1; i >= 0; i--) {
        let seg = segments[i];
        if (!seg.flag) {
          continue;
        }

        let matchEnd = 0;
        if (seg.prev && (seg.prev.startPoint === seg.endPoint
          || seg.prev.endPoint === seg.endPoint)) {
          matchEnd = -1;
        }
        if (seg.next && (seg.next.startPoint === seg.endPoint
          || seg.next.endPoint === seg.endPoint)) {
          matchEnd = 1;
        }

        // find the start of sequence
        if (seg.next && seg.prev) {
          let current = matchEnd === -1 ? seg.prev : seg.next;
          if (seg.prev && (seg.prev.startPoint === seg.endPoint
            || seg.prev.endPoint === seg.endPoint)) {
            current = seg.next;
          }

          let prev = seg;
          while (current !== seg && current) {
            let next = current.next === prev ? current.prev : current.next;
            if (!next) {
              break;
            }
            prev = current;
            current = next;
          }

          if (current !== seg) {
            seg = current;

            matchEnd = 0;
            if (seg.prev && (seg.prev.startPoint === seg.endPoint
              || seg.prev.endPoint === seg.endPoint)) {
              matchEnd = -1;
            }
            if (seg.next && (seg.next.startPoint === seg.endPoint
              || seg.next.endPoint === seg.endPoint)) {
              matchEnd = 1;
            }
          }
        }

        if (matchEnd === 0 && (seg.next || seg.prev)) {
          seg.flipDirection();
        }

        let current = seg.next;
        if (seg.prev && (seg.prev.startPoint === seg.endPoint
          || seg.prev.endPoint === seg.endPoint)) {
          current = seg.prev;
        }
        seg.serialize(shape, lastPosition);
        //console.log(`${counter} : ${seg.startPoint} - ${seg.endPoint}`);
        // counter++;

        seg.flag = false;
        let prev = seg;
        let prevPoint = seg.endPoint;

        while (current && current.flag) {
          current.flag = false;
          if (current.endPoint === prevPoint) current.flipDirection();

          current.serialize(shape, lastPosition);
          // console.log(`${counter} : ${current.startPoint} - ${current.endPoint}`);
          // counter++;

          prevPoint = current.endPoint;
          let next = current.next === prev ? current.prev : current.next;
          prev = current;
          current = next;
        }
      }

      this.match = new Map();

      if (this.fillStyle) {
        shape.endFill();
      } else {
        shape.endLine();
      }
      return shape;
    }

    serializeStyle(shape: ShapeData) {
      if (this.fillStyle) {
        let style = this.fillStyle;
        let morph = style.morph;
        switch (style.type) {
          case FillType.Solid:
            shape.beginFill(style.color);
            if (morph) {
              shape.writeMorphFill(morph.color);
            }
            break;
          case FillType.LinearGradient:
          case FillType.RadialGradient:
          case FillType.FocalRadialGradient:
            writeGradient(PathCommand.BeginGradientFill, style, shape);
            if (morph) {
              writeMorphGradient(morph, shape);
            }
            break;
          case FillType.ClippedBitmap:
          case FillType.RepeatingBitmap:
          case FillType.NonsmoothedClippedBitmap:
          case FillType.NonsmoothedRepeatingBitmap:
            writeBitmap(PathCommand.BeginBitmapFill, style, shape);
            if (morph) {
              writeMorphBitmap(morph, shape);
            }
            break;
          default:
            release || assertUnreachable('Invalid fill style type: ' + style.type);
        }
      } else {
        let style = this.lineStyle;
        let morph = style.morph;
        release || assert(style);
        switch (style.type) {
          case FillType.Solid:
            writeLineStyle(style, shape);
            if (morph) {
              writeMorphLineStyle(morph, shape);
            }
            break;
          case FillType.LinearGradient:
          case FillType.RadialGradient:
          case FillType.FocalRadialGradient:
            writeLineStyle(style, shape);
            writeGradient(PathCommand.LineStyleGradient, style, shape);
            if (morph) {
              writeMorphLineStyle(morph, shape);
              writeMorphGradient(morph, shape);
            }
            break;
          case FillType.ClippedBitmap:
          case FillType.RepeatingBitmap:
          case FillType.NonsmoothedClippedBitmap:
          case FillType.NonsmoothedRepeatingBitmap:
            writeLineStyle(style, shape);
            writeBitmap(PathCommand.LineStyleBitmap, style, shape);
            if (morph) {
              writeMorphLineStyle(morph, shape);
              writeMorphBitmap(morph, shape);
            }
            break;
          default:
          //console.error('Line style type not yet supported: ' + style.type);
        }
      }
    }
  }
}
