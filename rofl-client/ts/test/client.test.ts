import {
    KeyKind,
    RoflClient,
    type RoflClientOptions,
    type TransportRequest,
    type TransportResponse,
} from '../src/roflClient';
import {
    RoflClient as PublicRoflClient,
    KeyKind as PublicKeyKind,
    ROFL_SOCKET_PATH as PublicSocketPath,
} from '../src/index';

function makeClient(collector: TransportRequest[]): RoflClient {
    const transport = async (req: TransportRequest): Promise<TransportResponse> => {
        collector.push(req);
        if (req.path === '/rofl/v1/keys/generate' && req.method === 'POST') {
            return {status: 200, body: {key: 'abcdef'}, headers: {}};
        }
        if (req.path === '/rofl/v1/metadata' && req.method === 'GET') {
            return {status: 200, body: {k1: 'v1'}, headers: {'content-type': 'application/json'}};
        }
        if (req.path === '/rofl/v1/metadata' && req.method === 'POST') {
            return {status: 200, body: {}, headers: {}};
        }
        if (req.path === '/rofl/v1/app/id' && req.method === 'GET') {
            return {
                status: 200,
                body: 'rofl1qqqqqqqqqqqqqqqqqqqqqqqqq',
                headers: {'content-type': 'text/plain'},
            };
        }
        if (req.path === '/rofl/v1/tx/sign-submit' && req.method === 'POST') {
            return {
                status: 200,
                body: {data: 'a1626f6b40'}, // {"ok": ''} in CBOR as per docs
                headers: {'content-type': 'application/json'},
            };
        }
        return {status: 404, body: 'not found', headers: {}};
    };
    const opts: RoflClientOptions = {transport};
    return new RoflClient(opts);
}

