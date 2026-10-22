// EVM Decimal and Units formatting utilities.

import { BigInt } from "./bigint";

/**
 * Formats a BigInt wei amount into human-readable units (e.g. ether with 18 decimals).
 * Examples:
 *   formatUnits(1000000000000000000, 18) => "1"
 *   formatUnits(1500000000000000000, 18) => "1.5"
 *   formatUnits(500000000000000000, 18)  => "0.5"
 */
export function formatUnits(value: BigInt, decimals: i32 = 18): string {
  let s = value.toString();
  if (decimals <= 0) return s;
  let isNegative = false;
  if (s.startsWith("-")) {
    isNegative = true;
    s = s.slice(1);
  }
  if (s.length <= decimals) {
    let zeros = "";
    for (let i = 0; i < decimals - s.length; i++) {
      zeros += "0";
    }
    s = "0." + zeros + s;
  } else {
    const intPart = s.slice(0, s.length - decimals);
    const fracPart = s.slice(s.length - decimals);
    s = intPart + "." + fracPart;
  }
  const dotIdx = s.indexOf(".");
  if (dotIdx != -1) {
    let end = s.length - 1;
    while (end >= dotIdx && (s.charAt(end) == "0" || s.charAt(end) == ".")) {
      if (s.charAt(end) == ".") {
        end--;
        break;
      }
      end--;
    }
    s = s.slice(0, end + 1);
  }
  return isNegative ? "-" + s : s;
}
