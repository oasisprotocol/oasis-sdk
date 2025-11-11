import {KeyKind, RoflClient} from '../src';

async function main() {
    const client = new RoflClient();

    try {
        const key = await client.generateKey('my-first-key');
        console.log('Generated SECP256K1 key:', key);

        const ed25519 = await client.generateKey('my-ed25519-key', KeyKind.ED25519);
        console.log('Generated Ed25519 key:', ed25519);

        const entropy = await client.generateKey('my-entropy', KeyKind.RAW_256);
        console.log('Generated 256-bit entropy:', entropy);

        console.log('\nPublishing metadata...');
        await client.setMetadata({key_fingerprint: key.slice(0, 16)});
        console.log('Metadata set successfully');

        const metadata = await client.getMetadata();
        console.log('Current metadata:', metadata);

        const appId = await client.getAppId();
        console.log('App ID:', appId);

        // Read-only runtime query (no args -> CBOR null)
        const runtimeInfo = await client.query('core.RuntimeInfo');
        console.log('Runtime info:', runtimeInfo);

        // Sign & submit (ETH style)
        const result = await client.signAndSubmit({
            kind: 'eth',
            gas_limit: 200_000,
            to: '', // empty => contract creation
            value: '0',
            data: '0x', // no-op calldata
        });
        console.log('CallResult (hex):', Buffer.from(result).toString('hex'));
    } catch (err) {
        console.log('Note: Operations require a running ROFL service');
        console.error(err);
    }
}

void main();
