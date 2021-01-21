const HEX_DIGITS = '0123456789abcdef';

export function toHex(u8: Uint8Array) {
    let hex = '';
    for (const b of u8) {
        hex += HEX_DIGITS[b >>> 4] + HEX_DIGITS[b & 0xf];
    }
    return hex;
}

export function fromHex(hex: string) {
    let byteLength = hex.length >> 1;
    const u8 = new Uint8Array(byteLength);
    for (let i = 0; i < byteLength; i++) {
        u8[i] = parseInt(hex.substr(2 * i, 2), 16);
    }
    return u8;
}

export function fromString(str: string) {
    return new TextEncoder().encode(str);
}

export function concat(...parts: Uint8Array[]) {
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
