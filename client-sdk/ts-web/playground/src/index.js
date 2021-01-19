import * as oasisBridge from './../..';

const client = new oasisBridge.OasisNodeClient('http://localhost:42280');

(async function () {
    try {
        const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
        console.log('delegations to', toAddr);
        const response = await client.stakingDelegations({
            owner: oasisBridge.address.u8FromStr(toAddr),
            height: 1920228,
        });
        for (const [fromAddrU8, delegation] of response) {
            console.log(
                'from', oasisBridge.address.strFromU8(fromAddrU8),
                'shares', oasisBridge.quantity.biFromU8(delegation.get('shares')),
            );
        }
    } catch (e) {
        console.error(e);
    }
})();
