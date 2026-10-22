// Dynamic ABI Offset Decoding Helpers (Solidity Head/Tail)

import { parseHexChar, parseHexI32 } from "./math";
import { Bytes } from "./types";

/**
 * Decodes a dynamic ABI string from raw log event data at paramIndex.
 */
export function decodeDynamicString(dataHex: string, paramIndex: i32): string {
  let clean = dataHex.startsWith("0x") ? dataHex.slice(2) : dataHex;
  const headOffset = paramIndex * 64;
  if (clean.length < headOffset + 64) return "";
  const ptrWord = clean.slice(headOffset, headOffset + 64);
  const offsetBytes = parseHexI32(ptrWord);
  const offsetHex = offsetBytes * 2;
  if (clean.length < offsetHex + 64) return "";
  const lenWord = clean.slice(offsetHex, offsetHex + 64);
  const byteLen = parseHexI32(lenWord);
  const contentHexStart = offsetHex + 64;
  const contentHexEnd = contentHexStart + byteLen * 2;
  if (clean.length < contentHexEnd) return "";
  const contentHex = clean.slice(contentHexStart, contentHexEnd);
  const bytes = new Uint8Array(byteLen);
  for (let i = 0; i < byteLen; i++) {
    const high = parseHexChar(contentHex.charCodeAt(i * 2));
    const low = parseHexChar(contentHex.charCodeAt(i * 2 + 1));
    bytes[i] = ((high << 4) | low) as u8;
  }
  return String.UTF8.decode(bytes.buffer);
}

/**
 * Decodes dynamic ABI bytes from raw log event data at paramIndex.
 */
export function decodeDynamicBytes(dataHex: string, paramIndex: i32): Bytes {
  let clean = dataHex.startsWith("0x") ? dataHex.slice(2) : dataHex;
  const headOffset = paramIndex * 64;
  if (clean.length < headOffset + 64) return Bytes.fromHexString("0x");
  const ptrWord = clean.slice(headOffset, headOffset + 64);
  const offsetBytes = parseHexI32(ptrWord);
  const offsetHex = offsetBytes * 2;
  if (clean.length < offsetHex + 64) return Bytes.fromHexString("0x");
  const lenWord = clean.slice(offsetHex, offsetHex + 64);
  const byteLen = parseHexI32(lenWord);
  const contentHexStart = offsetHex + 64;
  const contentHexEnd = contentHexStart + byteLen * 2;
  if (clean.length < contentHexEnd) return Bytes.fromHexString("0x");
  const contentHex = clean.slice(contentHexStart, contentHexEnd);
  return Bytes.fromHexString("0x" + contentHex);
}
