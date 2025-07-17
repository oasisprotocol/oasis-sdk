import * as oasis from '@oasisprotocol/client';
import * as event from './event';
import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';
import * as nacl from 'tweetnacl';
import * as mraeDeoxysii from './mrae/deoxysii';
import * as deoxysii from '@oasisprotocol/deoxysii';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'rofl';

// Callable methods.
export const METHOD_CREATE = 'rofl.Create';
export const METHOD_UPDATE = 'rofl.Update';
export const METHOD_REMOVE = 'rofl.Remove';
export const METHOD_REGISTER = 'rofl.Register';

// Queries.
export const METHOD_APP = 'rofl.App';
export const METHOD_APPS = 'rofl.Apps';
export const METHOD_APP_INSTANCE = 'rofl.AppInstance';
export const METHOD_APP_INSTANCES = 'rofl.AppInstances';
export const METHOD_PARAMETERS = 'rofl.Parameters';
export const METHOD_STAKE_THRESHOLDS = 'rofl.StakeThresholds';

// Events.
export const EVENT_APP_CREATED_CODE = 1;
export const EVENT_APP_UPDATED_CODE = 2;
export const EVENT_APP_REMOVED_CODE = 3;
export const EVENT_INSTANCE_REGISTERED_CODE = 4;

export const ADDRESS_PREFIX = 'rofl';
export function toBech32(appID: types.AppID) {
    if (appID.length !== 21) {
        throw new Error(`wrong address length: ${appID}`);
    }
    return oasis.address.toBech32(ADDRESS_PREFIX, appID);
}
export function fromBech32(str: string): types.AppID {
    if (str.length !== 41 + ADDRESS_PREFIX.length) {
        // prevent e.g. rofl1qqqqqqqtwy44z
        throw new Error(`wrong address length: ${str}`);
    }
    return oasis.address.fromBech32(ADDRESS_PREFIX, str);
}

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    /** Create a new ROFL application. */
    callCreate() {
        return this.call<types.RoflCreate, void>(METHOD_CREATE);
    }

    /** Update a ROFL application. */
    callUpdate() {
        return this.call<types.RoflUpdate, void>(METHOD_UPDATE);
    }

    /** Remove a ROFL application. */
    callRemove() {
        return this.call<types.RoflRemove, void>(METHOD_REMOVE);
    }

    /** Register a new ROFL App instance replica. */
    callRegister() {
        return this.call<types.RoflRegister, void>(METHOD_REGISTER);
    }

    /** Returns the configuration for the given ROFL application. */
    queryApp() {
        return this.query<types.RoflAppQuery, types.RoflAppConfig>(METHOD_APP);
    }

    /** Returns all ROFL app configurations. */
    queryApps() {
        return this.query<void, types.RoflAppConfig[]>(METHOD_APPS);
    }

    /** Returns a specific registered instance replica for the given ROFL application. */
    queryAppInstance() {
        return this.query<types.RoflAppInstanceQuery, types.RoflRegistration>(METHOD_APP_INSTANCE);
    }

    /** Returns a list of all registered instances replicas for the given ROFL application. */
    queryAppInstances() {
        return this.query<types.RoflAppQuery, types.RoflRegistration[]>(METHOD_APP_INSTANCES);
    }

    /** Returns the minimum stake thresholds for managing ROFL. */
    queryStakeThresholds() {
        return this.query<void, types.RoflStakeThresholds>(METHOD_STAKE_THRESHOLDS);
    }

    /** Queries the module parameters */
    queryParameters() {
        return this.query<void, void>(METHOD_PARAMETERS);
    }
}

export function moduleEventHandler(codes: {
    [EVENT_APP_CREATED_CODE]?: event.Handler<types.RoflAppCreatedEvent>;
    [EVENT_APP_UPDATED_CODE]?: event.Handler<types.RoflAppUpdatedEvent>;
    [EVENT_APP_REMOVED_CODE]?: event.Handler<types.RoflAppRemovedEvent>;
    [EVENT_INSTANCE_REGISTERED_CODE]?: event.Handler<types.RoflInstanceRegisteredEvent>;
}) {
    return [MODULE_NAME, codes] as event.ModuleHandler;
}

export type TransactionCallHandlers = {
    [METHOD_CREATE]?: transaction.CallHandler<types.RoflCreate>;
    [METHOD_UPDATE]?: transaction.CallHandler<types.RoflUpdate>;
    [METHOD_REMOVE]?: transaction.CallHandler<types.RoflRemove>;
    [METHOD_REGISTER]?: transaction.CallHandler<types.RoflRegister>;
};

/** SecretEnvelope is the envelope used for storing encrypted secrets. */
export interface SecretEnvelope {
    /** Pk is the ephemeral public key used for X25519. */
    pk: Uint8Array;
    /** Nonce. */
    nonce: Uint8Array;
    /** Name is the encrypted secret name. */
    name: Uint8Array;
    /** Value is the encrypted secret value. */
    value: Uint8Array;
}

/**
 * EncryptSecret encrypts the given secret given its plain-text name and value together with the
 * secrets encryption key (SEK) obtained for the given application. Returns the Base64-encoded
 * value that can be used in the configuration.
 *
 * Based on https://github.com/oasisprotocol/cli/blob/1fd79c1/build/rofl/secrets.go#L57
 * and https://github.com/oasisprotocol/oasis-sdk/blob/3177bbc/client-sdk/ts-web/rt/src/callformat.ts#L52
 */
export function encryptSecret(
    name: string,
    value: Uint8Array,
    app_sek: types.RoflAppConfig['sek'],
): string {
    // Generate ephemeral X25519 key pair.
    const keyPair = nacl.box.keyPair();
    const pk = keyPair.publicKey;
    const sk = keyPair.secretKey;

    // Generate random nonce.
    const nonce = new Uint8Array(deoxysii.NonceSize);
    crypto.getRandomValues(nonce);

    // Seal plain-text secret name and value.
    const encName = mraeDeoxysii.boxSeal(
        nonce,
        oasis.misc.fromString(name),
        oasis.misc.fromString('name'),
        app_sek,
        sk,
    );
    const encValue = mraeDeoxysii.boxSeal(
        nonce,
        value,
        oasis.misc.fromString('value'),
        app_sek,
        sk,
    );

    const envelope: SecretEnvelope = {
        pk: pk,
        nonce: nonce,
        name: encName,
        value: encValue,
    };

    const data = oasis.misc.toCBOR(envelope);
    return oasis.misc.toBase64(data);
}
