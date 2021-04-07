import * as oasis from '@oasisprotocol/client';

export function toKey(module: string, code: number) {
    const codeBuf = new ArrayBuffer(4);
    const codeDV = new DataView(codeBuf);
    codeDV.setUint32(0, code, false);
    return oasis.misc.concat(oasis.misc.fromString(module), new Uint8Array(codeBuf));
}

export type Handler<V> = (e: oasis.types.RuntimeClientEvent, value: V) => void;
export type ModuleHandler = [module: string, codes: {[code: number]: Handler<unknown>}];

export class Visitor {
    handlers: {[keyHex: string]: Handler<unknown>};

    constructor(modules: ModuleHandler[]) {
        this.handlers = {};
        for (const [module, codes] of modules) {
            for (const code in codes) {
                this.handlers[oasis.misc.toHex(toKey(module, +code))] = codes[code];
            }
        }
    }

    visit(e: oasis.types.RuntimeClientEvent) {
        const keyHex = oasis.misc.toHex(e.key);
        if (keyHex in this.handlers) {
            const value = oasis.misc.fromCBOR(e.value);
            this.handlers[keyHex](e, value);
        }
    }
}
