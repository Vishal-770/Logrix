// AssemblyScript runtime library for Logrix blockchain indexers.
// Import this in your handler files: import { store, log, BigInt, Address, Bytes, ... } from "@logrix/sdk"

// ---------------------------------------------------------------------------
// Host function declarations
// The Logrix Rust engine registers these in both "env" and "logrix" namespaces.
// ---------------------------------------------------------------------------

@external("env", "logrix_emit")
declare function host_emit(
  entity_ptr: i32,
  entity_len: i32,
  val_ptr: i32,
  val_len: i32
): i32;

@external("env", "logrix_db_get")
declare function host_db_get(
  key_ptr: i32,
  key_len: i32,
  out_ptr: i32,
  max_out_len: i32
): i32;

@external("env", "logrix_db_set")
declare function host_db_set(
  key_ptr: i32,
  key_len: i32,
  val_ptr: i32,
  val_len: i32
): i32;

@external("env", "logrix_log")
declare function host_log(level: i32, msg_ptr: i32, msg_len: i32): i32;

// ---------------------------------------------------------------------------
// Memory allocator export
// ---------------------------------------------------------------------------

export function allocate(size: i32): i32 {
  return heap.alloc(size) as i32;
}

// ---------------------------------------------------------------------------
// Raw blockchain event log payload delivered to indexer handlers.
// ---------------------------------------------------------------------------

export class EventLog {
  address: string = "";
  block_number: u64 = 0;
  block_hash: string = "";
  transaction_hash: string = "";
  log_index: u32 = 0;
  block_timestamp: u64 = 0;
  topics: Array<string> = [];
  data: string = "";
}

// ---------------------------------------------------------------------------
// Public Low-Level Host API
// ---------------------------------------------------------------------------

export function logrix_emit(entityType: string, jsonPayload: string): void {
  const eBuf = String.UTF8.encode(entityType);
  const pBuf = String.UTF8.encode(jsonPayload);
  host_emit(
    changetype<i32>(eBuf),
    eBuf.byteLength,
    changetype<i32>(pBuf),
    pBuf.byteLength
  );
}

export function logrix_db_get(key: string): string {
  const kBuf = String.UTF8.encode(key);
  const outBuf = new ArrayBuffer(8192);
  const written = host_db_get(
    changetype<i32>(kBuf),
    kBuf.byteLength,
    changetype<i32>(outBuf),
    outBuf.byteLength
  );
  if (written <= 0) return "";
  return String.UTF8.decode(outBuf.slice(0, written));
}

export function logrix_db_set(key: string, value: string): void {
  const kBuf = String.UTF8.encode(key);
  const vBuf = String.UTF8.encode(value);
  host_db_set(
    changetype<i32>(kBuf),
    kBuf.byteLength,
    changetype<i32>(vBuf),
    vBuf.byteLength
  );
}

export function logrix_log(level: i32, message: string): void {
  const mBuf = String.UTF8.encode(message);
  host_log(level, changetype<i32>(mBuf), mBuf.byteLength);
}

// ---------------------------------------------------------------------------
// Store namespace (Graph Protocol compatible Entity store)
// ---------------------------------------------------------------------------

export namespace store {
  export function get(entityType: string, id: string): string {
    return logrix_db_get(entityType + ":" + id);
  }

  export function set(entityType: string, id: string, jsonPayload: string): void {
    logrix_db_set(entityType + ":" + id, jsonPayload);
    logrix_emit(entityType, jsonPayload);
  }

  export function remove(entityType: string, id: string): void {
    logrix_db_set(entityType + ":" + id, "");
    logrix_emit(entityType, "{\"id\":\"" + id + "\",\"_deleted\":true}");
  }
}

// ---------------------------------------------------------------------------
// Log namespace (Graph Protocol compatible logging)
// ---------------------------------------------------------------------------

export namespace log {
  export function debug(message: string): void {
    logrix_log(0, message);
  }

  export function info(message: string): void {
    logrix_log(1, message);
  }

  export function warning(message: string): void {
    logrix_log(2, message);
  }

  export function error(message: string): void {
    logrix_log(3, message);
  }
}

// ---------------------------------------------------------------------------
// Address Type
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Bytes Type
// ---------------------------------------------------------------------------

export class Bytes {
  private _hex: string;

  constructor(hex: string) {
    let clean = hex.trim().toLowerCase();
    if (clean.startsWith("0x")) clean = clean.slice(2);
    this._hex = "0x" + clean;
  }

