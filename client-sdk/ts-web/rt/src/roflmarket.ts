import * as event from './event';
import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'roflmarket';

// Callable methods.
export const METHOD_PROVIDER_CREATE = 'roflmarket.ProviderCreate';
export const METHOD_PROVIDER_UPDATE = 'roflmarket.ProviderUpdate';
export const METHOD_PROVIDER_UPDATE_OFFERS = 'roflmarket.ProviderUpdateOffers';
export const METHOD_PROVIDER_REMOVE = 'roflmarket.ProviderRemove';
export const METHOD_INSTANCE_CREATE = 'roflmarket.InstanceCreate';
export const METHOD_INSTANCE_TOP_UP = 'roflmarket.InstanceTopUp';
export const METHOD_INSTANCE_CANCEL = 'roflmarket.InstanceCancel';
export const METHOD_INSTANCE_EXECUTE_CMDS = 'roflmarket.InstanceExecuteCmds';

// Queries.
export const METHOD_PROVIDER = 'roflmarket.Provider';
export const METHOD_PROVIDERS = 'roflmarket.Providers';
export const METHOD_OFFER = 'roflmarket.Offer';
export const METHOD_OFFERS = 'roflmarket.Offers';
export const METHOD_INSTANCE = 'roflmarket.Instance';
export const METHOD_INSTANCES = 'roflmarket.Instances';
export const METHOD_INSTANCE_COMMANDS = 'roflmarket.InstanceCommands';
export const METHOD_PARAMETERS = 'roflmarket.Parameters';
export const METHOD_STAKE_THRESHOLDS = 'roflmarket.StakeThresholds';

// Events.
export const EVENT_PROVIDER_CREATED_CODE = 1;
export const EVENT_PROVIDER_UPDATED_CODE = 2;
export const EVENT_PROVIDER_REMOVED_CODE = 3;
export const EVENT_INSTANCE_CREATED_CODE = 4;
export const EVENT_INSTANCE_UPDATED_CODE = 5;
export const EVENT_INSTANCE_ACCEPTED_CODE = 6;
export const EVENT_INSTANCE_CANCELLED_CODE = 7;
export const EVENT_INSTANCE_REMOVED_CODE = 8;
export const EVENT_INSTANCE_COMMAND_QUEUED_CODE = 9;

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    callProviderCreate() {
        return this.call<types.RoflmarketProviderCreate, void>(METHOD_PROVIDER_CREATE);
    }

    callProviderUpdate() {
        return this.call<types.RoflmarketProviderUpdate, void>(METHOD_PROVIDER_UPDATE);
    }

    callProviderUpdateOffers() {
        return this.call<types.RoflmarketProviderUpdateOffers, void>(METHOD_PROVIDER_UPDATE_OFFERS);
    }

    callProviderRemove() {
        return this.call<types.RoflmarketProviderRemove, void>(METHOD_PROVIDER_REMOVE);
    }

    callInstanceCreate() {
        return this.call<types.RoflmarketInstanceCreate, void>(METHOD_INSTANCE_CREATE);
    }

    callInstanceTopUp() {
        return this.call<types.RoflmarketInstanceTopUp, void>(METHOD_INSTANCE_TOP_UP);
    }

    callInstanceCancel() {
        return this.call<types.RoflmarketInstanceCancel, void>(METHOD_INSTANCE_CANCEL);
    }

    callInstanceExecuteCmds() {
        return this.call<types.RoflmarketInstanceExecuteCmds, void>(METHOD_INSTANCE_EXECUTE_CMDS);
    }

    queryProvider() {
        return this.query<types.RoflmarketProviderQuery, types.RoflmarketProvider>(METHOD_PROVIDER);
    }

    queryProviders() {
        return this.query<void, types.RoflmarketProvider[]>(METHOD_PROVIDERS);
    }

    queryOffer() {
        return this.query<types.RoflmarketOfferQuery, types.RoflmarketOffer>(METHOD_OFFER);
    }

    queryOffers() {
        return this.query<types.RoflmarketProviderQuery, types.RoflmarketOffer[]>(METHOD_OFFERS);
    }

    queryInstance() {
        return this.query<types.RoflmarketInstanceQuery, types.RoflmarketInstance>(METHOD_INSTANCE);
    }

    queryInstances() {
        return this.query<types.RoflmarketProviderQuery, types.RoflmarketInstance[]>(
            METHOD_INSTANCES,
        );
    }

    queryInstanceCommands() {
        return this.query<types.RoflmarketInstanceQuery, types.RoflmarketQueuedCommand[]>(
            METHOD_INSTANCE_COMMANDS,
        );
    }

    queryStakeThresholds() {
        return this.query<void, types.RoflmarketStakeThresholds>(METHOD_STAKE_THRESHOLDS);
    }

    queryParameters() {
        return this.query<void, void>(METHOD_PARAMETERS);
    }
}

export function moduleEventHandler(codes: {
    [EVENT_PROVIDER_CREATED_CODE]?: event.Handler<types.RoflmarketProviderCreatedEvent>;
    [EVENT_PROVIDER_UPDATED_CODE]?: event.Handler<types.RoflmarketProviderUpdatedEvent>;
    [EVENT_PROVIDER_REMOVED_CODE]?: event.Handler<types.RoflmarketProviderRemovedEvent>;
    [EVENT_INSTANCE_CREATED_CODE]?: event.Handler<types.RoflmarketInstanceCreatedEvent>;
    [EVENT_INSTANCE_UPDATED_CODE]?: event.Handler<types.RoflmarketInstanceUpdatedEvent>;
    [EVENT_INSTANCE_ACCEPTED_CODE]?: event.Handler<types.RoflmarketInstanceAcceptedEvent>;
    [EVENT_INSTANCE_CANCELLED_CODE]?: event.Handler<types.RoflmarketInstanceCancelledEvent>;
    [EVENT_INSTANCE_REMOVED_CODE]?: event.Handler<types.RoflmarketInstanceRemovedEvent>;
    [EVENT_INSTANCE_COMMAND_QUEUED_CODE]?: event.Handler<types.RoflmarketInstanceCommandQueuedEvent>;
}) {
    return [MODULE_NAME, codes] as event.ModuleHandler;
}

export type TransactionCallHandlers = {
    [METHOD_PROVIDER_CREATE]?: transaction.CallHandler<types.RoflmarketProviderCreate>;
    [METHOD_PROVIDER_UPDATE]?: transaction.CallHandler<types.RoflmarketProviderUpdate>;
    [METHOD_PROVIDER_UPDATE_OFFERS]?: transaction.CallHandler<types.RoflmarketProviderUpdateOffers>;
    [METHOD_PROVIDER_REMOVE]?: transaction.CallHandler<types.RoflmarketProviderRemove>;
    [METHOD_INSTANCE_CREATE]?: transaction.CallHandler<types.RoflmarketInstanceCreate>;
    [METHOD_INSTANCE_TOP_UP]?: transaction.CallHandler<types.RoflmarketInstanceTopUp>;
    [METHOD_INSTANCE_CANCEL]?: transaction.CallHandler<types.RoflmarketInstanceCancel>;
    [METHOD_INSTANCE_EXECUTE_CMDS]?: transaction.CallHandler<types.RoflmarketInstanceExecuteCmds>;
};
