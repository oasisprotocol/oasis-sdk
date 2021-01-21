// @ts-expect-error missing declaration
import * as cborg from 'cborg';

import * as hash from './hash';
import * as misc from './misc';
import * as types from './types';

export async function chainContext(doc: types.GenesisDocument) {
    return misc.toHex(await hash.hash(cborg.encode(doc)));
}
