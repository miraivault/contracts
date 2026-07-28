#![no_std]
use soroban_sdk::{contract, contractimpl, contractevent, contracttype, token, Address, Env, Vec};


const TESTNET_NATIVE_XLM: &str = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
const MIN_VAULT_AMT: i128 = 1_000_000_000;
const MAX_LARGENESS: i128 = 100_000_000_000;


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
    Admin,
    OnlyStellar,
    AdjustmentFactor,
    CFactor,
    DFactor,
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
    pub frequency: u32,           // 1, 2, or 4
    pub reward_split: u32,
    pub packet_amount: i128,
    pub start_timestamp: u64,
    pub claimed_packets: u32,
    pub is_cancelled: bool,
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

    pub fn initialize(env: Env, mirai_token: Address, total_supply: i128, initial_true_balance: i128, admin: Address, only_stellar_token: bool){
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
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::OnlyStellar, &only_stellar_token);

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

    fn get_admin(env: &Env) -> Address{
        env.storage().persistent().get(&DataKey::Admin).unwrap_or_else(|| panic!("Contract has not been initialized yet."))
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
    ) -> u64 {
        sender.require_auth();

        let stellar_only_requirement = Self::is_only_stellar(&env);
        let expected_xlm = Address::from_str(&env, TESTNET_NATIVE_XLM);

        if stellar_only_requirement && token_address != expected_xlm {
            panic!("This contract allows only stellar token to be Vaulted.");
        }

        if num_years < 4 || num_years > 30 { panic!("num_years must be 4-30"); }
        if frequency != 1 && frequency != 2 && frequency != 4 { panic!("frequency must be 1, 2 or 4"); }
        if total_amount < MIN_VAULT_AMT { panic!("total amount must be more than 100 XLM"); }
        if reward_split > 100 { panic!("reward_split 0-100"); }

        let mut next_id: u64 = env.storage().persistent().get(&DataKey::NextVaultId).unwrap_or(0);
        let vault_id = next_id;
        next_id += 1;
        env.storage().persistent().set(&DataKey::NextVaultId, &next_id);

        let total_packets = (num_years * frequency) as i128;
        let base_packet = total_amount / total_packets;

        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&sender, &env.current_contract_address(), &total_amount);

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

        // Calculate base amount due
        let mut amount_due = schedule.packet_amount * (due as i128);

        // If this claim completes the vesting, add any remainder to the beneficiary
        if is_final_claim {
            let total_released_so_far = schedule.packet_amount * (schedule.claimed_packets as i128);
            let total_should_be_released = schedule.total_amount; // exact original amount

            amount_due = total_should_be_released - total_released_so_far;
        }

        if amount_due <= 0 {
            panic!("Nothing to claim");
        }

        // Safety: ensure contract has enough
        let contract_balance = token_client.balance(&env.current_contract_address());
        if contract_balance < amount_due {
            panic!("Insufficient token balance in contract");
        }

        token_client.transfer(&env.current_contract_address(), &beneficiary, &amount_due);

        schedule.claimed_packets += due;
        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);

        MainClaimedEvent {
            vault_id,
            beneficiary,
            amount: amount_due,
            packets: due,
        }.publish(&env);
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

        let vested = Self::get_vested_packets(&env, &schedule);
        let total_packets = schedule.num_years * schedule.frequency;
        let unvested_packets = total_packets - vested;
        let unvested_main = schedule.packet_amount * (unvested_packets as i128);

        // Return unvested main tokens
        if unvested_main > 0 {
            let token_client = token::Client::new(&env, &schedule.token_address);
            token_client.transfer(&env.current_contract_address(), &sender, &unvested_main);
        }

        let reward_info: RewardInfo = Self::get_reward(env.clone(), vault_id);
        let remaining_reward = (reward_info.sender_share - reward_info.claimed_sender) +
                               (reward_info.receiver_share - reward_info.claimed_receiver);

        if remaining_reward > 0 {
            let mut tb = Self::get_true_balance(&env);
            tb += remaining_reward;
            Self::set_true_balance(&env, tb);
        }

        schedule.is_cancelled = true;
        env.storage().persistent().set(&DataKey::Vault(vault_id), &schedule);

        VaultCancelledEvent {
            vault_id,
            sender,
            beneficiary: schedule.beneficiary,
        }.publish(&env);
    }

    pub fn update_true_balance(env: Env, amount: i128){
        let admin = Self::get_admin(&env);
        admin.require_auth();

        if amount <= 0{
            panic!("Amount must be positive!");
        }

        let mirai_addr = Self::get_mirai_address(&env);
        let client = token::Client::new(&env, &mirai_addr);
        let apparent_bal = client.balance(&env.current_contract_address());

        let true_bal = Self::get_true_balance(&env);

        if apparent_bal < true_bal + amount {
            panic!("Not enough tokens held by contract to actualize this update");
        }

        let mut tb = true_bal;
        tb += amount;
        Self::set_true_balance(&env, tb);
    }

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