  static fromHexString(hex: string): Bytes {
    return new Bytes(hex);
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

// ---------------------------------------------------------------------------
// Arbitrary-Precision BigInt Type (Supports Hex & Decimal, EVM 256-bit safe)
// ---------------------------------------------------------------------------

export class BigInt {
  private _raw: string;

  constructor(val: string) {
    let clean = val.trim();
    if (clean.length == 0) clean = "0";

    // Handle Hex values (e.g. from RPC event logs or ABI words)
    if (clean.startsWith("0x") || clean.startsWith("0X")) {
      clean = hexToDecimal(clean.slice(2));
    }

    this._raw = clean;
  }

  static fromString(val: string): BigInt {
    return new BigInt(val);
  }

  static fromU64(val: u64): BigInt {
    return new BigInt(val.toString());
  }

  static fromI32(val: i32): BigInt {
    return new BigInt(val.toString());
  }

  static zero(): BigInt {
    return new BigInt("0");
  }

  isZero(): bool {
    return this._raw == "0" || this._raw.length == 0;
  }

  toString(): string {
    return this._raw;
  }

  toU64(): u64 {
    return u64(parseInt(this._raw) as i64);
  }

  toI32(): i32 {
    return parseInt(this._raw) as i32;
  }

  plus(other: BigInt): BigInt {
    let a = this._raw;
    let b = other._raw;
    let i = a.length - 1;
    let j = b.length - 1;
    let carry = 0;
    let res = "";
    while (i >= 0 || j >= 0 || carry > 0) {
      let sum = carry;
      if (i >= 0) {
        sum += a.charCodeAt(i) - 48;
        i--;
      }
      if (j >= 0) {
        sum += b.charCodeAt(j) - 48;
        j--;
      }
      carry = sum >= 10 ? 1 : 0;
      res = (sum % 10).toString() + res;
    }
    return new BigInt(res.length > 0 ? res : "0");
  }

  minus(other: BigInt): BigInt {
    let a = this._raw;
    let b = other._raw;
    let cmp = compareDecimalStrings(a, b);
    if (cmp == 0) return BigInt.zero();
    if (cmp < 0) return BigInt.zero();

    let i = a.length - 1;
    let j = b.length - 1;
    let borrow = 0;
    let res = "";
    while (i >= 0) {
      let digitA = a.charCodeAt(i) - 48 - borrow;
      let digitB = j >= 0 ? b.charCodeAt(j) - 48 : 0;
      j--;
      i--;
      if (digitA < digitB) {
        digitA += 10;
        borrow = 1;
      } else {
        borrow = 0;
      }
      res = (digitA - digitB).toString() + res;
    }
    let start = 0;
    while (start < res.length - 1 && res.charAt(start) == "0") {
      start++;
    }
    return new BigInt(res.slice(start));
  }

  times(other: BigInt): BigInt {
    let a = this._raw;
    let b = other._raw;
    if (a == "0" || b == "0") return BigInt.zero();
    let lenA = a.length;
    let lenB = b.length;
    let result = new Array<i32>(lenA + lenB);
    for (let k = 0; k < lenA + lenB; k++) {
      result[k] = 0;
    }
    for (let i = lenA - 1; i >= 0; i--) {
      let da = a.charCodeAt(i) - 48;
      for (let j = lenB - 1; j >= 0; j--) {
        let db = b.charCodeAt(j) - 48;
        let mul = da * db;
        let p2 = i + j + 1;
        let p1 = i + j;
        let sum = mul + result[p2];
        result[p2] = sum % 10;
        result[p1] += sum / 10;
      }
    }
    let str = "";
    let started = false;
    for (let k = 0; k < result.length; k++) {
      if (result[k] != 0 || started) {
        started = true;
        str += result[k].toString();
      }
    }
    return new BigInt(str.length == 0 ? "0" : str);
  }

  div(other: BigInt): BigInt {
    if (other.isZero()) return BigInt.zero();
    let a = this.toU64();
    let b = other.toU64();
    if (b == 0) return BigInt.zero();
    return BigInt.fromU64(a / b);
  }

  equals(other: BigInt): bool {
    return this._raw == other._raw;
  }

  gt(other: BigInt): bool {
    return compareDecimalStrings(this._raw, other._raw) > 0;
  }

  lt(other: BigInt): bool {
    return compareDecimalStrings(this._raw, other._raw) < 0;
  }

  gte(other: BigInt): bool {
    return compareDecimalStrings(this._raw, other._raw) >= 0;
  }

  lte(other: BigInt): bool {
    return compareDecimalStrings(this._raw, other._raw) <= 0;
  }
}

function compareDecimalStrings(a: string, b: string): i32 {
  if (a.length > b.length) return 1;
  if (a.length < b.length) return -1;
  for (let i = 0; i < a.length; i++) {
    let ca = a.charCodeAt(i);
    let cb = b.charCodeAt(i);
    if (ca > cb) return 1;
    if (ca < cb) return -1;
  }
  return 0;
}

function hexToDecimal(hex: string): string {
  if (hex.length == 0) return "0";
  let dec = "0";

  for (let i = 0; i < hex.length; i++) {
    let c = hex.charCodeAt(i);
    let digit = 0;
    if (c >= 48 && c <= 57) {
      digit = c - 48; // 0-9
    } else if (c >= 97 && c <= 102) {
      digit = c - 97 + 10; // a-f
    } else if (c >= 65 && c <= 70) {
      digit = c - 65 + 10; // A-F
    } else {
      continue;
    }

    // dec = dec * 16 + digit
    let carry = digit;
    let nextDec = "";
    for (let j = dec.length - 1; j >= 0; j--) {
      let prod = (dec.charCodeAt(j) - 48) * 16 + carry;
      nextDec = (prod % 10).toString() + nextDec;
      carry = prod / 10;
    }
    while (carry > 0) {
      nextDec = (carry % 10).toString() + nextDec;
      carry = carry / 10;
    }
    dec = nextDec.length > 0 ? nextDec : "0";
  }

  // Remove leading zeros
  let start = 0;
  while (start < dec.length - 1 && dec.charAt(start) == "0") {
    start++;
  }
  return dec.slice(start);
}

// ---------------------------------------------------------------------------
// JSON Parsing Helper for Entity.load()
// ---------------------------------------------------------------------------

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
