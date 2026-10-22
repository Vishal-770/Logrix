// Keccak-256 Implementation & Crypto namespace for AssemblyScript.

import { Bytes } from "./types";

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

function keccakF1600(state: u64[]): void {
  const C = new Array<u64>(5);
  const D = new Array<u64>(5);
  const B = new Array<u64>(25);

  for (let round = 0; round < 24; round++) {
    // Theta step
    for (let x = 0; x < 5; x++) {
      C[x] =
        state[x * 5] ^
        state[x * 5 + 1] ^
        state[x * 5 + 2] ^
        state[x * 5 + 3] ^
        state[x * 5 + 4];
    }
    for (let x = 0; x < 5; x++) {
      D[x] = C[(x + 4) % 5] ^ rotl64(C[(x + 1) % 5], 1);
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        state[x * 5 + y] ^= D[x];
      }
    }

    // Rho and Pi steps
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        const idx = x * 5 + y;
        const newX = y;
        const newY = (2 * x + 3 * y) % 5;
        B[newX * 5 + newY] = rotl64(state[idx], KECCAK_R[idx]);
      }
    }

    // Chi step
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        state[x * 5 + y] =
          B[x * 5 + y] ^ (~B[((x + 1) % 5) * 5 + y] & B[((x + 2) % 5) * 5 + y]);
      }
    }

    // Iota step
    state[0] ^= KECCAK_RC[round];
  }
}

export function keccak256Raw(data: Uint8Array): Uint8Array {
  const rate = 136;
  const A = new Array<u64>(25);
  for (let i = 0; i < 25; i++) A[i] = 0;

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
