import * as hash from './hash';
import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

/**
 * HeightLatest is the height that represents the most recent block height.
 */
export const HEIGHT_LATEST = 0n;
/**
 * SignatureContext is the context used for signing transactions.
 */
export const TRANSACTION_SIGNATURE_CONTEXT = 'oasis-core/consensus: tx';

/**
 * FeatureServices indicates support for communicating with consensus services.
 */
export const FEATURE_SERVICES = 1 << 0;
/**
 * FeatureFullNode indicates that the consensus backend is independently fully verifying all
 * consensus-layer blocks.
 */
export const FEATURE_FULL_NODE = 1 << 1;

/**
 * GasOpTxByte is the gas operation identifier for costing each transaction byte.
 */
export const GAS_OP_TX_BYTE = 'tx_byte';

/**
 * BackendName is the consensus backend name.
 */
export const TENDERMINT_BACKEND_NAME = 'tendermint';

/**
 * moduleName is the module name used for error definitions.
 */
export const MODULE_NAME = 'consensus';
/**
 * ErrNoCommittedBlocks is the error returned when there are no committed
 * blocks and as such no state can be queried.
 */
export const CODE_NO_COMMITTED_BLOCKS = 1;
/**
 * ErrOversizedTx is the error returned when the given transaction is too big to be processed.
 */
export const CODE_OVERSIZED_TX = 2;
/**
 * ErrVersionNotFound is the error returned when the given version (height) cannot be found,
 * possibly because it was pruned.
 */
export const CODE_VERSION_NOT_FOUND = 3;
/**
 * ErrUnsupported is the error returned when the given method is not supported by the consensus
 * backend.
 */
export const CODE_UNSUPPORTED = 4;
/**
 * ErrDuplicateTx is the error returned when the transaction already exists in the mempool.
 */
export const CODE_DUPLICATE_TX = 5;

/**
 * moduleName is the module name used for error definitions.
 */
export const TRANSACTION_MODULE_NAME = 'consensus/transaction';
/**
 * ErrInvalidNonce is the error returned when a nonce is invalid.
 */
export const CODE_INVALID_NONCE = 1;
/**
 * ErrInsufficientFeeBalance is the error returned when there is insufficient
 * balance to pay consensus fees.
 */
export const CODE_INSUFFICIENT_FEE_BALANCE = 2;
/**
 * ErrGasPriceTooLow is the error returned when the gas price is too low.
 */
export const CODE_GAS_PRICE_TOO_LOW = 3;

export async function openSignedTransaction(chainContext: string, signed: types.SignatureSigned) {
    const context = signature.combineChainContext(TRANSACTION_SIGNATURE_CONTEXT, chainContext);
    return misc.fromCBOR(await signature.openSigned(context, signed)) as types.ConsensusTransaction;
}

export async function signSignedTransaction(signer: signature.ContextSigner, chainContext: string, transaction: types.ConsensusTransaction) {
    const context = signature.combineChainContext(TRANSACTION_SIGNATURE_CONTEXT, chainContext);
    return await signature.signSigned(signer, context, misc.toCBOR(transaction));
}

/**
 * This special hex-hash-of-the-CBOR-encoded signed transaction is useful for interoperability
 * with block explorers, so here's a special function for doing it.
 */
export async function hashSignedTransaction(signed: types.SignatureSigned) {
    return misc.toHex(await hash.hash(misc.toCBOR(signed)));
}
