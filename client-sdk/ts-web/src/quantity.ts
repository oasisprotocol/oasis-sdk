const HEX_DIGITS = '0123456789abcdef';

export function biFromU8(u8: Uint8Array) {
    let hex = '0x';
    for (const b of u8) {
        hex += HEX_DIGITS[b >>> 4] + HEX_DIGITS[b & 0xf];
    }
    return BigInt(hex);
}

export function u8FromBI(bi: bigint) {
    let hex = bi.toString(16);
    let byteLength = (hex.length + 1) >> 1;
    const u8 = new Uint8Array(byteLength);
    for (let i = 0; i < byteLength; i++) {
        const startOffset = -2 * (byteLength - i);
        const endOffset = startOffset + hex.length + 2;
        u8[i] = parseInt(hex.slice(startOffset, endOffset), 16);
    }
    return u8;
}
