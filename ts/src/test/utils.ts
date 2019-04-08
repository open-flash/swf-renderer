import fs from "fs";
import sysPath from "path";
import meta from "./meta.js";

export const PROJECT_ROOT: string = sysPath.join(meta.dirname, "..", "..", "..");
export const TEST_SAMPLES_ROOT: string = sysPath.join(PROJECT_ROOT, "..", "tests");

export async function readTextFile(filePath: fs.PathLike): Promise<string> {
  return new Promise<string>((resolve, reject): void => {
    fs.readFile(filePath, {encoding: "UTF-8"}, (err: NodeJS.ErrnoException | null, data: string): void => {
      if (err !== null) {
        reject(err);
      } else {
        resolve(data);
      }
    });
  });
}

export async function writeTextFile(filePath: fs.PathLike, text: string): Promise<void> {
  return new Promise<void>((resolve, reject): void => {
    fs.writeFile(filePath, text, (err: NodeJS.ErrnoException | null): void => {
      if (err !== null) {
        reject(err);
      } else {
        resolve();
      }
    });
  });
}
