import * as oasisBridge from './../..';

const client = new oasisBridge.OasisNodeClient('http://localhost:42280');

(async function () {
    try {
        const response = await client.stakingDelegations({
            owner: new Uint8Array([0,127,77,70,174,39,53,254,142,111,175,175,146,245,62,236,64,75,136,212,47]),
            height: 1920228,
        });
        for (const [delegationFrom, delegation] of response) {
            console.log(
                'from', delegationFrom,
                'shares', oasisBridge.quantity.biFromU8(delegation.get('shares')),
            );
        }
    } catch (e) {
        console.error(e);
    }
})();
