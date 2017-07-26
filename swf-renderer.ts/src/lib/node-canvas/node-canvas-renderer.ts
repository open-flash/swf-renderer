import Canvas = require("canvas");
import {SRgb, StraightSRgba} from "semantic-types";
import {Renderer} from "../renderer";
import {FillType} from "../simple-shape/fill-style";
import {LineType} from "../simple-shape/line-style";
import {CommandType, PathWithStyle} from "../simple-shape/path";
import {Shape} from "../simple-shape/shape";

function straightSRgbaToCanvasColor(color: StraightSRgba<number>): string {
  return `rgba(${color.r * 255}, ${color.g * 255}, ${color.b * 255}, ${color.a})`;
}

export class NodeCanvasRenderer implements Renderer {
  readonly bufferWidth: number;
  readonly bufferHeight: number;

  private clearColor: string;
  private readonly context: CanvasRenderingContext2D;

  constructor(width: number, height: number) {
    this.bufferWidth = width;
    this.bufferHeight = height;
    this.clearColor = "#ffffff";

    const canvas: Canvas = new Canvas(width, height);
    const ctx: CanvasRenderingContext2D | null = canvas.getContext("2d");
    if (ctx === null) {
      throw new Error("Cannot create renderer");
    }
    this.context = ctx;
    this.context.scale(1 / 20, 1 / 20);
  }

  static create(width: number, height: number): NodeCanvasRenderer {
    return new NodeCanvasRenderer(width, height);
  }

  setClearColor(color: SRgb<number>): void {
    this.clearColor = straightSRgbaToCanvasColor({...color, a: 1});
  }

  clear(): void {
    this.context.scale(20, 20);
    this.context.fillStyle = this.clearColor;
    this.context.fillRect(0, 0, this.bufferWidth, this.bufferHeight);
    this.context.scale(1 / 20, 1 / 20);
  }

  drawPath(path: PathWithStyle): void {
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
          throw new Error("FailedAssertion: Unexpected command");
      }
    }

    if (path.fill !== undefined) {
      switch (path.fill.type) {
        case FillType.Solid:
          this.context.fillStyle = straightSRgbaToCanvasColor(path.fill.color);
          break;
        default:
          throw new Error("TODO: FailedAssertion");
      }
      this.context.fill();
    }

    if (path.line !== undefined) {
      switch (path.line.type) {
        case LineType.Solid:
          this.context.lineWidth = path.line.width;
          this.context.strokeStyle = straightSRgbaToCanvasColor(path.line.color);
          break;
        default:
          throw new Error("TODO: FailedAssertion");
      }
      this.context.stroke();
    }
  }

  drawShape(shape: Shape): void {
    for (const path of shape.paths) {
      this.drawPath(path);
    }
  }

  exportImageData(): ImageData {
    return this.context.getImageData(0, 0, this.bufferWidth, this.bufferHeight);
  }
}
