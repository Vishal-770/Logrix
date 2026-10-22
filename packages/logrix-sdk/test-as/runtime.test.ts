import {
  BigInt,
  Address,
  Bytes,
  formatUnits,
  escapeJson,
  extractJsonField,
  decodeDynamicString,
  decodeDynamicBytes,
  crypto,
  EventLog,
} from "../index";

// 1. BigInt and sign-aware decimal arithmetic tests
const zero = BigInt.zero();
assert(zero.toString() == "0", "zero must be '0'");

const posA = new BigInt("1000");
const posB = new BigInt("250");
assert(posA.plus(posB).toString() == "1250", "1000 + 250 must be 1250");
assert(posA.minus(posB).toString() == "750", "1000 - 250 must be 750");
assert(posA.times(posB).toString() == "250000", "1000 * 250 must be 250000");
assert(posA.div(posB).toString() == "4", "1000 / 250 must be 4");

// Comparisons
assert(posA.gt(posB) == true, "1000 > 250");
assert(posB.lt(posA) == true, "250 < 1000");
assert(posA.gte(posA) == true, "1000 >= 1000");
assert(posB.lte(posB) == true, "250 <= 250");
assert(posA.equals(posB) == false, "1000 != 250");

// Signed BigInt operations
const negVal = BigInt.fromSignedString("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 256);
assert(negVal.toString() == "-1", "256-bit all 0xff must decode to -1");

const neg500 = new BigInt("-500");
const pos200 = new BigInt("200");
assert(neg500.plus(pos200).toString() == "-300", "-500 + 200 must be -300");
assert(neg500.minus(pos200).toString() == "-700", "-500 - 200 must be -700");
assert(neg500.times(pos200).toString() == "-100000", "-500 * 200 must be -100000");
assert(neg500.div(pos200).toString() == "-2", "-500 / 200 must truncate to -2");

// 2. formatUnits tests
const oneEther = BigInt.fromString("1000000000000000000");
assert(formatUnits(oneEther, 18) == "1", "10^18 with 18 decimals must format as 1");

const halfEther = BigInt.fromString("500000000000000000");
assert(formatUnits(halfEther, 18) == "0.5", "5*10^17 with 18 decimals must format as 0.5");

const negEther = BigInt.fromString("-1500000000000000000");
assert(formatUnits(negEther, 18) == "-1.5", "-1.5*10^18 with 18 decimals must format as -1.5");

// 3. Address normalization tests
const addrUpper = Address.fromString("0X1F98431CAD98523631AE4A59F267346EA31F984");
const addrLower = Address.fromString("0x1f98431cad98523631ae4a59f267346ea31f984");
assert(addrUpper.equals(addrLower) == true, "Address normalization case-insensitive");
assert(addrUpper.toHexString() == "0x1f98431cad98523631ae4a59f267346ea31f984", "Address output lowercase hex");

// 4. String escaping for JSON
const rawStr = "Hello \"World\"\nLine 2\\Backslash";
const escaped = escapeJson(rawStr);
assert(escaped == "Hello \\\"World\\\"\\nLine 2\\\\Backslash", "escapeJson handles quotes, newlines, backslashes");

// 5. extractJsonField
const sampleJson = "{\"name\":\"TokenName\",\"decimals\":18,\"symbol\":\"TKN\"}";
assert(extractJsonField(sampleJson, "name") == "TokenName", "extractJsonField parses string field");
assert(extractJsonField(sampleJson, "decimals") == "18", "extractJsonField parses numeric field");

// 6. Keccak-256
const textBytes = Bytes.fromUTF8("");
const emptyHash = crypto.keccak256(textBytes);
// keccak256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
assert(
  emptyHash.toHexString() == "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
  "Keccak-256 of empty bytes must match standard"
);

export function main(): void {}
