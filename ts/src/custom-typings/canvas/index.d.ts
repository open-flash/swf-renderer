declare module "canvas" {
  namespace canvas {
    interface CanvasConstructor {
      new(width: number, height: number): Canvas;
    }

    interface Canvas {
      getContext(contextId: "2d", contextAttributes?: Canvas2DContextAttributes): CanvasRenderingContext2D | null;
    }
  }

  type canvas = canvas.Canvas;
  const canvas: canvas.CanvasConstructor;

  export = canvas;
}