describe('RoflClient', () => {
    it('KeyKind string values are stable', () => {
        expect(KeyKind.RAW_256).toBe('raw-256');
        expect(KeyKind.RAW_384).toBe('raw-384');
        expect(KeyKind.ED25519).toBe('ed25519');
        expect(KeyKind.SECP256K1).toBe('secp256k1');
    });

    it('generates key with defaults (UDS) and sets canonical headers + User-Agent', async () => {
        const calls: TransportRequest[] = [];
        const client = makeClient(calls);
        const key = await client.generateKey('my-key');
        expect(key).toBe('abcdef');

        expect(calls).toHaveLength(1);
        expect(calls[0].method).toBe('POST');
        expect(calls[0].path).toBe('/rofl/v1/keys/generate');
        expect(calls[0].socketPath).toBe('/run/rofl-appd.sock');
        expect(calls[0].baseUrl).toBeUndefined();
        expect(calls[0].payload).toEqual({key_id: 'my-key', kind: KeyKind.SECP256K1});

        const headers = calls[0].headers!;
        expect(headers['Content-Type']).toBe('application/json');
        expect(headers['Content-Length']).toBeDefined();
        expect(Number(headers['Content-Length'])).toBeGreaterThan(0);
        expect(headers['User-Agent']).toMatch(/^@oasisprotocol\/rofl-client/);
    });

    it('supports HTTP base URL', async () => {
        const calls: TransportRequest[] = [];
        const transport = async (req: TransportRequest): Promise<TransportResponse> => {
            calls.push(req);
            return {status: 200, body: {key: '1234'}, headers: {}};
        };
        const client = new RoflClient({url: 'https://rofl.example.com', transport});
        const key = await client.generateKey('id', KeyKind.ED25519);
        expect(key).toBe('1234');
        expect(calls[0].baseUrl).toBe('https://rofl.example.com');
        expect(calls[0].socketPath).toBeUndefined();
    });

    it('supports custom unix domain sockets', async () => {
        const calls: TransportRequest[] = [];
        const transport = async (req: TransportRequest): Promise<TransportResponse> => {
            calls.push(req);
            return {status: 200, body: {k1: 'v1'}, headers: {'content-type': 'application/json'}};
        };
        const client = new RoflClient({url: '/custom/rofl.sock', transport});
        await client.getMetadata();

        expect(calls[0].socketPath).toBe('/custom/rofl.sock');
        expect(calls[0].baseUrl).toBeUndefined();
    });

    it('metadata get/set', async () => {
        const calls: TransportRequest[] = [];
        const client = makeClient(calls);

        const metadata = await client.getMetadata();
        expect(metadata).toEqual({k1: 'v1'});

        await client.setMetadata({a: 'b'});
        expect(calls[1].method).toBe('POST');
        expect(calls[1].path).toBe('/rofl/v1/metadata');
        expect(calls[1].payload).toEqual({a: 'b'});
    });

    it('getAppId returns bech32 string', async () => {
        const calls: TransportRequest[] = [];
        const client = makeClient(calls);

        const appId = await client.getAppId();
        expect(appId.startsWith('rofl1')).toBeTruthy();
    });

    it('propagates HTTP errors', async () => {
        const transport = async (): Promise<TransportResponse> => {
            return {
                status: 500,
                body: {error: 'boom'},
                headers: {'content-type': 'application/json'},
            };
        };
        const client = new RoflClient({transport});
        await expect(client.getMetadata()).rejects.toThrow(/500/);
    });

    it('propagates timeoutMs (default 60s and custom override)', async () => {
        // default
        {
            const calls: TransportRequest[] = [];
            const transport = async (req: TransportRequest): Promise<TransportResponse> => {
                calls.push(req);
                return {
                    status: 200,
                    body: {ok: true},
                    headers: {'content-type': 'application/json'},
                };
            };
            const client = new RoflClient({transport});
            await client.getMetadata();
            expect(calls[0].timeoutMs).toBe(60000);
        }
        // override
        {
            const calls: TransportRequest[] = [];
            const transport = async (req: TransportRequest): Promise<TransportResponse> => {
                calls.push(req);
                return {
                    status: 200,
                    body: {ok: true},
                    headers: {'content-type': 'application/json'},
                };
            };
            const client = new RoflClient({transport, timeoutMs: 1234});
            await client.getMetadata();
            expect(calls[0].timeoutMs).toBe(1234);
        }
    });

    it('sign-submit returns raw bytes and normalizes hex prefixes', async () => {
        const calls: TransportRequest[] = [];
        const client = makeClient(calls);

        const res = await client.signAndSubmit(
            {
                kind: 'eth',
                gas_limit: 200000,
                to: '0x1234',
                value: 0,
                data: '0xdeadbeef',
            },
            {encrypt: false},
        );

        // Response should be bytes matching the hex payload.
        expect(Buffer.from(res).toString('hex')).toBe('a1626f6b40');

        // Verify request payload normalization and fields (adjacently-tagged under tx.data).
        const call = calls.find((c) => c.path === '/rofl/v1/tx/sign-submit')!;
        expect(call).toBeTruthy();
        expect(call.method).toBe('POST');
        const payload = call.payload as any;
        expect(payload.encrypt).toBe(false);
        expect(payload.tx.kind).toBe('eth');
        expect(payload.tx.data.gas_limit).toBe(200000);
        expect(payload.tx.data.to).toBe('1234'); // 0x stripped
        expect(payload.tx.data.value).toBe(0);
        expect(payload.tx.data.data).toBe('deadbeef'); // 0x stripped
    });

    it('index exports are wired correctly', () => {
        expect(PublicKeyKind.SECP256K1).toBe('secp256k1');
        expect(PublicSocketPath).toBe('/run/rofl-appd.sock');
        // Construct a client via the public export to ensure type resolves.
        const transport = async (_req: TransportRequest): Promise<TransportResponse> =>
            Promise.resolve({
                status: 200,
                body: {k1: 'v1'},
                headers: {'content-type': 'application/json'},
            });
        const c = new PublicRoflClient({transport});
        expect(typeof c).toBe('object');
    });
});
