import {SRgb} from "semantic-types";
import {Shape} from "./simple-shape/shape";

export interface Renderer {
  readonly bufferWidth: number;
  readonly bufferHeight: number;

  setClearColor(color: SRgb<number>): void;

  clear(): void;

  drawShape(shape: Shape): void;

  exportImageData(): ImageData;
}

export interface RendererFactory<R extends Renderer = Renderer> {
  create(width: number, height: number): R;
}
