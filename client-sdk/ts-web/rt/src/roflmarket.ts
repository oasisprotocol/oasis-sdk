import * as event from './event';
import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'roflmarket';

export const ERR_INVALID_ARGUMENT_CODE = 1;
export const ERR_PROVIDER_ALREADY_EXISTS_CODE = 2;
export const ERR_PROVIDER_NOT_FOUND_CODE = 3;
export const ERR_FORBIDDEN_CODE = 4;
export const ERR_PROVIDER_HAS_INSTANCES_CODE = 5;
export const ERR_OUT_OF_CAPACITY_CODE = 6;
export const ERR_OFFER_NOT_FOUND_CODE = 7;
export const ERR_INSTANCE_NOT_FOUND_CODE = 8;
export const ERR_TOO_MANY_QUEUED_COMMANDS_CODE = 9;
export const ERR_PAYMENT_FAILED_CODE = 10;
export const ERR_BAD_RESOURCE_DESCRIPTOR_CODE = 11;
export const ERR_INVALID_INSTANCE_STATE_CODE = 12;

// Callable methods.
export const METHOD_PROVIDER_CREATE = 'roflmarket.ProviderCreate';
export const METHOD_PROVIDER_UPDATE = 'roflmarket.ProviderUpdate';
export const METHOD_PROVIDER_UPDATE_OFFERS = 'roflmarket.ProviderUpdateOffers';
export const METHOD_PROVIDER_REMOVE = 'roflmarket.ProviderRemove';
export const METHOD_INSTANCE_CREATE = 'roflmarket.InstanceCreate';
export const METHOD_INSTANCE_CHANGE_ADMIN = 'roflmarket.InstanceChangeAdmin';
export const METHOD_INSTANCE_TOP_UP = 'roflmarket.InstanceTopUp';
export const METHOD_INSTANCE_ACCEPT = 'roflmarket.InstanceAccept';
export const METHOD_INSTANCE_UPDATE = 'roflmarket.InstanceUpdate';
export const METHOD_INSTANCE_CANCEL = 'roflmarket.InstanceCancel';
export const METHOD_INSTANCE_REMOVE = 'roflmarket.InstanceRemove';
export const METHOD_INSTANCE_EXECUTE_CMDS = 'roflmarket.InstanceExecuteCmds';
export const METHOD_INSTANCE_CLAIM_PAYMENT = 'roflmarket.InstanceClaimPayment';

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

    /** Creates a new machine provider. */
    callProviderCreate() {
        return this.call<types.RoflmarketProviderCreate, void>(METHOD_PROVIDER_CREATE);
    }

    /** Updates an existing provider. */
    callProviderUpdate() {
        return this.call<types.RoflmarketProviderUpdate, void>(METHOD_PROVIDER_UPDATE);
    }

    /** Updates offers for a provider. */
    callProviderUpdateOffers() {
        return this.call<types.RoflmarketProviderUpdateOffers, void>(METHOD_PROVIDER_UPDATE_OFFERS);
    }

    /** Removes a provider. */
    callProviderRemove() {
        return this.call<types.RoflmarketProviderRemove, void>(METHOD_PROVIDER_REMOVE);
    }

    /** Creates a new machine instance. */
    callInstanceCreate() {
        return this.call<types.RoflmarketInstanceCreate, void>(METHOD_INSTANCE_CREATE);
    }

    /** Changes the admin of a machine instance. */
    callInstanceChangeAdmin() {
        return this.call<types.RoflmarketInstanceChangeAdmin, void>(METHOD_INSTANCE_CHANGE_ADMIN);
    }

    /** Tops up a machine instance. */
    callInstanceTopUp() {
        return this.call<types.RoflmarketInstanceTopUp, void>(METHOD_INSTANCE_TOP_UP);
    }

    /** Accepts machine instances. Intended for scheduler use only. */
    private callInstanceAccept() {
        return this.call<types.RoflmarketInstanceAccept, void>(METHOD_INSTANCE_ACCEPT);
    }

    /** Updates machine instances. Intended for scheduler use only. */
    private callInstanceUpdate() {
        return this.call<types.RoflmarketInstanceUpdate, void>(METHOD_INSTANCE_UPDATE);
    }

    /** Cancels a machine instance. */
    callInstanceCancel() {
        return this.call<types.RoflmarketInstanceCancel, void>(METHOD_INSTANCE_CANCEL);
    }

    /** Removes a machine instance. Intended for scheduler use only. */
    private callInstanceRemove() {
        return this.call<types.RoflmarketInstanceRemove, void>(METHOD_INSTANCE_REMOVE);
    }

    /**
     * Executes commands on a machine instance.
     *
     * https://github.com/oasisprotocol/oasis-sdk/blob/4fdb76f/rofl-scheduler/src/types.rs
     * https://github.com/oasisprotocol/cli/blob/b6894a1/build/rofl/scheduler/commands.go#L9-L42
     */
    callInstanceExecuteCmds() {
        return this.call<types.RoflmarketInstanceExecuteCmds, void>(METHOD_INSTANCE_EXECUTE_CMDS);
    }

    /** Claims payment for machine instances. Intended for scheduler use only. */
    private callInstanceClaimPayment() {
        return this.call<types.RoflmarketInstanceClaimPayment, void>(METHOD_INSTANCE_CLAIM_PAYMENT);
    }

    /** Returns the provider descriptor. */
    queryProvider() {
        return this.query<types.RoflmarketProviderQuery, types.RoflmarketProvider>(METHOD_PROVIDER);
    }

    /** Returns all provider descriptors. */
    queryProviders() {
        return this.query<void, types.RoflmarketProvider[]>(METHOD_PROVIDERS);
    }

    /** Returns the specified offer. */
    queryOffer() {
        return this.query<types.RoflmarketOfferQuery, types.RoflmarketOffer>(METHOD_OFFER);
    }

    /** Returns all offers of a given provider. */
    queryOffers() {
        return this.query<types.RoflmarketProviderQuery, types.RoflmarketOffer[]>(METHOD_OFFERS);
    }

    /** Returns the machine instance descriptor. */
    queryInstance() {
        return this.query<types.RoflmarketInstanceQuery, types.RoflmarketInstance>(METHOD_INSTANCE);
    }

    /** Returns all instances of a given provider. */
    queryInstances() {
        return this.query<types.RoflmarketProviderQuery, types.RoflmarketInstance[]>(
            METHOD_INSTANCES,
        );
    }

    /** Returns all queued commands of a given machine instance. */
    queryInstanceCommands() {
        return this.query<types.RoflmarketInstanceQuery, types.RoflmarketQueuedCommand[]>(
            METHOD_INSTANCE_COMMANDS,
        );
    }

    /** Returns the stake requirements. */
    queryStakeThresholds() {
        return this.query<void, types.RoflmarketStakeThresholds>(METHOD_STAKE_THRESHOLDS);
    }

    /** Returns the module parameters. */
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
    [METHOD_INSTANCE_CHANGE_ADMIN]?: transaction.CallHandler<types.RoflmarketInstanceChangeAdmin>;
    [METHOD_INSTANCE_TOP_UP]?: transaction.CallHandler<types.RoflmarketInstanceTopUp>;
    [METHOD_INSTANCE_ACCEPT]?: transaction.CallHandler<types.RoflmarketInstanceAccept>;
    [METHOD_INSTANCE_UPDATE]?: transaction.CallHandler<types.RoflmarketInstanceUpdate>;
    [METHOD_INSTANCE_CANCEL]?: transaction.CallHandler<types.RoflmarketInstanceCancel>;
    [METHOD_INSTANCE_REMOVE]?: transaction.CallHandler<types.RoflmarketInstanceRemove>;
    [METHOD_INSTANCE_EXECUTE_CMDS]?: transaction.CallHandler<types.RoflmarketInstanceExecuteCmds>;
    [METHOD_INSTANCE_CLAIM_PAYMENT]?: transaction.CallHandler<types.RoflmarketInstanceClaimPayment>;
};
