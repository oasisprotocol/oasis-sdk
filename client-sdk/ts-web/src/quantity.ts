import * as misc from './misc';

export function toBigInt(q: Uint8Array) {
    return BigInt('0x' + misc.toHex(q));
}

export function fromBigInt(bi: bigint) {
    let hex = bi.toString(16);
    if (hex.length % 2) {
        hex = '0' + hex;
    }
    return misc.fromHex(hex);
}
