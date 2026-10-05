// AssemblyScript runtime library for Logrix blockchain indexers.
// Import this in your handler files: import { store, log, BigInt, Address, Bytes, crypto, formatUnits } from "@logrix/sdk"

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

@external("env", "logrix_db_remove")
declare function host_db_remove(
  key_ptr: i32,
  key_len: i32
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

export function logrix_db_remove(key: string): void {
  const kBuf = String.UTF8.encode(key);
  host_db_remove(changetype<i32>(kBuf), kBuf.byteLength);
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
    logrix_db_remove(entityType + ":" + id);
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

function parseHexChar(c: i32): i32 {
  if (c >= 48 && c <= 57) return c - 48;
  if (c >= 97 && c <= 102) return c - 97 + 10;
  if (c >= 65 && c <= 70) return c - 65 + 10;
  return 0;
}

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

// ---------------------------------------------------------------------------
// Keccak-256 Implementation & Crypto namespace
// ---------------------------------------------------------------------------

const KECCAK_RC: u64[] = [
  0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
  0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
  0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
  0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
  0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
  0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008
];

const KECCAK_R: i32[] = [
  0, 36, 3, 41, 18,
  1, 44, 10, 45, 2,
  62, 6, 43, 15, 61,
  28, 55, 25, 21, 56,
  27, 20, 39, 8, 14
];

function rotl64(x: u64, n: i32): u64 {
  const shift = (n as u64) & 63;
  if (shift == 0) return x;
  return (x << shift) | (x >> (64 - shift));
}

function keccakF1600(A: u64[]): void {
  const B = new Array<u64>(25);
  for (let i = 0; i < 25; i++) B[i] = 0;
  const C = new Array<u64>(5);
  for (let i = 0; i < 5; i++) C[i] = 0;
  const D = new Array<u64>(5);
  for (let i = 0; i < 5; i++) D[i] = 0;

  for (let ir = 0; ir < 24; ir++) {
    for (let x = 0; x < 5; x++) {
      C[x] = A[x * 5 + 0] ^ A[x * 5 + 1] ^ A[x * 5 + 2] ^ A[x * 5 + 3] ^ A[x * 5 + 4];
    }
    for (let x = 0; x < 5; x++) {
      D[x] = C[(x + 4) % 5] ^ rotl64(C[(x + 1) % 5], 1);
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        A[x * 5 + y] ^= D[x];
      }
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        B[y * 5 + ((2 * x + 3 * y) % 5)] = rotl64(A[x * 5 + y], KECCAK_R[x * 5 + y]);
      }
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        const b1 = B[((x + 1) % 5) * 5 + y];
        const b2 = B[((x + 2) % 5) * 5 + y];
        A[x * 5 + y] = B[x * 5 + y] ^ ((~b1) & b2);
      }
    }
    A[0] ^= KECCAK_RC[ir];
  }
}

function keccak256Raw(data: Uint8Array): Uint8Array {
  const A = new Array<u64>(25);
  for (let i = 0; i < 25; i++) A[i] = 0;

  const rate = 136;
  let offset = 0;
  const len = data.length;

  while (offset + rate <= len) {
    for (let i = 0; i < 17; i++) {
      const x = i % 5;
      const y = i / 5;
      let w: u64 = 0;
      const base = offset + i * 8;
      for (let b = 0; b < 8; b++) {
        w |= (data[base + b] as u64) << (b * 8);
      }
      A[x * 5 + y] ^= w;
    }
    keccakF1600(A);
    offset += rate;
  }

  const remaining = len - offset;
  const pad = new Uint8Array(rate);
  for (let i = 0; i < remaining; i++) {
    pad[i] = data[offset + i];
  }
  pad[remaining] = 0x01;
  pad[rate - 1] |= 0x80;

  for (let i = 0; i < 17; i++) {
    const x = i % 5;
    const y = i / 5;
    let w: u64 = 0;
    const base = i * 8;
    for (let b = 0; b < 8; b++) {
      w |= (pad[base + b] as u64) << (b * 8);
    }
    A[x * 5 + y] ^= w;
  }
  keccakF1600(A);

  const out = new Uint8Array(32);
  for (let i = 0; i < 4; i++) {
    const x = i % 5;
    const y = i / 5;
    const w = A[x * 5 + y];
    const base = i * 8;
    for (let b = 0; b < 8; b++) {
      out[base + b] = ((w >> (b * 8)) & 0xff) as u8;
    }
  }
  return out;
}

export namespace crypto {
  export function keccak256(data: Bytes): Bytes {
    const raw = data.toByteArray();
    const digest = keccak256Raw(raw);
    return Bytes.fromByteArray(digest);
  }
}

// ---------------------------------------------------------------------------
// Arbitrary-Precision BigInt Type (Supports Hex & Decimal, EVM 256-bit safe)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// formatUnits Helper (EVM Decimal Formatting)
// ---------------------------------------------------------------------------

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
    while (end > dotIdx + 1 && s.charAt(end) == "0") {
      end--;
    }
    s = s.slice(0, end + 1);
  }
  return isNegative ? "-" + s : s;
}

// ---------------------------------------------------------------------------
// Internal Arbitrary-Precision Decimal Helpers
// ---------------------------------------------------------------------------

