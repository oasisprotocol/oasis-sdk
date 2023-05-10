// @ts-check

/** @param {import('./../..')} oasis */
export async function startPlayground(oasis) {
    const nic = new oasis.client.NodeInternal('http://localhost:42280');
    const msgs = []
    // Wait for ready.
    {
        msgs.push('waiting for node to be ready');
        const waitStart = Date.now();
        await nic.nodeControllerWaitReady();
        const waitEnd = Date.now();
        msgs.push(`ready ${waitEnd - waitStart} ms`);
    }

    // Get something with addresses.
    {
        msgs.push('nodes', await nic.registryGetNodes(oasis.consensus.HEIGHT_LATEST));
    }
    // Try map with non-string keys.
    {
        const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
        msgs.push('delegations to', toAddr);
        const response = await nic.stakingDelegationsTo({
            height: oasis.consensus.HEIGHT_LATEST,
            owner: oasis.staking.addressFromBech32(toAddr),
        });
        for (const [fromAddr, delegation] of response) {
            msgs.push({
                from: oasis.staking.addressToBech32(fromAddr),
                shares: oasis.quantity.toBigInt(delegation.shares),
            });
        }
    }
    return msgs
}
