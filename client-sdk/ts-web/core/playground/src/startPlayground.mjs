// @ts-check

/** @param {import('./../..')} oasis */
export async function startPlayground(oasis) {
    const nic = new oasis.client.NodeInternal('http://[::1]:42280');
    // Try map with non-string keys.
    {
        const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
        console.log('delegations to', toAddr);
        const response = await nic.stakingDelegationsTo({
            height: oasis.consensus.HEIGHT_LATEST,
            owner: oasis.staking.addressFromBech32(toAddr),
        });
        for (const [fromAddr, delegation] of response) {
            console.log({
                from: oasis.staking.addressToBech32(fromAddr),
                shares: oasis.quantity.toBigInt(delegation.shares),
            });
        }
    }
}
