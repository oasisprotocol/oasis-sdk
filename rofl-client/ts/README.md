# @oasisprotocol/rofl-client

TypeScript/Node client SDK for **Oasis ROFL**.

## Install

```bash
npm install @oasisprotocol/rofl-client
# or
yarn add @oasisprotocol/rofl-client
```

## Quickstart

```ts
import {RoflClient, KeyKind} from '@oasisprotocol/rofl-client';

const client = new RoflClient(); // UDS: /run/rofl-appd.sock

const key = await client.generateKey('my-key');                   // => hex string
const ed  = await client.generateKey('my-ed25519', KeyKind.ED25519);

await client.setMetadata({key_fingerprint: key.slice(0, 16)});
const metadata = await client.getMetadata();

const appId = await client.getAppId(); // bech32 string (helper)
```

`generateKey` returns the raw private key encoded as a
**hex string (no `0x` prefix)**.

### RoflClient(options?)

- `url?: string`
  - `''` (default): UDS at `/run/rofl-appd.sock`
  - `http(s)://...`: HTTP(S) base URL
  - `'/path/to.sock'`: custom UDS path
- `timeoutMs?: number` (default **60000**)
  - This is a **socket inactivity timeout** (i.e., triggers on no activity).
  It does not strictly bound total wall-clock time for very slow responses.
- `userAgent?: string` (default `@oasisprotocol/rofl-client/<version>`)
  - Override the `User-Agent` header if you need custom telemetry.

### Methods

- `generateKey(keyId: string, kind?: KeyKind): Promise<string>`
- `getMetadata(): Promise<Record<string, string>>`
- `setMetadata(metadata: Record<string, string>): Promise<void>`
- `getAppId(): Promise<string>` (helper)

### KeyKind

`RAW_256 | RAW_384 | ED25519 | SECP256K1`, serialized as
`'raw-256' | 'raw-384' | 'ed25519' | 'secp256k1'`.

### Troubleshooting

If `rofl-appd` isn't running or reachable, requests will fail, for example:

```
Error: ROFL request failed (500): {"error":"..."}
```

Ensure the daemon is running and that you're using the correct UDS path or
HTTP(S) URL.
