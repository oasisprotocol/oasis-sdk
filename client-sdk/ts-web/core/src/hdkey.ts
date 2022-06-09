import {sha512} from 'js-sha512';
import {SignKeyPair, sign} from 'tweetnacl';
import {generateMnemonic, mnemonicToSeed, validateMnemonic} from 'bip39';
import {concat} from './misc';

const ED25519_CURVE = 'ed25519 seed';
const HARDENED_OFFSET = 0x80000000;
const pathRegex = new RegExp("^m(\\/[0-9]+')+$");

/**
 * HDKey handles hierarchical key generation according to ADR 0008
 * https://github.com/oasisprotocol/adrs/blob/main/0008-standard-account-key-generation.md
 */
export class HDKey {
    public readonly keypair: SignKeyPair;

    /**
     * Generates the keypair matching the supplied parameters
     * @param mnemonic BIP-0039 Mnemonic
     * @param index Account index
     * @param passphrase Optional BIP-0039 passphrase
     * @returns SignKeyPair for these parameters
     */
    public static async getAccountSigner(
        mnemonic: string,
        index: number = 0,
        passphrase?: string,
    ): Promise<SignKeyPair> {
        if (index < 0 || index > 0x7fffffff) {
            throw new Error('Account number must be >= 0 and <= 2147483647');
        }

        const seed = await mnemonicToSeed(mnemonic, passphrase);
        const key = HDKey.makeHDKey(ED25519_CURVE, seed);
        return key.derivePath(`m/44'/474'/${index}'`).keypair;
    }

    /**
     * Generates a mnemonic
     * @param strength Length in bits of the generated mnemonic
     * @returns Generated BIP-0039 Mnemonic
     */
    public static generateMnemonic(strength = 256): string {
        return generateMnemonic(strength);
    }

    private constructor(
        private readonly privateKey: Uint8Array,
        private readonly chainCode: Uint8Array,
    ) {
        this.keypair = sign.keyPair.fromSeed(privateKey);
    }

    /**
     * Returns the HDKey for the given derivation path
     * using SLIP-0010
     * @param path Derivation path, starting with m/
     * @returns Instance of HDKey
     */
    private derivePath(path: string): HDKey {
        if (!pathRegex.test(path)) {
            throw new Error(
                "Invalid derivation path. Valid paths must use a format similar to : m/44'/474'/0' and all indexes must be hardened",
            );
        }

        const segments = path
            .split('/')
            .slice(1)
            .map((val: string): string => val.replace("'", ''))
            .map((el) => parseInt(el, 10));

        return segments.reduce<HDKey>(
            (parent, segment) => parent.derive(segment + HARDENED_OFFSET),
            this,
        );
    }

    /**
     * Derive the child key at the specified index
     * @param index
     * @returns Instance of HDKey
     */
    private derive(index: number): HDKey {
        const buffer = new ArrayBuffer(4);
        const view = new DataView(buffer);
        view.setUint32(0, index);

        const data = concat(new Uint8Array([0]), this.privateKey, new Uint8Array(buffer));
        return HDKey.makeHDKey(this.chainCode, data);
    }

    private static makeHDKey(hmacKey: string | Uint8Array, data: Uint8Array): HDKey {
        //@ts-ignore The types of js-sha512 are outdated
        const hash = sha512.hmac.arrayBuffer(hmacKey, data);

        const I = new Uint8Array(hash);
        const IL = I.slice(0, 32);
        const IR = I.slice(32);

        return new HDKey(IL, IR);
    }
}
