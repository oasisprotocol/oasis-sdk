import * as oasis from '@oasisprotocol/client';
import * as event from './event';
import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';

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
    return oasis.address.toBech32(ADDRESS_PREFIX, appID);
}
export function fromBech32(str: string): types.AppID {
    return oasis.address.fromBech32(ADDRESS_PREFIX, str);
}

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    callCreate() {
        return this.call<types.RoflCreate, void>(METHOD_CREATE);
    }

    callUpdate() {
        return this.call<types.RoflUpdate, void>(METHOD_UPDATE);
    }

    callRemove() {
        return this.call<types.RoflRemove, void>(METHOD_REMOVE);
    }

    callRegister() {
        return this.call<types.RoflRegister, void>(METHOD_REGISTER);
    }

    queryApp() {
        return this.query<types.RoflAppQuery, types.RoflAppConfig>(METHOD_APP);
    }

    queryApps() {
        return this.query<void, types.RoflAppConfig[]>(METHOD_APPS);
    }

    queryAppInstance() {
        return this.query<types.RoflAppInstanceQuery, types.RoflRegistration>(METHOD_APP_INSTANCE);
    }

    queryAppInstances() {
        return this.query<types.RoflAppQuery, types.RoflRegistration[]>(METHOD_APP_INSTANCES);
    }

    queryStakeThresholds() {
        return this.query<void, types.RoflStakeThresholds>(METHOD_STAKE_THRESHOLDS);
    }

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
