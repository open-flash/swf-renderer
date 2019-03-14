// import { Shape } from "../simple-shape/shape";

export type RgbaVec = [number, number, number, number];

export class NodeGlRenderer {
  // private viewportWidth: number;
  // private viewportHeight: number;
  //
  // private backgroundColor: RgbaVec;

  private context: WebGLRenderingContext;

  constructor(context: WebGLRenderingContext, _width: number, _height: number) {
    this.context = context;
    // this.viewportWidth = width;
    // this.viewportHeight = height;
    // this.backgroundColor = [0, 0, 0, 1];
  }

  render(_shapes: any /* Shape[] */): void {
    this.context.clearColor(1, 0, 0, 1);
  }
}
