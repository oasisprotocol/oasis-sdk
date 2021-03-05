import * as oasis from '@oasisprotocol/client';

export function toTag(module: string, code: number) {
    const codeBuf = new ArrayBuffer(4);
    const codeDV = new DataView(codeBuf);
    codeDV.setUint32(0, code, false);
    return oasis.misc.concat(oasis.misc.fromString(module), new Uint8Array(codeBuf));
}
