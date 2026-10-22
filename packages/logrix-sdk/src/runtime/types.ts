// EVM types, JSON extractors, and formatting helpers.

import { parseHexChar, parseU64 } from "./math";

/**
 * Address Type representing a 20-byte EVM contract or account address.
 */
export class Address {
  private _val: string;

  constructor(hex: string) {
    let clean = hex.trim().toLowerCase();
    if (clean.startsWith("0x")) clean = clean.slice(2);
    // If 32-byte topic (64 chars), take the last 40 chars for EVM 20-byte address
    if (clean.length == 64) {
      clean = clean.slice(24);
    }
    this._val = "0x" + clean;
  }

  static fromString(hex: string): Address {
    return new Address(hex);
  }

  toHexString(): string {
    return this._val;
  }

  toString(): string {
    return this._val;
  }

  equals(other: Address): bool {
    return this._val == other._val;
  }
}

/**
 * Arbitrary byte sequence with hex and byte-array representations.
 */
export class Bytes {
  private _hex: string;

  constructor(hex: string) {
    let clean = hex.trim().toLowerCase();
    if (clean.startsWith("0x")) clean = clean.slice(2);
    if (clean.length % 2 != 0) clean = "0" + clean;
    this._hex = "0x" + clean;
  }

  static fromHexString(hex: string): Bytes {
    return new Bytes(hex);
  }

  static fromUTF8(str: string): Bytes {
    const buf = String.UTF8.encode(str);
    const u8 = Uint8Array.wrap(buf);
    return Bytes.fromByteArray(u8);
  }

  static fromByteArray(arr: Uint8Array): Bytes {
    const hexChars = "0123456789abcdef";
    let res = "0x";
    for (let i = 0; i < arr.length; i++) {
      const b = arr[i];
      res += hexChars.charAt((b >> 4) & 0x0f) + hexChars.charAt(b & 0x0f);
    }
    return new Bytes(res);
  }

  toByteArray(): Uint8Array {
    const clean = this._hex.slice(2);
    const len = clean.length / 2;
    const res = new Uint8Array(len);
    for (let i = 0; i < len; i++) {
      const high = parseHexChar(clean.charCodeAt(i * 2));
      const low = parseHexChar(clean.charCodeAt(i * 2 + 1));
      res[i] = ((high << 4) | low) as u8;
    }
    return res;
  }

  toHexString(): string {
    return this._hex;
  }

  toString(): string {
    return this._hex;
  }

  get length(): i32 {
    const raw = this._hex.slice(2);
    return (raw.length as i32) / 2;
  }

  equals(other: Bytes): bool {
    return this._hex == other._hex;
  }
}

/**
 * Blockchain event log payload delivered to indexer handlers.
 */
export class EventLog {
  address: string = "";
  block_number: u64 = 0;
  block_hash: string = "";
  transaction_hash: string = "";
  log_index: u32 = 0;
  block_timestamp: u64 = 0;
  topics: Array<string> = [];
  data: string = "";

  static fromJson(json: string): EventLog {
    const log = new EventLog();
    log.address = extractJsonField(json, "address");

    const blockNumStr = extractJsonField(json, "block_number");
    if (blockNumStr.length > 0) {
      log.block_number = parseU64(blockNumStr);
    }

    log.block_hash = extractJsonField(json, "block_hash");

    let txHash = extractJsonField(json, "tx_hash");
    if (txHash.length == 0) {
      txHash = extractJsonField(json, "transaction_hash");
    }
    log.transaction_hash = txHash;

    const logIdxStr = extractJsonField(json, "log_index");
    if (logIdxStr.length > 0) {
      log.log_index = u32(parseU64(logIdxStr));
    }

    const tsStr = extractJsonField(json, "block_timestamp");
    if (tsStr.length > 0) {
      log.block_timestamp = parseU64(tsStr);
    }

    log.data = extractJsonField(json, "data");
    log.topics = parseJsonStringArray(json, "topics");
    return log;
  }
}

export function parseJsonStringArray(json: string, key: string): Array<string> {
  const result = new Array<string>();
  const needle = '"' + key + '":';
  const idx = json.indexOf(needle);
  if (idx == -1) return result;

  const arrayStart = json.indexOf("[", idx + needle.length);
  if (arrayStart == -1) return result;

  const arrayEnd = json.indexOf("]", arrayStart);
  if (arrayEnd == -1) return result;

  const inside = json.slice(arrayStart + 1, arrayEnd);
  let pos = 0;
  while (pos < inside.length) {
    const q1 = inside.indexOf('"', pos);
    if (q1 == -1) break;
    const q2 = inside.indexOf('"', q1 + 1);
    if (q2 == -1) break;
    result.push(inside.slice(q1 + 1, q2));
    pos = q2 + 1;
  }
  return result;
}

export function extractJsonField(json: string, key: string): string {
  const needle = '"' + key + '":';
  const idx = json.indexOf(needle);
  if (idx == -1) return "";
  let start = idx + needle.length;
  while (
    start < json.length &&
    (json.charAt(start) == " " ||
      json.charAt(start) == "\t" ||
      json.charAt(start) == "\n" ||
      json.charAt(start) == "\r")
  ) {
    start++;
  }
  if (start >= json.length) return "";
  const ch = json.charAt(start);
  if (ch == '"') {
    start++;
    let end = start;
    while (end < json.length) {
      if (json.charAt(end) == '"' && json.charAt(end - 1) != "\\") {
        break;
      }
      end++;
    }
    if (end >= json.length) return "";
    return json.slice(start, end);
  } else {
    let end = start;
    while (end < json.length) {
      const c = json.charCodeAt(end);
      if (
        c == 44 ||
        c == 125 ||
        c == 93 ||
        c == 32 ||
        c == 10 ||
        c == 13 ||
        c == 9
      ) {
        break;
      }
      end++;
    }
    const val = json.slice(start, end).trim();
    if (val == "null") return "";
    return val;
  }
}

export function escapeJson(s: string): string {
  let out = "";
  for (let i = 0; i < s.length; i++) {
    const c = s.charAt(i);
    if (c == '"') {
      out += '\\"';
    } else if (c == '\\') {
      out += "\\\\";
    } else if (c == "\n") {
      out += "\\n";
    } else if (c == "\r") {
      out += "\\r";
    } else if (c == "\t") {
      out += "\\t";
    } else {
      const code = s.charCodeAt(i);
      if (code < 32) {
        out += "\\u00" + (code < 16 ? "0" : "") + code.toString(16);
      } else {
        out += c;
      }
    }
  }
  return out;
}
