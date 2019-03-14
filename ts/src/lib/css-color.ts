import { StraightSRgba } from "semantic-types";

/**
 * Represents a valid CSS color such as `"rgba(200, 13, 53, 0.5)"` or `"transparent"`.
 */
export type CssColor = string;

/**
 * Converts a normalized color to a CSS color
 */
export function fromNormalizedColor(color: StraightSRgba<number>): CssColor {
  return `rgba(${(color.r * 0xff) & 0xff}, ${color.g * 255}, ${color.b * 255}, ${color.a})`;
}
