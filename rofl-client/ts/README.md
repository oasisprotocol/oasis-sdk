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

// Sign & submit an authenticated transaction (ETH-style)
const callResultBytes = await client.signAndSubmit({
  kind: 'eth',
  gas_limit: 200_000,
  to: '',                    // empty => contract creation
  value: 0,
  data: '0x',                // hex calldata (0x optional)
});
// `callResultBytes` is the raw CBOR-encoded CallResult (Uint8Array)
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
- `signAndSubmit(tx: StdTx | EthTx, opts?: { encrypt?: boolean }):
  Promise<Uint8Array>`
  - Signs the transaction with an app-authenticated key, submits it,
    and returns **raw CBOR-encoded** [`CallResult`] bytes.
  - Hex fields may be provided with or without `0x` and will be normalized.

[`CallResult`]: https://api.docs.oasis.io/rust/oasis_runtime_sdk/types/transaction/enum.CallResult.html

### KeyKind

Supported key generation types (serialized as stable strings):

- `RAW_256` → `'raw-256'`: Generate **256 bits of entropy**
- `RAW_384` → `'raw-384'`: Generate **384 bits of entropy**
- `ED25519` → `'ed25519'`: Generate an **Ed25519** private key
- `SECP256K1` → `'secp256k1'`: Generate a **Secp256k1** private key

### Sign-and-Submit Types

```ts
type StdTx = {
  kind: 'std';
  /** CBOR-serialized hex-encoded Transaction bytes (0x optional). */
  data: string;
};

type EthTx = {
  kind: 'eth';
  gas_limit: number;
  /** Hex address (0x optional). Empty string => contract creation. */
  to: string;
  /** JSON number; must fit JS number range (backend expects u128). */
  value: number;
  /** Hex calldata (0x optional). */
  data: string;
};
```

### Troubleshooting

If `rofl-appd` isn't running or reachable, requests will fail, for example:

```
Error: ROFL request failed (500): {"error":"..."}
```

Ensure the daemon is running and that you're using the correct UDS path or
HTTP(S) URL.
