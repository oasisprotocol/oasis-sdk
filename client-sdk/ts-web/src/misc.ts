const HEX_DIGITS = '0123456789abcdef';

export function hexFromU8(u8: Uint8Array) {
    let hex = '';
    for (const b of u8) {
        hex += HEX_DIGITS[b >>> 4] + HEX_DIGITS[b & 0xf];
    }
    return hex;
}

export function u8FromHex(hex: string) {
    let byteLength = hex.length >> 1;
    const u8 = new Uint8Array(byteLength);
    for (let i = 0; i < byteLength; i++) {
        u8[i] = parseInt(hex.substr(2 * i, 2), 16);
    }
    return u8;
}

export function u8FromStr(str: string) {
    return new TextEncoder().encode(str);
}

export function concatU8(...parts: Uint8Array[]) {
    let length = 0;
    for (const part of parts) {
        length += part.length;
    }
    let result = new Uint8Array(length);
    let pos = 0;
    for (const part of parts) {
        result.set(part, pos);
        pos += part.length;
    }
    return result;
}
