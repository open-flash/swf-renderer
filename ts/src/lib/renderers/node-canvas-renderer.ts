import canvas from "canvas";
import { DefineBitmap } from "swf-tree/tags";
import { Stage } from "../display/stage";
import { Renderer } from "../renderer";
import { CanvasRenderer } from "./canvas-renderer";

export class NodeCanvasRenderer implements Renderer {
  public readonly canvas: canvas.Canvas;
  private readonly renderer: CanvasRenderer;

  constructor(width: number, height: number) {
    this.canvas = canvas.createCanvas(width, height);
    const ctx: canvas.CanvasRenderingContext2D = this.canvas.getContext("2d");
    this.renderer = new CanvasRenderer(ctx);
  }

  render(stage: Stage): void {
    this.renderer.render(stage);
  }

  addBitmap(tag: DefineBitmap): Promise<void> {
    return this.renderer.addBitmap(tag);
  }
}
