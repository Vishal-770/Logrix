// Arbitrary-precision decimal arithmetic and hex conversion helpers.

export function parseHexChar(c: i32): i32 {
  if (c >= 48 && c <= 57) return c - 48;
  if (c >= 97 && c <= 102) return c - 97 + 10;
  if (c >= 65 && c <= 70) return c - 65 + 10;
  return 0;
}

export function parseHexI32(hex: string): i32 {
  let val: i32 = 0;
  for (let i = 0; i < hex.length; i++) {
    val = (val << 4) | parseHexChar(hex.charCodeAt(i));
  }
  return val;
}

export function parseU64(str: string): u64 {
  let s = str.trim();
  if (s.startsWith("0x") || s.startsWith("0X")) {
    s = s.slice(2);
    let val: u64 = 0;
    for (let i = 0; i < s.length; i++) {
      const c = s.charCodeAt(i);
      let d: u64 = 0;
      if (c >= 48 && c <= 57) d = (c - 48) as u64;
      else if (c >= 97 && c <= 102) d = (c - 97 + 10) as u64;
      else if (c >= 65 && c <= 70) d = (c - 65 + 10) as u64;
      else unreachable();
      val = (val << 4) | d;
    }
    return val;
  }
  let val: u64 = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c >= 48 && c <= 57) {
      val = val * 10 + ((c - 48) as u64);
    } else {
      unreachable();
    }
  }
  return val;
}

export function addUnsigned(a: string, b: string): string {
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

export function cmpUnsigned(a: string, b: string): i32 {
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

export function subUnsigned(a: string, b: string): string {
  const cmp = cmpUnsigned(a, b);
  if (cmp == 0) return "0";
  if (cmp < 0) return "-" + subUnsigned(b, a);

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

export function mulUnsigned(a: string, b: string): string {
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

export function divUnsigned(a: string, b: string): string {
  if (cmpUnsigned(a, b) < 0) return "0";
  if (cmpUnsigned(a, b) == 0) return "1";

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
    while (cmpUnsigned(remainder, b) >= 0) {
      remainder = subUnsigned(remainder, b);
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

export function cmpDecimal(a: string, b: string): i32 {
  if (a == b) return 0;
  const aNeg = a.startsWith("-");
  const bNeg = b.startsWith("-");
  if (aNeg && !bNeg) return -1;
  if (!aNeg && bNeg) return 1;
  if (aNeg && bNeg) {
    return -cmpUnsigned(a.slice(1), b.slice(1));
  }
  return cmpUnsigned(a, b);
}

export function addDecimal(a: string, b: string): string {
  const aNeg = a.startsWith("-");
  const bNeg = b.startsWith("-");

  if (aNeg && bNeg) {
    const sum = addUnsigned(a.slice(1), b.slice(1));
    return sum == "0" ? "0" : "-" + sum;
  }
  if (aNeg && !bNeg) {
    const aAbs = a.slice(1);
    const cmp = cmpUnsigned(b, aAbs);
    if (cmp == 0) return "0";
    if (cmp > 0) return subUnsigned(b, aAbs);
    return "-" + subUnsigned(aAbs, b);
  }
  if (!aNeg && bNeg) {
    const bAbs = b.slice(1);
    const cmp = cmpUnsigned(a, bAbs);
    if (cmp == 0) return "0";
    if (cmp > 0) return subUnsigned(a, bAbs);
    return "-" + subUnsigned(bAbs, a);
  }
  return addUnsigned(a, b);
}

export function subDecimal(a: string, b: string): string {
  const bNegated = b.startsWith("-") ? b.slice(1) : (b == "0" ? "0" : "-" + b);
  return addDecimal(a, bNegated);
}

export function mulDecimal(a: string, b: string): string {
  if (a == "0" || b == "0") return "0";
  const aNeg = a.startsWith("-");
  const bNeg = b.startsWith("-");
  const aAbs = aNeg ? a.slice(1) : a;
  const bAbs = bNeg ? b.slice(1) : b;
  const prod = mulUnsigned(aAbs, bAbs);
  if (prod == "0") return "0";
  return (aNeg != bNeg) ? "-" + prod : prod;
}

export function divDecimal(a: string, b: string): string {
  const aNeg = a.startsWith("-");
  const bNeg = b.startsWith("-");
  const aAbs = aNeg ? a.slice(1) : a;
  const bAbs = bNeg ? b.slice(1) : b;

  if (bAbs == "0" || bAbs.length == 0) {
    unreachable(); // EVM division by zero must trap
  }
  const quot = divUnsigned(aAbs, bAbs);
  if (quot == "0") return "0";
  return (aNeg != bNeg) ? "-" + quot : quot;
}

export function decimalToHexUnsigned(dec: string): string {
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

export function decimalToHex(dec: string): string {
  if (dec.startsWith("-")) {
    const hex = decimalToHexUnsigned(dec.slice(1));
    return hex == "0" ? "0" : "-" + hex;
  }
  return decimalToHexUnsigned(dec);
}

export function hexToDecimal(hexStr: string): string {
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
      unreachable(); // Strictly reject invalid hex character
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

export function hexToSignedDecimal(hexStr: string, bitWidth: i32 = 256): string {
  let clean = hexStr.trim().toLowerCase();
  if (clean.startsWith("0x")) clean = clean.slice(2);
  if (clean.length == 0) return "0";

  const hexChars = (bitWidth + 3) / 4;
  while (clean.length < hexChars) {
    clean = "0" + clean;
  }
  if (clean.length > hexChars) {
    clean = clean.slice(clean.length - hexChars);
  }

  const firstNibble = clean.charCodeAt(0);
  let isNegative = false;
  if (
    (firstNibble >= 56 && firstNibble <= 57) ||
    (firstNibble >= 97 && firstNibble <= 102)
  ) {
    isNegative = true;
  }

  if (!isNegative) {
    return hexToDecimal(clean);
  }

  let invHex = "";
  for (let i = 0; i < clean.length; i++) {
    const c = clean.charCodeAt(i);
    let d = 0;
    if (c >= 48 && c <= 57) d = c - 48;
    else if (c >= 97 && c <= 102) d = c - 97 + 10;
    else unreachable();
    const inv = 15 - d;
    const hexCharsTable = "0123456789abcdef";
    invHex += hexCharsTable.charAt(inv);
  }

  const invDec = hexToDecimal(invHex);
  const magnitude = addDecimal(invDec, "1");
  return "-" + magnitude;
}
