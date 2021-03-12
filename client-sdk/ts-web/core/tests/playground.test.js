// import {test} from '@jest/globals';

console.log(TextDecoder);
console.log(crypto);
console.log(require('cborg'));
throw 'bailing'; // %%%

// import * as oasis from './..';
// oh jest is easier with commonjs
const oasis = require('./..');

test('transfer workflow', async () => {
    const src = oasis.signature.EllipticSigner.fromRandom('this key is not important');
    const dst = oasis.signature.EllipticSigner.fromRandom('this key is not important');
    console.log('src', src, 'dst', dst);

    const genesis = await nic.consensusGetGenesisDocument();
    const chainContext = await oasis.genesis.chainContext(genesis);
    console.log('chain context', chainContext);

    const account = await nic.stakingAccount({
        height: oasis.consensus.HEIGHT_LATEST,
        owner: await oasis.staking.addressFromPublicKey(src.public()),
    });
    console.log('account', account);

    const tw = oasis.staking.transferWrapper();
    tw.setNonce(account.general && account.general.nonce || 0);
    tw.setFeeAmount(oasis.quantity.fromBigInt(0n));
    tw.setBody({
        to: await oasis.staking.addressFromPublicKey(dst.public()),
        amount: oasis.quantity.fromBigInt(0n),
    });

    const gas = await tw.estimateGas(nic, src.public());
    console.log('gas', gas);
    tw.setFeeGas(gas);
    console.log('transaction', tw.transaction);

    await tw.sign(new oasis.signature.BlindContextSigner(src), chainContext);
    console.log('singed transaction', tw.signedTransaction);
    console.log('hash', await tw.hash());

    await tw.submit(nic);
    console.log('sent');
});
