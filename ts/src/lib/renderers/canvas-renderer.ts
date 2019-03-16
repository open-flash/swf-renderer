import { Incident } from "incident";
import { Matrix } from "swf-tree/matrix";
import { DefineMorphShape, DefineShape } from "swf-tree/tags";
import { fromNormalizedColor } from "../css-color";
import { DisplayObject } from "../display/display-object";
import { DisplayObjectContainer } from "../display/display-object-container";
import { DisplayObjectType } from "../display/display-object-type";
import { MorphShape } from "../display/morph-shape";
import { Shape } from "../display/shape";
import { Stage } from "../display/stage";
import { Renderer } from "../renderer";
import { decodeSwfMorphShape } from "../shape/decode-swf-morph-shape";
import { decodeSwfShape } from "../shape/decode-swf-shape";
import { FillStyleType } from "../shape/fill-style";
import { LineStyleType } from "../shape/line-style";
import { MorphFillStyleType } from "../shape/morph-fill-style";
import { MorphLineStyleType } from "../shape/morph-line-style";
import { MorphCommandType, MorphPath } from "../shape/morph-path";
import { MorphShape as CompiledMorphShape } from "../shape/morph-shape";
import { CommandType, Path } from "../shape/path";
import { Shape as CompiledShape } from "../shape/shape";

function lerp(start: number, end: number, ratio: number): number {
  return end * ratio + start * (1 - ratio);
}

interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

function lerpRgba(start: Rgba, end: Rgba, ratio: number): Rgba {
  return {
    r: lerp(start.r, end.r, ratio),
    g: lerp(start.g, end.g, ratio),
    b: lerp(start.b, end.b, ratio),
    a: lerp(start.a, end.a, ratio),
  };
}

// function clamp(x: number, min: number, max: number): number {
//   return Math.min(Math.max(x, min), max);
// }

export class CanvasRenderer implements Renderer {
  private readonly context: CanvasRenderingContext2D;
  private readonly morphShapeCache: WeakMap<DefineMorphShape, CompiledMorphShape>;
  private readonly shapeCache: WeakMap<DefineShape, CompiledShape>;
  // private width: number;
  // private height: number;

  constructor(context: CanvasRenderingContext2D /*, width: number, height: number */) {
    this.context = context;
    this.morphShapeCache = new WeakMap();
    this.shapeCache = new WeakMap();
    // this.width = width;
    // this.height = height;
  }

  // updateSize(width: number, height: number): void {
  //   this.width = width;
  //   this.height = height;
  // }

  render(stage: Stage): void {
    this.renderStage(stage);
  }

  private renderStage(stage: Stage): void {
    this.context.setTransform(1, 0, 0, 1, 0, 0);
    this.context.clearRect(0, 0, this.context.canvas.width, this.context.canvas.height);
    // TODO: FillRect with background color?

    this.context.scale(1 / 20, 1 / 20);
    for (const child of stage.children) {
      this.drawDisplayObject(child);
    }
  }

  private drawDisplayObject(displayObject: DisplayObject): void {
    switch (displayObject.type) {
      case DisplayObjectType.Container:
        this.drawContainer(displayObject);
        break;
      case DisplayObjectType.Shape:
        this.drawShape(displayObject);
        break;
      case DisplayObjectType.MorphShape:
        this.drawMorphShape(displayObject);
        break;
      default:
        throw new Error("UnexpectedDisplayObjectType");
    }
  }

  private getCompiledMorphShape(swfShape: DefineMorphShape): CompiledMorphShape {
    let compiled: CompiledMorphShape | undefined = this.morphShapeCache.get(swfShape);
    if (compiled === undefined) {
      compiled = decodeSwfMorphShape(swfShape);
      this.morphShapeCache.set(swfShape, compiled);
    }
    return compiled;
  }

  private getCompiledShape(swfShape: DefineShape): CompiledShape {
    let compiled: CompiledShape | undefined = this.shapeCache.get(swfShape);
    if (compiled === undefined) {
      compiled = decodeSwfShape(swfShape);
      this.shapeCache.set(swfShape, compiled);
    }
    return compiled;
  }

  private drawShape(shape: Shape): void {
    this.context.save();
    try {
      if (shape.matrix !== undefined) {
        this.applyMatrix(shape.matrix);
      }
      const compiled: CompiledShape = this.getCompiledShape(shape.definition);
      for (const path of compiled.paths) {
        this.drawPath(path);
      }
    } catch (err) {
      throw err;
    } finally {
      this.context.restore();
    }
  }

  private drawContainer(container: DisplayObjectContainer): void {
    this.context.save();
    try {
      if (container.matrix !== undefined) {
        this.applyMatrix(container.matrix);
      }
      for (const child of container.children) {
        this.drawDisplayObject(child);
      }
    } catch (err) {
      throw err;
    } finally {
      this.context.restore();
    }
  }
  //
  // private renderSimpleButton(simpleButton: SimpleButton): void {
  //   switch (simpleButton.state) {
  //     case ButtonState.Up:
  //       if (simpleButton.upState !== undefined) {
  //         this.renderDisplayObject(simpleButton.upState);
  //       }
  //       break;
  //     case ButtonState.Down:
  //       if (simpleButton.downState !== undefined) {
  //         this.renderDisplayObject(simpleButton.downState);
  //       }
  //       break;
  //     case ButtonState.Over:
  //       if (simpleButton.overState !== undefined) {
  //         this.renderDisplayObject(simpleButton.overState);
  //       }
  //       break;
  //     case ButtonState.HitTest:
  //       if (simpleButton.hitTestState !== undefined) {
  //         this.renderDisplayObject(simpleButton.hitTestState);
  //       }
  //       break;
  //     default:
  //       throw new Incident("UnexpectedSwitchVariant", simpleButton.state);
  //   }
  // }

