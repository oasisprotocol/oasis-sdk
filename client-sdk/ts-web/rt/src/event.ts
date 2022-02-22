import * as oasis from '@oasisprotocol/client';

export function toKey(module: string, code: number) {
    const codeBuf = new ArrayBuffer(4);
    const codeDV = new DataView(codeBuf);
    codeDV.setUint32(0, code, false);
    return oasis.misc.concat(oasis.misc.fromString(module), new Uint8Array(codeBuf));
}

export type Handler<V> = (e: oasis.types.RuntimeClientEvent, value: V) => void;
export type ModuleHandler = [module: string, codes: {[code: number]: Handler<never>}];

export class Visitor {
    handlers: {[keyHex: string]: Handler<never>};

    constructor(modules: ModuleHandler[]) {
        this.handlers = {};
        for (const [module, codes] of modules) {
            for (const code in codes) {
                this.handlers[oasis.misc.toHex(toKey(module, +code))] = codes[code];
            }
        }
    }

    /**
     * Calls one of the handlers based on a given event's key.
     * @param e The event
     * @returns `true` if the event matched one of the handlers
     */
    visit(e: oasis.types.RuntimeClientEvent) {
        const keyHex = oasis.misc.toHex(e.key);
        if (this.handlers[keyHex]) {
            const values = oasis.misc.fromCBOR(e.value) as never[];
            for (const value of values) {
                this.handlers[keyHex](e, value);
            }
            return true;
        }
        return false;
    }
}
