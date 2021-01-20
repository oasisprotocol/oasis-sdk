import * as misc from './misc';

export function biFromU8(u8: Uint8Array) {
    return BigInt('0x' + misc.hexFromU8(u8));
}

export function u8FromBI(bi: bigint) {
    let hex = bi.toString(16);
    if (hex.length % 2) {
        hex = '0' + hex;
    }
    return misc.u8FromHex(hex);
}
