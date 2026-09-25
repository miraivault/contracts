#![no_std]
use soroban_sdk::{contract, contractimpl, contractevent, contracttype, token, Address, Env, Vec, symbol_short, IntoVal, Symbol};
use soroban_sdk::auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation};
use crate::pool_client::{ PoolClient, Request, REQUEST_TYPE_SUPPLY_COLLATERAL, REQUEST_TYPE_WITHDRAW_COLLATERAL };

use crate::defindex_client::{ DefindexClient };


#[cfg(test)]
use crate::mock_protocol::MockProtocolClient;


#[cfg(test)]
use crate::mock_defindex::MockDefindexClient;

#[cfg(test)]
mod mock_protocol;

#[cfg(test)]
mod mock_defindex;

mod pool_client;
mod defindex_client;

const TESTNET_NATIVE_XLM: &str = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
const MIN_VAULT_AMT: i128 = 1_000_000_000;
const MAX_LARGENESS: i128 = 100_000_000_000;
const MAX_TRUE_BALANCE_RATIO: i128 = 66;   // 66% of total MIRAI supply
const PROTOCOL_FEE_BPS: u32 = 500;          // 5% protocol fee on yield rewards
const REWARD_PRECISION: i128 = 1_000_000_000_000; // 1e12



#[contract]
pub struct TimeCapsule;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    NextVaultId,                  // u64 global counter
    Vault(u64),                   // vault_id → VestingSchedule
    Reward(u64),                  // vault_id → RewardInfo
    SenderVaults(Address),        // sender → Vec<u64>
    BeneficiaryVaults(Address),   // beneficiary → Vec<u64>
    MiraiToken,
    TrueBalance,
    TotalSupply,
    OnlyStellar,
    AdjustmentFactor,
    CFactor,
    DFactor,
    Treasury,
    P1Token,
    AccRewardPerShareP1,
    TotalSuppliedP1,
    LastRewardBalanceP1,
    AccRewardPerShareP2,
    TotalSuppliedP2,
    LastRewardBalanceP2,

}

#[contracttype]
#[derive(Clone)]
pub struct VestingSchedule {
    pub vault_id: u64,
    pub sender: Address,
    pub beneficiary: Address,
    pub token_address: Address,
    pub total_amount: i128,
    pub num_years: u32,
    pub frequency: u32,           // 1, 2, 4, or 365
    pub reward_split: u32,
    pub packet_amount: i128,
    pub start_timestamp: u64,
    pub claimed_packets: u32,
    pub is_cancelled: bool,

    // Yield protocols (p1 = Blend, p2 = DeFindex)
    pub idle_amount: i128,
    pub p1_pool: Option<Address>,
    pub p2_pool: Option<Address>,
    pub p1_ratio: u32,
    pub p2_ratio: u32,

    pub supplied_p1: i128,
    pub supplied_p2: i128,

    pub p2_shares: i128,
    pub p2_principal: i128,

    // Simple round-robin for forced withdrawals (0 = p1, 1 = p2)
    pub last_withdraw_from: u32,

    // already credited rewards from secondary protocols
    pub reward_debt_p1: i128,
    pub reward_debt_p2: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct RewardInfo {
    pub vault_id: u64,
    pub total_reward: i128,
    pub sender_share: i128,
    pub receiver_share: i128,
    pub claimed_sender: i128,
    pub claimed_receiver: i128,
}

// ================== CONTRACT EVENTS ==================
#[contractevent(data_format = "single-value")]
struct FundCreatedEvent {
    #[topic]
    vault_id: u64,
    #[topic]
    sender: Address,
    #[topic]
    beneficiary: Address,
    #[topic]
    total_amount: i128,
    #[topic]
    num_years: u32,
    #[topic]
    frequency: u32,
    #[topic]
    reward_split: u32,
    #[topic]
    total_reward: i128,
    #[topic]
    p1_ratio: u32,
    #[topic]
    p2_ratio: u32,
}

#[contractevent(data_format = "single-value")]
struct MainClaimedEvent {
    #[topic]
    vault_id: u64,
    #[topic]
    beneficiary: Address,
    #[topic]
    amount: i128,
    #[topic]
    packets: u32,
}

#[contractevent(data_format = "single-value")]
struct RewardClaimedEvent {
    #[topic]
    vault_id: u64,
    #[topic]
    claimant: Address,
    #[topic]
    amount: i128,
    #[topic]
    is_sender: bool,
}

#[contractevent(data_format = "single-value")]
struct ProtocolRewardClaimedEvent {
    #[topic]
    vault_id: u64,
    #[topic]
    claimant: Address,
    #[topic]
    protocol: u32,          // 1 = P1 (Blend), 2 = P2 (DeFindex)
    #[topic]
    total_claimed: i128,
    #[topic]
    user_share: i128,
    #[topic]
    fee: i128,
}

#[contractevent(data_format = "single-value")]
struct VaultCancelledEvent {
    #[topic]
    vault_id: u64,
    #[topic]
    sender: Address,
    #[topic]
    beneficiary: Address,
}


#[contractimpl]
impl TimeCapsule {

    pub fn initialize(env: Env, mirai_token: Address, total_supply: i128, initial_true_balance: i128, only_stellar_token: bool, p1_token: Address, treasury: Address){
        if env.storage().persistent().has(&DataKey::MiraiToken){
            panic!("Contract has already been initialized!");
        }

        if total_supply <= 1_000_000 as i128 || initial_true_balance <= 100000 as i128{
            panic!("Total Supply cannot be lower than 1 million. True Balance cannot be lower than 100000");
        }

        if total_supply >= 1_000_000_000 as i128 {
            panic!("Total Supply cannot exceed 1 billion.");
        }

        if 2 * total_supply < 3 * initial_true_balance{
            panic!("True Balance cannot be more than 2/3 of the Total Supply!");
        }

        if total_supply > 2 * initial_true_balance{
            panic!("True Balance cannot be less than 1/2 of the Total Supply!");
        }

        env.storage().persistent().set(&DataKey::MiraiToken, &mirai_token);
        env.storage().persistent().set(&DataKey::TrueBalance, &initial_true_balance);
        env.storage().persistent().set(&DataKey::TotalSupply, &total_supply);
        env.storage().persistent().set(&DataKey::OnlyStellar, &only_stellar_token);
        env.storage().persistent().set(&DataKey::P1Token, &p1_token);
        env.storage().persistent().set(&DataKey::Treasury, &treasury);

        let adj = (1920 as i128 * total_supply) / (10_000_000 as i128);
        env.storage().persistent().set(&DataKey::AdjustmentFactor, &adj);

        let cf: i128;
        let df: i128;

        if total_supply > 10_000_000{
            cf = total_supply / 10_000_000;
            df = 1;
        }
        else {
            df = 10_000_000 / total_supply;
            cf = 1;
        }
        env.storage().persistent().set(&DataKey::CFactor, &cf);
        env.storage().persistent().set(&DataKey::DFactor, &df);
    }


