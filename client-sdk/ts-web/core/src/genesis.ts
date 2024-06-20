import * as hash from './hash';
import * as misc from './misc';
import * as types from './types';

export function chainContext(doc: types.GenesisDocument) {
    return misc.toHex(hash.hash(misc.toCBOR(doc)));
}