function addDecimal(a: string, b: string): string {
  let i = a.length - 1;
  let j = b.length - 1;
  let carry = 0;
  let res = "";

  while (i >= 0 || j >= 0 || carry > 0) {
    let sum = carry;
    if (i >= 0) sum += a.charCodeAt(i) - 48;
    if (j >= 0) sum += b.charCodeAt(j) - 48;
    res = (sum % 10).toString() + res;
    carry = sum / 10;
    i--;
    j--;
  }
  return res.length > 0 ? res : "0";
}

function cmpDecimal(a: string, b: string): i32 {
  if (a.length > b.length) return 1;
  if (a.length < b.length) return -1;
  for (let i = 0; i < a.length; i++) {
    const c1 = a.charCodeAt(i);
    const c2 = b.charCodeAt(i);
    if (c1 > c2) return 1;
    if (c1 < c2) return -1;
  }
  return 0;
}

function subDecimal(a: string, b: string): string {
  const cmp = cmpDecimal(a, b);
  if (cmp == 0) return "0";
  if (cmp < 0) return "-" + subDecimal(b, a);

  let i = a.length - 1;
  let j = b.length - 1;
  let borrow = 0;
  let res = "";

  while (i >= 0) {
    let d1 = a.charCodeAt(i) - 48 - borrow;
    let d2 = j >= 0 ? b.charCodeAt(j) - 48 : 0;
    if (d1 < d2) {
      d1 += 10;
      borrow = 1;
    } else {
      borrow = 0;
    }
    res = (d1 - d2).toString() + res;
    i--;
    j--;
  }

  let start = 0;
  while (start < res.length - 1 && res.charAt(start) == "0") {
    start++;
  }
  return res.slice(start);
}

function mulDecimal(a: string, b: string): string {
  if (a == "0" || b == "0") return "0";
  const lenA = a.length;
  const lenB = b.length;
  const result = new Array<i32>(lenA + lenB);
  for (let idx = 0; idx < lenA + lenB; idx++) result[idx] = 0;

  for (let i = lenA - 1; i >= 0; i--) {
    for (let j = lenB - 1; j >= 0; j--) {
      const mul = (a.charCodeAt(i) - 48) * (b.charCodeAt(j) - 48);
      const sum = mul + result[i + j + 1];
      result[i + j + 1] = sum % 10;
      result[i + j] += sum / 10;
    }
  }

  let res = "";
  let i = 0;
  while (i < result.length && result[i] == 0) i++;
  while (i < result.length) {
    res += result[i].toString();
    i++;
  }
  return res.length > 0 ? res : "0";
}

function divDecimal(a: string, b: string): string {
  if (b == "0") return "0";
  if (cmpDecimal(a, b) < 0) return "0";
  if (cmpDecimal(a, b) == 0) return "1";

  let quotient = "";
  let remainder = "";

  for (let i = 0; i < a.length; i++) {
    remainder += a.charAt(i);
    let start = 0;
    while (start < remainder.length - 1 && remainder.charAt(start) == "0") {
      start++;
    }
    remainder = remainder.slice(start);

    let count = 0;
    while (cmpDecimal(remainder, b) >= 0) {
      remainder = subDecimal(remainder, b);
      count++;
    }
    quotient += count.toString();
  }

  let start = 0;
  while (start < quotient.length - 1 && quotient.charAt(start) == "0") {
    start++;
  }
  return quotient.slice(start);
}

function decimalToHex(dec: string): string {
  if (dec == "0" || dec.length == 0) return "0";
  let hex = "";
  let current = dec;
  const hexChars = "0123456789abcdef";

  while (current != "0" && current.length > 0) {
    let next = "";
    let rem = 0;
    for (let i = 0; i < current.length; i++) {
      const cur = rem * 10 + (current.charCodeAt(i) - 48);
      const div = cur / 16;
      rem = cur % 16;
      if (next.length > 0 || div > 0) {
        next += div.toString();
      }
    }
    hex = hexChars.charAt(rem) + hex;
    current = next.length > 0 ? next : "0";
  }
  return hex.length > 0 ? hex : "0";
}

function hexToDecimal(hexStr: string): string {
  let hex = hexStr.trim().toLowerCase();
  if (hex.startsWith("0x")) hex = hex.slice(2);
  if (hex.length == 0) return "0";

  let dec = "0";
  for (let i = 0; i < hex.length; i++) {
    const c = hex.charCodeAt(i);
    let digit = 0;
    if (c >= 48 && c <= 57) {
      digit = c - 48;
    } else if (c >= 97 && c <= 102) {
      digit = c - 97 + 10;
    } else if (c >= 65 && c <= 70) {
      digit = c - 65 + 10;
    } else {
      continue;
    }

    let carry = digit;
    let nextDec = "";
    for (let j = dec.length - 1; j >= 0; j--) {
      const prod = (dec.charCodeAt(j) - 48) * 16 + carry;
      nextDec = (prod % 10).toString() + nextDec;
      carry = prod / 10;
    }
    while (carry > 0) {
      nextDec = (carry % 10).toString() + nextDec;
      carry = carry / 10;
    }
    dec = nextDec.length > 0 ? nextDec : "0";
  }

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
