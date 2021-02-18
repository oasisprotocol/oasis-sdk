import * as misc from './misc';

export function toBigInt(q: Uint8Array) {
    if (q.length === 0) return 0n;
    return BigInt('0x' + misc.toHex(q));
}

export function fromBigInt(bi: bigint) {
    if (bi === 0n) return new Uint8Array();
    let hex = bi.toString(16);
    if (hex.length % 2) {
        hex = '0' + hex;
    }
    return misc.fromHex(hex);
}
