// @ts-expect-error missing declaration
import * as cborg from 'cborg';
import * as nacl from 'tweetnacl';

import * as hash from './hash';
import * as misc from './misc';
import * as types from './types';

export function combineChainContext(context: string, chainContext: string) {
    return `${context} for chain ${chainContext}`;
}

async function prepareSignerMessage(context: string, message: Uint8Array) {
    return hash.hash(misc.concatU8(misc.u8FromStr(context), message));
}

export async function signedOpenRaw(signed: types.SignatureSigned, context: string) {
    const untrustedRawValue = signed.get('untrusted_raw_value');
    const signature = signed.get('signature');
    const signerMessage = await prepareSignerMessage(context, untrustedRawValue);
    const sigOk = nacl.sign.detached.verify(signerMessage, signature.get('signature'), signature.get('public_key'));
    if (!sigOk) throw new Error('signature verification failed');
    return untrustedRawValue;
}

export function deserializeSigned(raw: Uint8Array): types.SignatureSigned {
    return cborg.decode(raw, {useMaps: true});
}
