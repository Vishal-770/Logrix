// Arbitrary-Precision BigInt Type (Supports Hex & Decimal, EVM 256-bit safe)

import {
  addDecimal,
  cmpDecimal,
  decimalToHex,
  divDecimal,
  hexToDecimal,
  hexToSignedDecimal,
  mulDecimal,
  subDecimal,
} from "./math";

export class BigInt {
  private _raw: string;

  constructor(val: string) {
    let clean = val.trim();
    if (clean.startsWith("0x") || clean.startsWith("0X")) {
      this._raw = hexToDecimal(clean);
    } else {
      this._raw = clean.length > 0 ? clean : "0";
    }
  }

  static fromString(val: string): BigInt {
    return new BigInt(val);
  }

  static fromSignedString(val: string, bitWidth: i32 = 256): BigInt {
    let clean = val.trim();
    if (clean.startsWith("0x") || clean.startsWith("0X")) {
      const dec = hexToSignedDecimal(clean, bitWidth);
      const b = new BigInt("0");
      b._raw = dec;
      return b;
    }
    return new BigInt(clean);
  }

  static fromI32(val: i32): BigInt {
    return new BigInt(val.toString());
  }

  static fromU64(val: u64): BigInt {
    return new BigInt(val.toString());
  }

  static zero(): BigInt {
    return new BigInt("0");
  }

  toString(): string {
    return this._raw;
  }

  toHexString(): string {
    return "0x" + decimalToHex(this._raw);
  }

  plus(other: BigInt): BigInt {
    return new BigInt(addDecimal(this._raw, other._raw));
  }

  minus(other: BigInt): BigInt {
    return new BigInt(subDecimal(this._raw, other._raw));
  }

  times(other: BigInt): BigInt {
    return new BigInt(mulDecimal(this._raw, other._raw));
  }

  div(other: BigInt): BigInt {
    return new BigInt(divDecimal(this._raw, other._raw));
  }

  equals(other: BigInt): bool {
    return cmpDecimal(this._raw, other._raw) == 0;
  }

  gt(other: BigInt): bool {
    return cmpDecimal(this._raw, other._raw) > 0;
  }

  lt(other: BigInt): bool {
    return cmpDecimal(this._raw, other._raw) < 0;
  }

  gte(other: BigInt): bool {
    return cmpDecimal(this._raw, other._raw) >= 0;
  }

  lte(other: BigInt): bool {
    return cmpDecimal(this._raw, other._raw) <= 0;
  }
}
