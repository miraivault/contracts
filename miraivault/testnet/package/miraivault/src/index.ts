import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}


export const networks = {
  testnet: {
    networkPassphrase: "Test SDF Network ; September 2015",
    contractId: "CBQ7BXA53NFG5PH4L3PBBWAYZKYATCQHSHNXRN6DXDBRZPMWG2PZTBNU",
  }
} as const

export type DataKey = {tag: "NextVaultId", values: void} | {tag: "Vault", values: readonly [u64]} | {tag: "Reward", values: readonly [u64]} | {tag: "SenderVaults", values: readonly [string]} | {tag: "BeneficiaryVaults", values: readonly [string]} | {tag: "MiraiToken", values: void} | {tag: "TrueBalance", values: void} | {tag: "TotalSupply", values: void} | {tag: "Admin", values: void} | {tag: "OnlyStellar", values: void} | {tag: "AdjustmentFactor", values: void} | {tag: "CFactor", values: void} | {tag: "DFactor", values: void};


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
  get_vault: ({vault_id}: {vault_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<VestingSchedule>>

  /**
   * Construct and simulate a claim_main transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_main: ({vault_id, beneficiary}: {vault_id: u64, beneficiary: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_reward: ({vault_id}: {vault_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<RewardInfo>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({mirai_token, total_supply, initial_true_balance, admin, only_stellar_token}: {mirai_token: string, total_supply: i128, initial_true_balance: i128, admin: string, only_stellar_token: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a create_fund transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_fund: ({sender, beneficiary, token_address, total_amount, num_years, frequency, reward_split}: {sender: string, beneficiary: string, token_address: string, total_amount: i128, num_years: u32, frequency: u32, reward_split: u32}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a claim_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_reward: ({vault_id, claimant}: {vault_id: u64, claimant: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a is_fully_vested transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_fully_vested: ({vault_id}: {vault_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a cancel_remaining transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_remaining: ({vault_id, sender}: {vault_id: u64, sender: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_total_vaults transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_total_vaults: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a get_vaults_by_me transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_vaults_by_me: ({sender}: {sender: string}, options?: MethodOptions) => Promise<AssembledTransaction<Array<u64>>>

  /**
   * Construct and simulate a get_mirai_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_mirai_balance: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_next_vault_id transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_next_vault_id: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a get_vaults_for_me transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_vaults_for_me: ({beneficiary}: {beneficiary: string}, options?: MethodOptions) => Promise<AssembledTransaction<Array<u64>>>

  /**
   * Construct and simulate a get_claimable_main transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_claimable_main: ({vault_id}: {vault_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a update_true_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_true_balance: ({amount}: {amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_claimable_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_claimable_reward: ({vault_id, claimant}: {vault_id: u64, claimant: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_true_balance_public transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_true_balance_public: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "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" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_vault: this.txFromJSON<VestingSchedule>,
        claim_main: this.txFromJSON<null>,
        get_reward: this.txFromJSON<RewardInfo>,
        initialize: this.txFromJSON<null>,
        create_fund: this.txFromJSON<u64>,
        claim_reward: this.txFromJSON<null>,
        is_fully_vested: this.txFromJSON<boolean>,
        cancel_remaining: this.txFromJSON<null>,
        get_total_vaults: this.txFromJSON<u64>,
        get_vaults_by_me: this.txFromJSON<Array<u64>>,
        get_mirai_balance: this.txFromJSON<i128>,
        get_next_vault_id: this.txFromJSON<u64>,
        get_vaults_for_me: this.txFromJSON<Array<u64>>,
        get_claimable_main: this.txFromJSON<i128>,
        update_true_balance: this.txFromJSON<null>,
        get_claimable_reward: this.txFromJSON<i128>,
        get_true_balance_public: this.txFromJSON<i128>
  }
}