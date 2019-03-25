import { DefineBitmap } from "swf-tree/tags";

export interface Bitmap<T> {
  width: number;
  height: number;
  bitmap: T | undefined;
  bitmap$: Promise<T>;
}

export interface BitmapProvider<T> {
  getById(id: number): Bitmap<T>;
}

export interface BitmapConsumer {
  addBitmap(tag: DefineBitmap): void;
}
