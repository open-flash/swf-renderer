import canvas from "canvas";
import { Stage } from "../display/stage";
import { Renderer } from "../renderer";
import { CanvasRenderer } from "./canvas-renderer";

export class NodeCanvasRenderer implements Renderer {
  public readonly canvas: canvas.Canvas;

  constructor(width: number, height: number) {
    this.canvas = canvas.createCanvas(width, height);
  }

  render(stage: Stage): void {
    const ctx: canvas.CanvasRenderingContext2D = this.canvas.getContext("2d");
    const cvsRender: CanvasRenderer = new CanvasRenderer(ctx);
    cvsRender.render(stage);
  }
}