    // Important for $MIRAI buybacks
    pub fn refill_true_balance(env: Env, amount: i128) {
        if amount <= 0 { panic!("Amount must be positive"); }

        let mirai_addr = Self::get_mirai_address(&env);
        let client = token::Client::new(&env, &mirai_addr);
        let contract = env.current_contract_address();

        let apparent = client.balance(&contract);
        let current = Self::get_true_balance(&env);
        let supply = Self::get_total_supply(&env);

        let max_allowed = (supply * MAX_TRUE_BALANCE_RATIO) / 100;
        if current + amount > max_allowed {
            panic!("True balance cannot exceed 66% of total supply");
        }
        if apparent < current + amount {
            panic!("Insufficient MIRAI sent to contract");
        }

        Self::set_true_balance(&env, current + amount);
    }

    fn get_mirai_address(env: &Env) -> Address {
        env.storage().persistent().get(&DataKey::MiraiToken).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }

    fn get_true_balance(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::TrueBalance).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }

    fn set_true_balance(env: &Env, amount: i128) {
        env.storage().persistent().set(&DataKey::TrueBalance, &amount);
    }

    fn get_total_supply(env: &Env) -> i128{
        env.storage().persistent().get(&DataKey::TotalSupply).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }


    fn is_only_stellar(env: &Env) -> bool {
        env.storage().persistent().get(&DataKey::OnlyStellar).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }

