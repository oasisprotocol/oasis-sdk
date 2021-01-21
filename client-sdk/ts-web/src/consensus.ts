// @ts-expect-error missing declaration
import * as cborg from 'cborg';

import * as signature from './signature';
import * as types from './types';

export const TRANSACTION_SIGNATURE_CONTEXT = 'oasis-core/consensus: tx';

export async function signedTransactionOpen(signed: types.SignatureSigned, chainContext: string): Promise<types.ConsensusTransaction> {
    const context = signature.combineChainContext(TRANSACTION_SIGNATURE_CONTEXT, chainContext);
    return cborg.decode(await signature.signedOpenRaw(signed, context), {useMaps: true});
}
