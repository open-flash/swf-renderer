import { BitmapProvider } from "../bitmap-service";

export class NoopBitmapService implements BitmapProvider<any> {
  getById(): never {
    throw new Error("NotSupported: NoopBitmapService#getById");
  }
}