    fn get_adjustment_factor(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::AdjustmentFactor).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }

    fn get_c_factor(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::CFactor).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }

    fn get_d_factor(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::DFactor).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }

    pub fn get_acc_reward_per_share_p1(env: &Env) -> i128 {
    env.storage().persistent().get(&DataKey::AccRewardPerShareP1).unwrap_or(0)
    }

    fn set_acc_reward_per_share_p1(env: &Env, value: i128) {
        env.storage().persistent().set(&DataKey::AccRewardPerShareP1, &value);
    }

    pub fn get_total_supplied_p1(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::TotalSuppliedP1).unwrap_or(0)
    }

    fn set_total_supplied_p1(env: &Env, value: i128) {
        env.storage().persistent().set(&DataKey::TotalSuppliedP1, &value);
    }

    fn get_last_reward_balance_p1(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::LastRewardBalanceP1).unwrap_or(0)
    }

    fn set_last_reward_balance_p1(env: &Env, value: i128) {
        env.storage().persistent().set(&DataKey::LastRewardBalanceP1, &value);
    }

    pub fn get_acc_reward_per_share_p2(env: &Env) -> i128 {
    env.storage().persistent().get(&DataKey::AccRewardPerShareP2).unwrap_or(0)
    }

    fn set_acc_reward_per_share_p2(env: &Env, value: i128) {
        env.storage().persistent().set(&DataKey::AccRewardPerShareP2, &value);
    }

    pub fn get_total_supplied_p2(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::TotalSuppliedP2).unwrap_or(0)
    }

    fn set_total_supplied_p2(env: &Env, value: i128) {
        env.storage().persistent().set(&DataKey::TotalSuppliedP2, &value);
    }

    fn get_last_reward_balance_p2(env: &Env) -> i128 {
        env.storage().persistent().get(&DataKey::LastRewardBalanceP2).unwrap_or(0)
    }

    fn set_last_reward_balance_p2(env: &Env, value: i128) {
        env.storage().persistent().set(&DataKey::LastRewardBalanceP2, &value);
    }

    pub fn get_treasury_address(env: &Env) -> Address {
        env.storage().persistent().get(&DataKey::Treasury).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }

    fn get_p1_token_address(env: &Env) -> Address {
        env.storage().persistent().get(&DataKey::P1Token).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
    }


    fn get_vested_packets(env: &Env, schedule: &VestingSchedule) -> u32 {
        if schedule.is_cancelled {
            return schedule.claimed_packets;
        }
        let now = env.ledger().timestamp();
        if now < schedule.start_timestamp {
            return 0;
        }
        let elapsed = now - schedule.start_timestamp;
        let seconds_per_year: u64 = 31_536_000;
        let total_duration = (schedule.num_years as u64) * seconds_per_year;
        let interval = total_duration / (schedule.frequency as u64);
        if interval == 0 {
            return 0;
        }
        let periods = (elapsed / interval) as u32;
        periods.min(schedule.num_years * schedule.frequency)
    }

    // Get current vested + claimable main amount for a vault (beneficiary view)
    pub fn get_claimable_main(env: Env, vault_id: u64) -> i128 {
        let schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        if schedule.is_cancelled {
            return 0;
        }

        let vested_packets = Self::get_vested_packets(&env, &schedule);
        let due = vested_packets.saturating_sub(schedule.claimed_packets);
        if due == 0 {
            return 0;
        }

        let total_packets = (schedule.num_years * schedule.frequency) as u32;
        if schedule.claimed_packets + due >= total_packets {
            // Final claim: full remaining
            let already_released = schedule.packet_amount * (schedule.claimed_packets as i128);
            schedule.total_amount - already_released
        } else {
            schedule.packet_amount * (due as i128)
        }
    }

    // Get current claimable reward for sender or beneficiary
    pub fn get_claimable_reward(env: Env, vault_id: u64, claimant: Address) -> i128 {
        let reward_info: RewardInfo = Self::get_reward(env.clone(), vault_id);
        let schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        if schedule.is_cancelled {
            return 0;
        }

        let vested = Self::get_vested_packets(&env, &schedule) as i128;
        let total_packets = (schedule.num_years * schedule.frequency) as i128;
        if total_packets == 0 {
            return 0;
        }

        if claimant == schedule.sender {
            let per_packet = reward_info.sender_share / total_packets;
            let base_due = vested * per_packet - reward_info.claimed_sender;
            if vested >= total_packets {
                reward_info.sender_share - reward_info.claimed_sender  // final remainder
            } else {
                base_due.max(0)
            }
        } else if claimant == schedule.beneficiary {
            let per_packet = reward_info.receiver_share / total_packets;
            let base_due = vested * per_packet - reward_info.claimed_receiver;
            if vested >= total_packets {
                reward_info.receiver_share - reward_info.claimed_receiver
            } else {
                base_due.max(0)
            }
        } else {
            0
        }
    }

    // Check if a vault is fully vested
    pub fn is_fully_vested(env: Env, vault_id: u64) -> bool {
        let schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        let vested = Self::get_vested_packets(&env, &schedule);
        vested >= schedule.num_years * schedule.frequency
    }
    
    pub fn get_idle_amount(env: Env, vault_id: u64) -> i128 {
        let schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        schedule.idle_amount
    }

    // Global stats (nice for dashboard)
    pub fn get_next_vault_id(env: Env) -> u64 {
        env.storage().persistent()
            .get(&DataKey::NextVaultId)
            .unwrap_or(0)
    }

    pub fn get_total_vaults(env: Env) -> u64 {
        Self::get_next_vault_id(env)  // since we increment from 0
    }

    // Optional: Contract-level MIRAI info
    pub fn get_mirai_balance(env: Env) -> i128 {
        let mirai_addr = Self::get_mirai_address(&env);
        let client = token::Client::new(&env, &mirai_addr);
        client.balance(&env.current_contract_address())
    }

    pub fn get_true_balance_public(env: Env) -> i128 {
        Self::get_true_balance(&env)
    }


    fn sqrt(n: i128) -> i128 {
        if n == 0 || n == 1{
            return n;
        }

        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x{
            x = y;
            y = (x + n / x) / 2;
        }
        return x;
    }

    fn calculate_reward(env: &Env, commitment: i128, time: u32) -> i128 {
        let true_bal = Self::get_true_balance(env);
        let supply = Self::get_total_supply(env);

        let rfactor = supply.saturating_div(true_bal);

        let term: i128;
        let algo = match rfactor {
            1..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            _ => 5
        };

        let adj = Self::get_adjustment_factor(env);
        let cf = Self::get_c_factor(env);
        let df = Self::get_d_factor(env);

        if algo == 1 {
            term = (9 * adj * Self::sqrt(true_bal)) / (Self::sqrt(supply) * 1);
        }
        else if algo == 2 {
            term = (8 * adj * Self::sqrt(true_bal)) / (Self::sqrt(supply) * 1);
        }
        else if algo == 3 {
            term = (6 * adj * Self::sqrt(true_bal)) / (Self::sqrt(supply) * 1);
        }
        else if algo == 4 {
            term = (cf * 2 as i128 * time as i128).max((35 * adj * Self::sqrt(true_bal)) / (Self::sqrt(supply) * 10 * df));
        }
        else {
            term = (cf * time as i128).max((adj * Self::sqrt(true_bal)) / (Self::sqrt(supply) * df));
        }



        let largeness_factor_num = Self::sqrt(commitment.min(MAX_LARGENESS));
        let amount = (largeness_factor_num.saturating_mul(term)).saturating_div(Self::sqrt(MAX_LARGENESS));

        if true_bal < amount {
            panic!("Insufficient Mirai Balance for Reward. Please try again later");
        }
        amount
    }

    // Protocol helpers (p1 = Blend)
    fn supply_to_p1_internal(env: &Env, schedule: &VestingSchedule, caller: &Address, amount: i128) {
        if schedule.p1_ratio == 0 || schedule.p1_pool.is_none() || amount <= 0 { return; }
        let pool = schedule.p1_pool.clone().unwrap();

        // ---- test version ----
        #[cfg(test)]
        {
            let mock_client = mock_protocol::MockProtocolClient::new(env, &pool);
            mock_client.supply(&schedule.vault_id, &caller, &amount);
        }

        // ---- production version ----
        #[cfg(not(test))]
        {
            let pool_addr = schedule.p1_pool.clone().unwrap();
            let asset = schedule.token_address.clone();
            let vault_addr = env.current_contract_address();




            let mut requests = Vec::new(env);
            requests.push_back(Request {
                request_type: REQUEST_TYPE_SUPPLY_COLLATERAL,
                address: asset,
                amount,
            });

            let auth_entry = InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: schedule.token_address.clone(),
                    fn_name: Symbol::new(env, "transfer"),
                    args: (
                        vault_addr.clone(),
                        pool_addr.clone(),
                        amount
                    ).into_val(env),
                },
                sub_invocations: Vec::new(env),
            });


            env.authorize_as_current_contract(Vec::from_array(env, [auth_entry]));

            let client = PoolClient::new(env, &pool_addr);
            // We still call submit so Blend credits the position to the vault
            let _positions = client.submit(&vault_addr, &vault_addr, &vault_addr, &requests);
            return;
        }
    }

    fn withdraw_from_p1_internal(env: &Env, schedule: &VestingSchedule, amount: i128, to: &Address) {
        if schedule.p1_ratio == 0 || schedule.p1_pool.is_none() || amount <= 0 { return; }
        let pool = schedule.p1_pool.clone().unwrap();

        // ---- test version ----
        #[cfg(test)]
        {
            let mock_client = mock_protocol::MockProtocolClient::new(env, &pool);
            mock_client.withdraw(&schedule.vault_id, &amount, to);
        }

        // ---- production version ----

        #[cfg(not(test))]
        {
            let pool_addr = schedule.p1_pool.clone().unwrap();
            let asset = schedule.token_address.clone();
            let vault = env.current_contract_address();

            let mut requests = Vec::new(env);
            requests.push_back(Request {
                request_type: REQUEST_TYPE_WITHDRAW_COLLATERAL,
                address: asset,
                amount,
            });

            let client = PoolClient::new(env, &pool_addr);
            // Tokens will be sent to `to` (normally the vault itself)
            let _positions = client.submit(&vault, &vault, to, &requests);
            return;
        }
    }


    // Protocol helpers (p2 = DeFindex)
    fn supply_to_p2_internal(env: &Env, schedule: &VestingSchedule, caller: &Address, amount: i128) -> i128 {
        if schedule.p2_ratio == 0 || schedule.p2_pool.is_none() || amount <= 0 {
            return 0;
        }

        let defindex_vault = schedule.p2_pool.clone().unwrap();
        let vault_addr = env.current_contract_address();


        // ---- test version ----
        #[cfg(test)]
        {
            let mock_client = mock_defindex::MockDefindexClient::new(env, &defindex_vault);
            mock_client.supply(&schedule.vault_id, caller, &amount);
            return amount; // in tests we just pretend 1:1 shares
        }

        // ---- production version ----
        #[cfg(not(test))]
        {
            let client = DefindexClient::new(env, &defindex_vault);
            let asset = schedule.token_address.clone();
            //let token_client = token::Client::new(env, &asset);


            let mut amounts_desired = Vec::new(env);
            amounts_desired.push_back(amount);

            let mut amounts_min = Vec::new(env);
            amounts_min.push_back(0);

            let auth_entry = InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: schedule.token_address.clone(),
                    fn_name: Symbol::new(env, "transfer"),
                    args: (
                        vault_addr.clone(),
                        defindex_vault.clone(),
                        amount
                    ).into_val(env),
                },
                sub_invocations: Vec::new(env),
            });

            env.authorize_as_current_contract(Vec::from_array(env, [auth_entry]));

            let shares_received = client.deposit(&amounts_desired, &amounts_min, &vault_addr, &true);

            shares_received
        }
    }




    fn withdraw_from_p2_internal(
        env: &Env,
        schedule: &VestingSchedule,
        amount: i128,
        to: &Address,
    ) -> (i128, i128) { // returns (underlying_received, shares_burned)
        if schedule.p2_ratio == 0 || schedule.p2_pool.is_none() || amount <= 0 {
            return (0, 0);
        }

        let defindex_vault = schedule.p2_pool.clone().unwrap();
        let vault_addr = env.current_contract_address();

        // ---------- TEST VERSION ----------
        #[cfg(test)]
        {
            let mock_client = mock_defindex::MockDefindexClient::new(env, &defindex_vault);
            let token_client = token::Client::new(env, &schedule.token_address);
            let bal_before = token_client.balance(&env.current_contract_address());
            mock_client.withdraw(&schedule.vault_id, &amount, to);
            let bal_after = token_client.balance(&env.current_contract_address());
            let underlying_received = bal_after - bal_before;
            let shares_burned = amount;
            return (underlying_received, shares_burned); // 1:1 in tests
        }

        // ---------- PRODUCTION VERSION ----------
        #[cfg(not(test))]
        {
            let client = DefindexClient::new(env, &defindex_vault);
            let token_client = token::Client::new(env, &schedule.token_address);

            let total_shares = client.total_supply();
            let managed_funds = client.fetch_total_managed_funds();
            let total_underlying = managed_funds.get(0).unwrap_or(0);

            if total_shares == 0 || total_underlying == 0 {
                return (0, 0);
            }

            // How many shares to burn to get ~`amount` of underlying
            let shares_to_burn = (amount * total_shares) / total_underlying;
            if shares_to_burn <= 0 {
                return (0, 0);
            }

            let mut min_amounts_out = Vec::new(env);
            min_amounts_out.push_back(0);

            let amounts_out = client.withdraw(shares_to_burn, &min_amounts_out, to);

            let underlying_received = amounts_out.get(0).unwrap_or(0);

            (underlying_received, shares_to_burn)
        }
    }





    // ================== CREATE FUND ==================
    pub fn create_fund(
        env: Env,
        sender: Address,
        beneficiary: Address,
        token_address: Address,
        total_amount: i128,
        num_years: u32,
        frequency: u32,
        reward_split: u32,
        p1_pool: Option<Address>,
        p1_ratio: u32,
        p2_pool: Option<Address>,
        p2_ratio: u32,
    ) -> u64 {
        sender.require_auth();

        let stellar_only_requirement = Self::is_only_stellar(&env);
        let expected_xlm = Address::from_str(&env, TESTNET_NATIVE_XLM);

        if stellar_only_requirement && token_address != expected_xlm {
            panic!("This contract allows only stellar token to be Vaulted.");
        }

        if num_years < 4 || num_years > 30 { panic!("num_years must be 4-30"); }
        if frequency != 1 && frequency != 2 && frequency != 4 && frequency != 365 { panic!("frequency must be 1, 2, 4 or 365"); }
        if total_amount < MIN_VAULT_AMT { panic!("total amount must be more than 100 XLM"); }
        if reward_split > 100 { panic!("reward_split 0-100"); }

        if p1_ratio > 50 || p2_ratio > 50 {
            panic!("No protocol can be allocated more than 50% of the funds.")
        }
        if p1_ratio + p2_ratio > 70 {
            panic!("Not more than 70% of the funds might be used up in third party protocols.");
        }

        if p1_ratio != 0 && p1_pool.is_none() { panic!("Blend Pool required"); }
        if p2_ratio != 0 && p2_pool.is_none() { panic!("DeFindex Pool required"); }


        let mut next_id: u64 = env.storage().persistent().get(&DataKey::NextVaultId).unwrap_or(0);
        let vault_id = next_id;
        next_id += 1;
        env.storage().persistent().set(&DataKey::NextVaultId, &next_id);

        let total_packets = (num_years * frequency) as i128;
        let base_packet = total_amount / total_packets;

        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&sender, &env.current_contract_address(), &total_amount);

        let lwf;
        if p1_ratio != 0 {
            lwf = 0;
        }
        else if p2_ratio != 0 {
            lwf = 1;
        }
        else {
            lwf = 2;
        }

        let schedule = VestingSchedule {
            vault_id,
            sender: sender.clone(),
            beneficiary: beneficiary.clone(),
            token_address,
            total_amount,
            num_years,
            frequency,
            reward_split,
            packet_amount: base_packet,
            start_timestamp: env.ledger().timestamp(),
            claimed_packets: 0,
            is_cancelled: false,
            idle_amount: total_amount,
            p1_pool,
            p2_pool,
            p1_ratio,
            p2_ratio,
            supplied_p1: 0,
            supplied_p2: 0,
            p2_shares: 0,
            p2_principal: 0,
            last_withdraw_from: lwf,
            reward_debt_p1: 0,
            reward_debt_p2: 0,
        };

        let total_reward = Self::calculate_reward(&env, total_amount, num_years);
        let sender_reward = (total_reward * (reward_split as i128)) / 100;
        let receiver_reward = total_reward - sender_reward;

        let mut tb = Self::get_true_balance(&env);
        tb -= total_reward;
        if tb < 0 {
            panic!("True Balance would go to 0. Skipping Transaction.");
        }
        Self::set_true_balance(&env, tb);

        let reward_info = RewardInfo {
            vault_id,
            total_reward,
            sender_share: sender_reward,
            receiver_share: receiver_reward,
            claimed_sender: 0,
            claimed_receiver: 0,
        };

        let storage = env.storage().persistent();

        storage.set(&DataKey::Vault(vault_id), &schedule);
        storage.set(&DataKey::Reward(vault_id), &reward_info);

        // Update sender vaults list
        let mut sender_vaults: Vec<u64> = storage.get(&DataKey::SenderVaults(sender.clone())).unwrap_or_else(|| Vec::<u64>::new(&env));
        sender_vaults.push_back(vault_id);
        storage.set(&DataKey::SenderVaults(sender.clone()), &sender_vaults);

        // Update beneficiary vaults list
        let mut beneficiary_vaults: Vec<u64> = storage.get(&DataKey::BeneficiaryVaults(beneficiary.clone())).unwrap_or_else(|| Vec::<u64>::new(&env));
        beneficiary_vaults.push_back(vault_id);
        storage.set(&DataKey::BeneficiaryVaults(beneficiary.clone()), &beneficiary_vaults);

        // Mint MIRAI reward to vault (adjust according to your MiraiToken mint logic)
        // let mirai_client = token::Client::new(&env, &Self::get_mirai_address(&env));
        // If your token allows public mint or vault is authorized:
        // mirai_client.mint(&env.current_contract_address(), &total_reward);



        FundCreatedEvent {
            vault_id,
            sender,
            beneficiary,
            total_amount,
            num_years,
            frequency,
            reward_split,
            total_reward,
            p1_ratio,
            p2_ratio,
        }.publish(&env);

        vault_id
    }

    // ================== QUERY FUNCTIONS ==================
    pub fn get_vaults_by_me(env: Env, sender: Address) -> Vec<u64> {
        env.storage().persistent()
            .get(&DataKey::SenderVaults(sender))
            .unwrap_or_else(|| Vec::<u64>::new(&env))
    }

    pub fn get_vaults_for_me(env: Env, beneficiary: Address) -> Vec<u64> {
        env.storage().persistent()
            .get(&DataKey::BeneficiaryVaults(beneficiary))
            .unwrap_or_else(|| Vec::<u64>::new(&env))
    }

    pub fn get_vault(env: Env, vault_id: u64) -> VestingSchedule {
        env.storage().persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic!("Vault not found"))
    }

    pub fn get_reward(env: Env, vault_id: u64) -> RewardInfo {
        env.storage().persistent()
            .get(&DataKey::Reward(vault_id))
            .unwrap_or_else(|| panic!("Reward not found"))
    }

    // ================== CLAIM MAIN (only beneficiary) ==================
    pub fn claim_main(env: Env, vault_id: u64, beneficiary: Address) {
        beneficiary.require_auth();
        Self::bump_vault_storage(&env, vault_id);

        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        if schedule.beneficiary != beneficiary {
            panic!("Only beneficiary can claim main");
        }
        if schedule.is_cancelled {
            panic!("Vault is cancelled");
        }

        let vested = Self::get_vested_packets(&env, &schedule);
        let due = vested.saturating_sub(schedule.claimed_packets);

        if due == 0 {
            panic!("No main packets due yet");
        }

        let total_packets = (schedule.num_years * schedule.frequency) as u32;
        let is_final_claim = (schedule.claimed_packets + due) >= total_packets;

        let token_client = token::Client::new(&env, &schedule.token_address);

        let mut amount_due = schedule.packet_amount * (due as i128);

        if is_final_claim {
            let total_released_so_far = schedule.packet_amount * (schedule.claimed_packets as i128);
            let total_should_be_released = schedule.total_amount; // exact original amount

            amount_due = total_should_be_released - total_released_so_far;
        }

        if amount_due <= 0 {
            panic!("Nothing to claim");
        }

        let mut schedule = Self::get_vault(env.clone(), vault_id);

        if is_final_claim {
            if schedule.supplied_p1 > 0 {
                Self::safe_withdraw_from_p1(env.clone(), vault_id, beneficiary.clone(), schedule.supplied_p1);
            }
            if schedule.supplied_p2 > 0 {
                Self::safe_withdraw_from_p2(env.clone(), vault_id, beneficiary.clone(), schedule.supplied_p2);
            }
            // Re-fetch after pulls
            schedule = Self::get_vault(env.clone(), vault_id);
        }

        if !is_final_claim && amount_due > schedule.idle_amount {
            let deficit = amount_due - schedule.idle_amount;



            if schedule.p1_ratio != 0 && schedule.p2_ratio != 0
                && schedule.supplied_p1 > 0 && schedule.supplied_p2 > 0
            {
                if schedule.last_withdraw_from == 0 {
                    if schedule.supplied_p2 >= deficit {
                        Self::safe_withdraw_from_p2(env.clone(), vault_id, beneficiary.clone(), deficit);
                        schedule.last_withdraw_from = 1;
                    } else {
                        let p2_amount = schedule.supplied_p2;
                        Self::safe_withdraw_from_p2(env.clone(), vault_id, beneficiary.clone(), p2_amount);
                        Self::safe_withdraw_from_p1(env.clone(), vault_id, beneficiary.clone(), deficit - p2_amount);
                        schedule.last_withdraw_from = 1;
                    }
                } else {
                    if schedule.supplied_p1 >= deficit {
                        Self::safe_withdraw_from_p1(env.clone(), vault_id, beneficiary.clone(), deficit);
                        schedule.last_withdraw_from = 0;
                    } else {
                        let p1_amount = schedule.supplied_p1;
                        Self::safe_withdraw_from_p1(env.clone(), vault_id, beneficiary.clone(), p1_amount);
                        Self::safe_withdraw_from_p2(env.clone(), vault_id, beneficiary.clone(), deficit - p1_amount);
                        schedule.last_withdraw_from = 0;
                    }
                }
            } else if schedule.supplied_p1 > 0 {
                Self::safe_withdraw_from_p1(env.clone(), vault_id, beneficiary.clone(), deficit);
                schedule.last_withdraw_from = 0;
            } else if schedule.supplied_p2 > 0 {
                Self::safe_withdraw_from_p2(env.clone(), vault_id, beneficiary.clone(), deficit);
                schedule.last_withdraw_from = 1;
            } else {
                panic!("Insufficient token balance in your vault");
            }

            schedule = Self::get_vault(env.clone(), vault_id);
        }

        // Now we can trust schedule.idle_amount
        if schedule.idle_amount < amount_due {
            panic!("Insufficient token balance in contract");
        }

        let token_client = token::Client::new(&env, &schedule.token_address);
        token_client.transfer(&env.current_contract_address(), &beneficiary, &amount_due);

        schedule.claimed_packets += due;
        schedule.idle_amount -= amount_due;

        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);

        MainClaimedEvent {
            vault_id,
            beneficiary,
            amount: amount_due,
            packets: due,
        }
        .publish(&env);
    }

    // ================== CLAIM REWARD (sender or beneficiary) ==================
    pub fn claim_reward(env: Env, vault_id: u64, claimant: Address) {
        claimant.require_auth();
        Self::bump_vault_storage(&env, vault_id);

        let mut reward_info: RewardInfo = Self::get_reward(env.clone(), vault_id);
        let schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);

        if schedule.is_cancelled {
            panic!("Vault is cancelled");
        }

        let vested = Self::get_vested_packets(&env, &schedule) as i128;
        let total_packets = (schedule.num_years * schedule.frequency) as i128;
        if total_packets == 0 {
            panic!("Invalid schedule");
        }

        let (amount, is_sender, new_claimed) = if claimant == schedule.sender {
            let per_packet = reward_info.sender_share / total_packets;
            let base_due = vested * per_packet - reward_info.claimed_sender;

            // On final packet, add any remainder
            let is_final = vested >= total_packets;
            let due = if is_final {
                reward_info.sender_share - reward_info.claimed_sender
            } else {
                base_due
            };

            if due <= 0 { panic!("No sender reward due yet"); }

            (due, true, reward_info.claimed_sender + due)
        } else if claimant == schedule.beneficiary {
            let per_packet = reward_info.receiver_share / total_packets;
            let base_due = vested * per_packet - reward_info.claimed_receiver;

            let is_final = vested >= total_packets;
            let due = if is_final {
                reward_info.receiver_share - reward_info.claimed_receiver
            } else {
                base_due
            };

            if due <= 0 { panic!("No receiver reward due yet"); }

            (due, false, reward_info.claimed_receiver + due)
        } else {
            panic!("Unauthorized claimant");
        };

        let mirai_client = token::Client::new(&env, &Self::get_mirai_address(&env));

        // Safety check
        let contract_mirai_bal = mirai_client.balance(&env.current_contract_address());
        if contract_mirai_bal < amount {
            panic!("Insufficient MIRAI balance in contract for reward");
        }

        mirai_client.transfer(&env.current_contract_address(), &claimant, &amount);

        // Update claimed amount
        if is_sender {
            reward_info.claimed_sender = new_claimed;  // wait, better to set directly
        } else {
            reward_info.claimed_receiver = new_claimed;
        }
        // (I adjusted the variable above for clarity — you can clean the assignment)

        env.storage().persistent().set(&DataKey::Reward(vault_id), &reward_info);

        RewardClaimedEvent {
            vault_id,
            claimant,
            amount,
            is_sender,
        }.publish(&env);
    }


    // ================== CANCEL REMAINING (only sender) ==================
    pub fn cancel_remaining(env: Env, vault_id: u64, sender: Address) {
        sender.require_auth();
        Self::bump_vault_storage(&env, vault_id);

        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        if schedule.sender != sender {
            panic!("Only sender can cancel");
        }
        if schedule.is_cancelled {
            panic!("Already cancelled");
        }

        // 1. Pull everything back from protocols
        if schedule.supplied_p1 > 0 {
            Self::safe_withdraw_from_p1(env.clone(), vault_id, sender.clone(), schedule.supplied_p1);
        }
        if schedule.supplied_p2 > 0 {
            Self::safe_withdraw_from_p2(env.clone(), vault_id, sender.clone(), schedule.supplied_p2);
        }

        // 2. Re-fetch after the safe withdraws (critical)
        schedule = Self::get_vault(env.clone(), vault_id);

        // 3. Everything that is now idle belongs to the sender
        let amount_to_return = schedule.idle_amount;

        if amount_to_return > 0 {
            let token_client = token::Client::new(&env, &schedule.token_address);
            token_client.transfer(&env.current_contract_address(), &sender, &amount_to_return);
        }

        // 4. Return any unclaimed MIRAI reward to true_balance
        let reward_info: RewardInfo = Self::get_reward(env.clone(), vault_id);
        let remaining_reward = (reward_info.sender_share - reward_info.claimed_sender)
            + (reward_info.receiver_share - reward_info.claimed_receiver);

        if remaining_reward > 0 {
            let mut tb = Self::get_true_balance(&env);
            tb += remaining_reward;
            Self::set_true_balance(&env, tb);
        }

        // 5. Mark as cancelled and zero everything
        schedule.is_cancelled = true;
        schedule.supplied_p1 = 0;
        schedule.supplied_p2 = 0;
        schedule.p2_shares = 0;
        schedule.p2_principal = 0;
        schedule.idle_amount = 0;

        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);

        VaultCancelledEvent {
            vault_id,
            sender,
            beneficiary: schedule.beneficiary,
        }
        .publish(&env);
    }


    pub fn supply_to_p1(env: Env, vault_id: u64, caller: Address, amount: i128) {
        caller.require_auth();
        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);

        if schedule.sender != caller { panic!("Only sender can supply"); }
        if schedule.is_cancelled { panic!("Vault cancelled"); }
        if schedule.p1_pool.is_none() { panic!("P1 not configured"); }
        if schedule.p1_ratio == 0 { panic!("P1 not authorized during vault creation"); }
        if amount <= 0 { panic!("Invalid amount"); }

        if amount > schedule.idle_amount { panic!("Insufficient idle balance"); }

        let potential_amount_protocol = amount + schedule.supplied_p1;
        if potential_amount_protocol > (50 * schedule.total_amount) / 100 {
            panic!("Cannot allocate more than 50% to one protocol");
        }
        if potential_amount_protocol > (schedule.p1_ratio as i128 * schedule.total_amount) / 100 {
            panic!("Cannot allocate more than the initially chosen protocol ratio");
        }
        let potential_amount_total = amount + schedule.supplied_p1 + schedule.supplied_p2;
        if potential_amount_total > (70 * schedule.total_amount) / 100 {
            panic!("Cannot allocate more than 70% to all external protocols");
        }

        // Actual supply
        Self::supply_to_p1_internal(&env, &schedule, &env.current_contract_address(), amount);

        // Update accounting only
        schedule.supplied_p1 += amount;
        schedule.idle_amount -= amount;

        // Update global total
        let mut total = Self::get_total_supplied_p1(&env);
        total += amount;
        Self::set_total_supplied_p1(&env, total);

        // Update reward debt so future rewards are calculated correctly
        schedule.reward_debt_p1 = (schedule.supplied_p1 * Self::get_acc_reward_per_share_p1(&env)) / REWARD_PRECISION;

        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);
    }


    pub fn supply_to_p2(env: Env, vault_id: u64, caller: Address, amount: i128) {
        caller.require_auth();
        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        // Only sender can supply
        if schedule.sender != caller { panic!("Only sender can supply"); }
        if schedule.is_cancelled { panic!("Vault cancelled"); }
        if schedule.p2_pool.is_none() { panic!("P2 not configured"); }
        if schedule.p2_ratio == 0 { panic!("P2 not authorized during vault creation") }
        if amount <= 0 { panic!("Invalid amount"); }

        let idle = schedule.idle_amount; // implement based on total - supplied
        if amount > idle { panic!("Insufficient idle balance"); }

        let potential_amount_protocol = amount + schedule.supplied_p2;
        if potential_amount_protocol > ((50 as i128) * schedule.total_amount) / (100 as i128) { panic!("Cannot allocate more than 50% to one protocol") };
        if potential_amount_protocol > ((schedule.p2_ratio as i128) * schedule.total_amount) / (100 as i128) { panic!("Cannot allocate more than the initially chosen protocol ratio") };
        let potential_amount_total = amount + schedule.supplied_p1 + schedule.supplied_p2;
        if potential_amount_total > ((70 as i128) * schedule.total_amount) / (100 as i128) { panic!("Cannot allocate more than 70% to all external protocols")};

        // Actual deposit (returns shares received)
        let shares_received = Self::supply_to_p2_internal(&env, &schedule, &env.current_contract_address(), amount);

        if shares_received <= 0 {
            panic!("Cannot have Negative Shares");
        }

        // Update accounting
        schedule.supplied_p2 += amount;
        schedule.p2_shares += shares_received;
        schedule.p2_principal += amount;
        schedule.idle_amount -= amount;

        // Update global total supplied
        let mut total = Self::get_total_supplied_p2(&env);
        total += amount;
        Self::set_total_supplied_p2(&env, total);

        // Persist
        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);
    }

    pub fn withdraw_from_p1(env: Env, vault_id: u64, caller: Address, amount: i128) {
        caller.require_auth();
        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);

        if schedule.sender != caller && schedule.beneficiary != caller {
            panic!("Only sender or beneficiary can withdraw from protocol");
        }
        if schedule.is_cancelled { panic!("Vault cancelled"); }
        if schedule.p1_pool.is_none() || amount <= 0 { panic!("Invalid request"); }
        if schedule.p1_ratio == 0 { panic!("P1 not authorized during vault creation"); }
        if amount > schedule.supplied_p1 { panic!("Insufficient supplied balance to protocol"); }

        // 1. Pull from Blend
        Self::withdraw_from_p1_internal(&env, &schedule, amount, &env.current_contract_address());

        // 2. Harvest any pending BLND rewards (safe version)
        let blnd = Self::get_p1_token_address(&env);
        Self::update_p1_rewards(&env, &blnd);

        let pending = Self::pending_p1_reward(&env, &schedule);
        if pending > 0 {
            let fee = (pending * PROTOCOL_FEE_BPS as i128) / 10_000;
            let user_share = pending - fee;

            let reward_client = token::Client::new(&env, &blnd);
            let treasury = Self::get_treasury_address(&env);

            // Only transfer if the contract actually has the tokens
            let contract_bal = reward_client.balance(&env.current_contract_address());
            if contract_bal >= pending {
                if user_share > 0 {
                    reward_client.transfer(&env.current_contract_address(), &schedule.sender, &user_share);
                }
                if fee > 0 {
                    reward_client.transfer(&env.current_contract_address(), &treasury, &fee);
                }
                // Update last balance after successful transfer
                let new_balance = reward_client.balance(&env.current_contract_address());
                Self::set_last_reward_balance_p1(&env, new_balance);
            }
        }

        // 3. Update accounting
        schedule.supplied_p1 -= amount;
        schedule.idle_amount += amount;
        schedule.reward_debt_p1 = (schedule.supplied_p1 * Self::get_acc_reward_per_share_p1(&env)) / REWARD_PRECISION;

        let mut total = Self::get_total_supplied_p1(&env);
        total -= amount;
        Self::set_total_supplied_p1(&env, total);

        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);
    }


    pub fn withdraw_from_p2(env: Env, vault_id: u64, caller: Address, amount: i128) {
        caller.require_auth();
        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);
        if schedule.sender != caller && schedule.beneficiary != caller {
            panic!("Only sender or beneficiary can withdraw from protocol");
        }
        if schedule.is_cancelled { panic!("Vault cancelled"); }
        if schedule.p2_pool.is_none() || amount <= 0 { panic!("Invalid request"); }
        if schedule.p2_ratio == 0 { panic!("P2 not authorized during vault creation"); }
        if amount > schedule.supplied_p2 { panic!("Insufficient supplied balance to protocol"); }

        let (underlying_received, shares_burned) =
        Self::withdraw_from_p2_internal(&env, &schedule, amount, &env.current_contract_address());

        if shares_burned == 0 {
            panic!("Could not withdraw from DeFindex");
        }

        // Calculate principal vs profit
        let principal_portion = if schedule.p2_shares > 0 {
            (shares_burned * schedule.p2_principal) / schedule.p2_shares
        } else {
            0
        };

        let profit = underlying_received - principal_portion;

        // Take 5% fee only on the real yield
        if profit > 0 {
            let fee = (profit * PROTOCOL_FEE_BPS as i128) / 10_000;
            let user_profit = profit - fee;

            let token_client = token::Client::new(&env, &schedule.token_address);
            let treasury = Self::get_treasury_address(&env);

            if user_profit > 0 {
                token_client.transfer(&env.current_contract_address(), &schedule.sender, &user_profit);
            }
            if fee > 0 {
                token_client.transfer(&env.current_contract_address(), &treasury, &fee);
            }
        }

        // Update accounting
        schedule.p2_shares = schedule.p2_shares.saturating_sub(shares_burned);
        schedule.p2_principal = schedule.p2_principal.saturating_sub(principal_portion);
        schedule.supplied_p2 = schedule.supplied_p2.saturating_sub(principal_portion);
        schedule.idle_amount += principal_portion;

        // Global total
        let mut total = Self::get_total_supplied_p2(&env);
        total = total.saturating_sub(principal_portion);
        Self::set_total_supplied_p2(&env, total);

        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);
    }

    fn safe_withdraw_from_p1(env: Env, vault_id: u64, caller: Address, amount: i128) {
        if amount <= 0 {
            return;
        }

        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);

        if schedule.sender != caller && schedule.beneficiary != caller {
            panic!("Only sender or beneficiary can withdraw from protocol");
        }
        if schedule.is_cancelled { panic!("Vault cancelled"); }
        if schedule.p1_pool.is_none() || amount <= 0 { panic!("Invalid request"); }
        if schedule.p1_ratio == 0 { panic!("P1 not authorized during vault creation"); }
        if amount > schedule.supplied_p1 { panic!("Insufficient supplied balance to protocol"); }

        Self::withdraw_from_p1_internal(&env, &schedule, amount, &env.current_contract_address());

        // Same safe reward harvesting
        let blnd = Self::get_p1_token_address(&env);
        Self::update_p1_rewards(&env, &blnd);

        let pending = Self::pending_p1_reward(&env, &schedule);
        if pending > 0 {
            let fee = (pending * PROTOCOL_FEE_BPS as i128) / 10_000;
            let user_share = pending - fee;

            let reward_client = token::Client::new(&env, &blnd);
            let treasury = Self::get_treasury_address(&env);

            let contract_bal = reward_client.balance(&env.current_contract_address());
            if contract_bal >= pending {
                if user_share > 0 {
                    reward_client.transfer(&env.current_contract_address(), &schedule.sender, &user_share);
                }
                if fee > 0 {
                    reward_client.transfer(&env.current_contract_address(), &treasury, &fee);
                }
                let new_balance = reward_client.balance(&env.current_contract_address());
                Self::set_last_reward_balance_p1(&env, new_balance);
            }
        }

        schedule.supplied_p1 -= amount;
        schedule.idle_amount += amount;
        schedule.reward_debt_p1 = (schedule.supplied_p1 * Self::get_acc_reward_per_share_p1(&env)) / REWARD_PRECISION;

        let mut total = Self::get_total_supplied_p1(&env);
        total -= amount;
        Self::set_total_supplied_p1(&env, total);

        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);
    }

    fn safe_withdraw_from_p2(env: Env, vault_id: u64, caller: Address, amount: i128) {
        if amount <= 0 {
            return;
        }

        let mut schedule: VestingSchedule = Self::get_vault(env.clone(), vault_id);


        if schedule.sender != caller && schedule.beneficiary != caller {
            panic!("Only sender or beneficiary can withdraw from protocol");
        }
        if schedule.is_cancelled { panic!("Vault cancelled"); }
        if schedule.p2_pool.is_none() || amount <= 0 { panic!("Invalid request"); }
        if schedule.p2_ratio == 0 { panic!("P2 not authorized during vault creation"); }
        if amount > schedule.supplied_p2 { panic!("Insufficient supplied balance to protocol"); }

         let (underlying_received, shares_burned) =
        Self::withdraw_from_p2_internal(&env, &schedule, amount, &env.current_contract_address());

        if shares_burned == 0 {
            panic!("Could not withdraw from DeFindex");
        }

        // Calculate principal vs profit
        let principal_portion = if schedule.p2_shares > 0 {
            (shares_burned * schedule.p2_principal) / schedule.p2_shares
        } else {
            0
        };

        let profit = underlying_received - principal_portion;

        // Take 5% fee only on the real yield
        if profit > 0 {
            let fee = (profit * PROTOCOL_FEE_BPS as i128) / 10_000;
            let user_profit = profit - fee;

            let token_client = token::Client::new(&env, &schedule.token_address);
            let treasury = Self::get_treasury_address(&env);

            if user_profit > 0 {
                token_client.transfer(&env.current_contract_address(), &schedule.sender, &user_profit);
            }
            if fee > 0 {
                token_client.transfer(&env.current_contract_address(), &treasury, &fee);
            }
        }

        // Update accounting
        schedule.p2_shares = schedule.p2_shares.saturating_sub(shares_burned);
        schedule.p2_principal = schedule.p2_principal.saturating_sub(principal_portion);
        schedule.supplied_p2 = schedule.supplied_p2.saturating_sub(principal_portion);
        schedule.idle_amount += principal_portion;

        // Global total
        let mut total = Self::get_total_supplied_p2(&env);
        total = total.saturating_sub(principal_portion);
        Self::set_total_supplied_p2(&env, total);


        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);



    }


    // ================== TIER 2 YIELD REWARD FUNCTIONS ==================

    fn update_p1_rewards(env: &Env, reward_token: &Address) {
        let total_supplied = Self::get_total_supplied_p1(env);


        let reward_client = token::Client::new(env, reward_token);
        let current_balance = reward_client.balance(&env.current_contract_address());
        let last_balance = Self::get_last_reward_balance_p1(env);

        if current_balance <= last_balance {
            return;
        }

        let new_rewards = current_balance - last_balance;

        if total_supplied == 0 {
            Self::set_last_reward_balance_p1(env, current_balance);
            return;
        }

        let acc = Self::get_acc_reward_per_share_p1(env);
        let new_acc = acc + (new_rewards * REWARD_PRECISION) / total_supplied;

        Self::set_acc_reward_per_share_p1(env, new_acc);
        Self::set_last_reward_balance_p1(env, current_balance);
    }



    fn pending_p1_reward(env: &Env, schedule: &VestingSchedule) -> i128 {
        let acc = Self::get_acc_reward_per_share_p1(env);
        let earned = (schedule.supplied_p1 * acc) / REWARD_PRECISION;
        earned.saturating_sub(schedule.reward_debt_p1)
    }



    pub fn claim_p1_rewards(env: Env, vault_id: u64, caller: Address) {
        caller.require_auth();
        let mut schedule = Self::get_vault(env.clone(), vault_id);

        if schedule.sender != caller {
            panic!("Only sender can claim protocol rewards");
        }
        if schedule.is_cancelled {
            panic!("Vault cancelled");
        }

        // 1. Update global accumulator
        let blnd_address = Self::get_p1_token_address(&env);
        Self::update_p1_rewards(&env, &blnd_address);

        // 2. Calculate pending
        let pending = Self::pending_p1_reward(&env, &schedule);
        if pending <= 0 {
            panic!("No rewards to claim");
        }

        // 3. Fee split
        let fee = (pending * PROTOCOL_FEE_BPS as i128) / 10000 as i128;          // 5% fee for MiraiVault to be able to afford food
        let user_share = pending - fee;

        let treasury: Address = env.storage().persistent().get(&DataKey::Treasury).unwrap();

        // 4. Transfer
        let reward_client = token::Client::new(&env, &blnd_address);
        reward_client.transfer(&env.current_contract_address(), &schedule.sender, &user_share);
        reward_client.transfer(&env.current_contract_address(), &treasury, &fee);

        // 5. Update debt
        schedule.reward_debt_p1 = (schedule.supplied_p1 * Self::get_acc_reward_per_share_p1(&env)) / REWARD_PRECISION;
        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);

        // Also update last_reward_balance because we just transferred tokens out
        let new_balance = reward_client.balance(&env.current_contract_address());
        Self::set_last_reward_balance_p1(&env, new_balance);

        ProtocolRewardClaimedEvent {
            vault_id,
            claimant: caller,
            protocol: 1,
            total_claimed: pending,
            user_share,
            fee,
        }.publish(&env);
    }




    // ================== THE GOOD OL' BUMP ============================

    fn bump_vault_storage(env: &Env, vault_id: u64){
        let storage = env.storage().persistent();
        storage.extend_ttl(
            &DataKey::Vault(vault_id),
            100_000,
            31_536_000 * 2,
        );

        storage.extend_ttl(
            &DataKey::Reward(vault_id),
            100_000,
            31_536_000 * 2,
        );
    }
}

mod test;


