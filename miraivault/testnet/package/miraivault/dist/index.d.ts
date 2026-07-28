import { Buffer } from "buffer";
import { AssembledTransaction, Client as ContractClient, ClientOptions as ContractClientOptions, MethodOptions } from "@stellar/stellar-sdk/contract";
import type { u32, u64, i128 } from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";
export declare const networks: {
    readonly testnet: {
        readonly networkPassphrase: "Test SDF Network ; September 2015";
        readonly contractId: "CBQ7BXA53NFG5PH4L3PBBWAYZKYATCQHSHNXRN6DXDBRZPMWG2PZTBNU";
    };
};
export type DataKey = {
    tag: "NextVaultId";
    values: void;
} | {
    tag: "Vault";
    values: readonly [u64];
} | {
    tag: "Reward";
    values: readonly [u64];
} | {
    tag: "SenderVaults";
    values: readonly [string];
} | {
    tag: "BeneficiaryVaults";
    values: readonly [string];
} | {
    tag: "MiraiToken";
    values: void;
} | {
    tag: "TrueBalance";
    values: void;
} | {
    tag: "TotalSupply";
    values: void;
} | {
    tag: "Admin";
    values: void;
} | {
    tag: "OnlyStellar";
    values: void;
} | {
    tag: "AdjustmentFactor";
    values: void;
} | {
    tag: "CFactor";
    values: void;
} | {
    tag: "DFactor";
    values: void;
};
export interface RewardInfo {
    claimed_receiver: i128;
    claimed_sender: i128;
    receiver_share: i128;
    sender_share: i128;
    total_reward: i128;
    vault_id: u64;
}
export interface VestingSchedule {
    beneficiary: string;
    claimed_packets: u32;
    frequency: u32;
    is_cancelled: boolean;
    num_years: u32;
    packet_amount: i128;
    reward_split: u32;
    sender: string;
    start_timestamp: u64;
    token_address: string;
    total_amount: i128;
    vault_id: u64;
}
export interface Client {
    /**
     * Construct and simulate a get_vault transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_vault: ({ vault_id }: {
        vault_id: u64;
    }, options?: MethodOptions) => Promise<AssembledTransaction<VestingSchedule>>;
    /**
     * Construct and simulate a claim_main transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    claim_main: ({ vault_id, beneficiary }: {
        vault_id: u64;
        beneficiary: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a get_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_reward: ({ vault_id }: {
        vault_id: u64;
    }, options?: MethodOptions) => Promise<AssembledTransaction<RewardInfo>>;
    /**
     * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    initialize: ({ mirai_token, total_supply, initial_true_balance, admin, only_stellar_token }: {
        mirai_token: string;
        total_supply: i128;
        initial_true_balance: i128;
        admin: string;
        only_stellar_token: boolean;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a create_fund transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    create_fund: ({ sender, beneficiary, token_address, total_amount, num_years, frequency, reward_split }: {
        sender: string;
        beneficiary: string;
        token_address: string;
        total_amount: i128;
        num_years: u32;
        frequency: u32;
        reward_split: u32;
    }, options?: MethodOptions) => Promise<AssembledTransaction<u64>>;
    /**
     * Construct and simulate a claim_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    claim_reward: ({ vault_id, claimant }: {
        vault_id: u64;
        claimant: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a is_fully_vested transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    is_fully_vested: ({ vault_id }: {
        vault_id: u64;
    }, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>;
    /**
     * Construct and simulate a cancel_remaining transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    cancel_remaining: ({ vault_id, sender }: {
        vault_id: u64;
        sender: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a get_total_vaults transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_total_vaults: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>;
    /**
     * Construct and simulate a get_vaults_by_me transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_vaults_by_me: ({ sender }: {
        sender: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<Array<u64>>>;
    /**
     * Construct and simulate a get_mirai_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_mirai_balance: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>;
    /**
     * Construct and simulate a get_next_vault_id transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_next_vault_id: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>;
    /**
     * Construct and simulate a get_vaults_for_me transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_vaults_for_me: ({ beneficiary }: {
        beneficiary: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<Array<u64>>>;
    /**
     * Construct and simulate a get_claimable_main transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_claimable_main: ({ vault_id }: {
        vault_id: u64;
    }, options?: MethodOptions) => Promise<AssembledTransaction<i128>>;
    /**
     * Construct and simulate a update_true_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    update_true_balance: ({ amount }: {
        amount: i128;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a get_claimable_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_claimable_reward: ({ vault_id, claimant }: {
        vault_id: u64;
        claimant: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<i128>>;
    /**
     * Construct and simulate a get_true_balance_public transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_true_balance_public: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>;
}
export declare class Client extends ContractClient {
    readonly options: ContractClientOptions;
    static deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions & Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
    }): Promise<AssembledTransaction<T>>;
    constructor(options: ContractClientOptions);
    readonly fromJSON: {
        get_vault: (json: string) => AssembledTransaction<VestingSchedule>;
        claim_main: (json: string) => AssembledTransaction<null>;
        get_reward: (json: string) => AssembledTransaction<RewardInfo>;
        initialize: (json: string) => AssembledTransaction<null>;
        create_fund: (json: string) => AssembledTransaction<bigint>;
        claim_reward: (json: string) => AssembledTransaction<null>;
        is_fully_vested: (json: string) => AssembledTransaction<boolean>;
        cancel_remaining: (json: string) => AssembledTransaction<null>;
        get_total_vaults: (json: string) => AssembledTransaction<bigint>;
        get_vaults_by_me: (json: string) => AssembledTransaction<bigint[]>;
        get_mirai_balance: (json: string) => AssembledTransaction<bigint>;
        get_next_vault_id: (json: string) => AssembledTransaction<bigint>;
        get_vaults_for_me: (json: string) => AssembledTransaction<bigint[]>;
        get_claimable_main: (json: string) => AssembledTransaction<bigint>;
        update_true_balance: (json: string) => AssembledTransaction<null>;
        get_claimable_reward: (json: string) => AssembledTransaction<bigint>;
        get_true_balance_public: (json: string) => AssembledTransaction<bigint>;
    };
}