  // private renderLoader(loader: SwfLoader): void {
    // console.log("Rendering loader");
  // }

  private applyMatrix(matrix: Matrix): void {
    this.context.transform(
      matrix.scaleX.valueOf(),
      matrix.rotateSkew0.valueOf(),
      matrix.rotateSkew1.valueOf(),
      matrix.scaleY.valueOf(),
      matrix.translateX,
      matrix.translateY,
    );
  }

  private drawMorphShape(shape: MorphShape): void {
    this.context.save();
    try {
      if (shape.matrix !== undefined) {
        this.applyMatrix(shape.matrix);
      }
      const compiled: CompiledMorphShape = this.getCompiledMorphShape(shape.definition);
      for (const path of compiled.paths) {
        this.drawMorphPath(path, shape.ratio);
      }
    } catch (err) {
      throw err;
    } finally {
      this.context.restore();
    }
  }

  private drawMorphPath(path: MorphPath, ratio: number): void {
    if (path.fill === undefined && path.line === undefined || path.commands.length === 0) {
      return;
    }

    this.context.beginPath();

    for (const command of path.commands) {
      switch (command.type) {
        case MorphCommandType.CurveTo:
          this.context.quadraticCurveTo(
            lerp(command.controlX[0], command.controlY[1], ratio),
            lerp(command.controlY[0], command.controlY[1], ratio),
            lerp(command.endX[0], command.endX[1], ratio),
            lerp(command.endY[0], command.endY[1], ratio),
          );
          break;
        case MorphCommandType.LineTo:
          this.context.lineTo(
            lerp(command.endX[0], command.endX[1], ratio),
            lerp(command.endY[0], command.endY[1], ratio),
          );
          break;
        case MorphCommandType.MoveTo:
          this.context.moveTo(
            lerp(command.x[0], command.x[1], ratio),
            lerp(command.y[0], command.y[1], ratio),
          );
          break;
        default:
          throw new Incident("UnexpectedMorphCommand", {command});
      }
    }

    if (path.fill !== undefined) {
      switch (path.fill.type) {
        case MorphFillStyleType.Solid:
          this.context.fillStyle = fromNormalizedColor(lerpRgba(path.fill.startColor, path.fill.endColor, ratio));
          break;
        default:
          throw new Incident("NotImplementedFillStyle", {style: path.fill});
      }
      this.context.fill();
    }

    if (path.line !== undefined) {
      switch (path.line.type) {
        case MorphLineStyleType.Solid:
          this.context.lineWidth = lerp(path.line.width[0], path.line.width[1], ratio);
          this.context.strokeStyle = fromNormalizedColor(lerpRgba(path.line.startColor, path.line.endColor, ratio));
          break;
        default:
          throw new Incident("NotImplementedLineStyle", {style: path.line});
      }
      this.context.lineCap = "round";
      this.context.lineJoin = "round";
      this.context.stroke();
    }
  }

  private drawPath(path: Path): void {
    if (path.fill === undefined && path.line === undefined || path.commands.length === 0) {
      return;
    }

    this.context.beginPath();

    for (const command of path.commands) {
      switch (command.type) {
        case CommandType.CurveTo:
          this.context.quadraticCurveTo(command.controlX, command.controlY, command.endX, command.endY);
          break;
        case CommandType.LineTo:
          this.context.lineTo(command.endX, command.endY);
          break;
        case CommandType.MoveTo:
          this.context.moveTo(command.x, command.y);
          break;
        default:
          throw new Incident("UnexpectedCommand", {command});
      }
    }

    if (path.fill !== undefined) {
      this.context.save();
      switch (path.fill.type) {
        case FillStyleType.Bitmap:
          this.context.fillStyle = fromNormalizedColor({
            r: 0.2,
            g: 0.6,
            b: 0.8,
            a: 0.9,
          });
          break;
        case FillStyleType.Solid:
          this.context.fillStyle = fromNormalizedColor(path.fill.color);
          break;
        case FillStyleType.FocalGradient:
          this.applyMatrix(path.fill.matrix);
          const GRAD_RADIUS: number = 16384;
          const gradient: CanvasGradient = this.context.createRadialGradient(
            lerp(0, GRAD_RADIUS, path.fill.focalPoint), 0, 0,
            0, 0, GRAD_RADIUS,
          );
          for (const colorStop of path.fill.gradient.colors) {
            gradient.addColorStop(colorStop.ratio, fromNormalizedColor(colorStop.color));
          }
          this.context.fillStyle = gradient;
          break;
        default:
          throw new Incident("NotImplementedFillStyle", {style: path.fill});
      }
      this.context.fill();
      this.context.restore();
    }

    if (path.line !== undefined) {
      switch (path.line.type) {
        case LineStyleType.Solid:
          this.context.lineWidth = path.line.width;
          this.context.strokeStyle = fromNormalizedColor(path.line.color);
          break;
        default:
          throw new Incident("NotImplementedLineStyle", {style: path.line});
      }
      this.context.stroke();
    }
  }
}
