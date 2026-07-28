#[cfg(test)]
mod test {
    use crate::{ TimeCapsule, TimeCapsuleClient };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, contractimpl, symbol_short, contract, String,
    };

    // Mock token contract for testing
#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn initialize(env: &Env, admin: Address, decimal: u32, name: String, symbol: String) {
        env.storage()
            .instance()
            .set(&symbol_short!("admin"), &admin);
        env.storage()
            .instance()
            .set(&symbol_short!("decimal"), &decimal);
        env.storage().instance().set(&symbol_short!("name"), &name);
        env.storage()
            .instance()
            .set(&symbol_short!("symbol"), &symbol);
    }

    pub fn mint(env: &Env, to: Address, amount: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&symbol_short!("admin"))
            .unwrap();
        admin.require_auth();

        let current_balance = env.storage().instance().get(&to).unwrap_or(0);
        env.storage()
            .instance()
            .set(&to, &(current_balance + amount));
    }

    pub fn balance(env: &Env, id: Address) -> i128 {
        env.storage().instance().get(&id).unwrap_or(0)
    }

    pub fn transfer(env: &Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let from_balance = env.storage().instance().get(&from).unwrap_or(0);
        if from_balance < amount {
            panic!("insufficient balance");
        }

        let to_balance = env.storage().instance().get(&to).unwrap_or(0);
        env.storage()
            .instance()
            .set(&from, &(from_balance - amount));
        env.storage().instance().set(&to, &(to_balance + amount));
    }
}


    #[test]
    fn test_full_vault_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy MiraiVault
        let vault_admin = Address::generate(&env);
        let vault_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_id);

        // Deploy MIRAI token using MockToken
        let mirai_admin = Address::generate(&env);
        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);

        mirai_client.initialize(
            &mirai_admin,
            &7u32,
            &String::from_str(&env, "Mirai Token"),
            &String::from_str(&env, "MIRAI"),
        );

        // Initialize vault with MIRAI address
        vault_client.initialize(&mirai_id, &5_000_000_i128, &2_600_000_i128, &vault_admin, &false);   // Note: pass the contract ID as the token address

        // Fund the vault with MIRAI (so calculate_reward succeeds)
        env.mock_all_auths();
        mirai_client.mint(&vault_id, &2_000_000_i128);   // Plenty for testing

        // Deploy mock vesting token (the asset users lock)
        let vesting_admin = Address::generate(&env);
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);

        let bal = mirai_client.balance(&vault_id);
        assert_eq!(bal, 2000000, "balance failed");

        vesting_client.initialize(
            &vesting_admin,
            &7u32,
            &String::from_str(&env, "Test USDC"),
            &String::from_str(&env, "USDC"),
        );

        // Create test users
        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        // Give sender tokens to lock
        env.mock_all_auths();
        vesting_client.mint(&sender, &50_000_000_000_i128);

        // Create vault: 5 years, quarterly, 70% reward to sender
        let created_vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,           // vesting token contract ID
            &10_000_000_000_i128,      // total_amount
            &5_u32,
            &4_u32,                // quarterly
            &70_u32,
        );

        // Verify
        let schedule = vault_client.get_vault(&created_vault_id);
        assert_eq!(schedule.sender, sender);
        assert_eq!(schedule.beneficiary, beneficiary);
        assert_eq!(schedule.num_years, 5);
        assert_eq!(schedule.frequency, 4);
        assert_eq!(schedule.reward_split, 70);

        // Check lists
        let by_me = vault_client.get_vaults_by_me(&sender);
        assert_eq!(by_me.len(), 1);
        assert_eq!(by_me.get(0).unwrap(), created_vault_id);

        let for_me = vault_client.get_vaults_for_me(&beneficiary);
        assert_eq!(for_me.len(), 1);
        assert_eq!(for_me.get(0).unwrap(), created_vault_id);

        // Advance time (1.25 years → 5 quarterly periods)
        let seconds_per_year: u64 = 31_536_000;
        let total_duration = seconds_per_year * schedule.num_years as u64;
        let interval = total_duration / schedule.frequency as u64;  // 39420000

        // Advance to just after the 5th packet (1.25 years + 1 second buffer)
        let target_elapsed = interval * 5 + 1;   // This ensures (elapsed / interval) >= 5
        let advanced_timestamp = schedule.start_timestamp + target_elapsed;

        env.ledger().set_timestamp(advanced_timestamp);

        // Claim main
        vault_client.claim_main(&created_vault_id, &beneficiary);

        let updated = vault_client.get_vault(&created_vault_id);
        assert!(updated.claimed_packets >= 5);

        // Claim reward as sender
        vault_client.claim_reward(&created_vault_id, &sender);

        // Test cancel on second vault
        let beneficiary2 = Address::generate(&env);
        let vault_id2 = vault_client.create_fund(
            &sender,
            &beneficiary2,
            &vesting_id,
            &5_000_000_000_i128,
            &4_u32,
            &2_u32,
            &50_u32,
        );

        vault_client.cancel_remaining(&vault_id2, &sender);

        let cancelled = vault_client.get_vault(&vault_id2);
        assert!(cancelled.is_cancelled);
    }

    #[test]
    #[should_panic(expected = "No main packets due yet")]
    fn test_nothing_to_claim_immediately_after_create() {
        let env = Env::default();
        env.mock_all_auths();

        let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &5_000_000_i128, &2_500_000_i128, &vault_admin, &false);
        mirai_client.mint(&vault_contract_id, &2_500_000_i128);

        let vesting_contract_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_contract_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "USDC"), &String::from_str(&env, "USDC"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        vesting_client.mint(&sender, &20_000_000_000_i128);

        let vault_id = vault_client.create_fund(&sender, &beneficiary, &vesting_contract_id, &10_000_000_000_i128, &5u32, &4u32, &60u32);

        // Try to claim immediately → should panic
        vault_client.claim_main(&vault_id, &beneficiary);
    }

    #[test]
    #[should_panic(expected = "Unauthorized claimant")]
    fn test_unauthorized_claim_reward() {
        let env = Env::default();
        env.mock_all_auths();

        let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &5_000_000_i128, &2_500_000_i128, &vault_admin, &false);
        mirai_client.mint(&vault_contract_id, &2_500_000_i128);

        let vesting_contract_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_contract_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "USDC"), &String::from_str(&env, "USDC"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let random_user = Address::generate(&env);

        vesting_client.mint(&sender, &20_000_000_000_i128);

        let vault_id = vault_client.create_fund(&sender, &beneficiary, &vesting_contract_id, &5_000_000_000_i128, &4u32, &2u32, &50u32);

        // Random user tries to claim reward → should panic
        vault_client.claim_reward(&vault_id, &random_user);
    }

    #[test]
    #[should_panic(expected = "Only sender can cancel")]
    fn test_beneficiary_cannot_cancel() {
        let env = Env::default();
        env.mock_all_auths();

        let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &5_000_000_i128, &2_500_000_i128, &vault_admin, &false);
        mirai_client.mint(&vault_contract_id, &2_500_000_i128);

        let vesting_contract_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_contract_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "USDC"), &String::from_str(&env, "USDC"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        vesting_client.mint(&sender, &20_000_000_000_i128);

        let vault_id = vault_client.create_fund(&sender, &beneficiary, &vesting_contract_id, &10_000_000_000_i128, &5u32, &1u32, &30u32);

        // Beneficiary tries to cancel → should panic
        vault_client.cancel_remaining(&vault_id, &beneficiary);
    }

    #[test]
    fn test_two_identical_vaults_reward_comparison() {
        let env = Env::default();
        env.mock_all_auths();

        let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &vault_admin, &false);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128); // plenty for both

        let vesting_contract_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_contract_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "USDC"), &String::from_str(&env, "USDC"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        vesting_client.mint(&sender, &200_000_000_000_000_i128);

        let params = (sender.clone(), beneficiary.clone(), vesting_contract_id.clone(), 300_000_000_000_i128, 5u32, 4u32, 50u32);

        let vault_id1 = vault_client.create_fund(&params.0, &params.1, &params.2, &params.3, &params.4, &params.5, &params.6);
        let reward1 = vault_client.get_reward(&vault_id1).total_reward;

        let vault_id2 = vault_client.create_fund(&params.0, &params.1, &params.2, &params.3, &params.4, &params.5, &params.6);
        let reward2 = vault_client.get_reward(&vault_id2).total_reward;

        assert!(reward1 > reward2, "Rewards must follow diminishing returns.");
    }

    #[test]
    #[should_panic]
    fn test_non_admin_cannot_update_true_balance() {
        let env = Env::default();
        env.mock_all_auths();

        let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);


        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000i128, &5_000_000i128, &vault_admin, &false);
        env.mock_auths(&[]);
        // Random user tries to update → should fail
        vault_client.update_true_balance(&1_000_000i128);  // note: the client call will use the address as auth
    }

    #[test]
    #[should_panic(expected = "Not enough tokens held by contract to actualize this update")]
    fn test_admin_cannot_over_increase_true_balance() {
        let env = Env::default();
        env.mock_all_auths();

        let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        let admin = Address::generate(&env);

        mirai_client.initialize(&admin, &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000i128, &5_000_000i128, &vault_admin, &false);

        // Only 10M MIRAI minted, trying to add 100M to TrueBalance should fail
        vault_client.update_true_balance(&100_000_000i128);
    }

    #[test]
    fn test_cancel_after_partial_claim() {
        let env = Env::default();
        env.mock_all_auths();

        let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);

        mirai_client.initialize(
            &Address::generate(&env),
            &7,
            &String::from_str(&env, "Mirai"),
            &String::from_str(&env, "MIRAI")
        );

        vault_client.initialize(&mirai_contract_id, &10_000_000i128, &5_000_000i128, &vault_admin, &false);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        let vesting_contract_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_contract_id);
        vesting_client.initialize(
            &Address::generate(&env),
            &7,
            &String::from_str(&env, "USDC"),
            &String::from_str(&env, "USDC")
        );

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        vesting_client.mint(&sender, &50_000_000_000_i128);

        // Record sender's vesting token balance before
        let sender_balance_before = vesting_client.balance(&sender);

        // Create vault: 4 years, quarterly (minimum allowed)
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_contract_id,
            &10_000_000_000_i128,
            &4u32,   // num_years = 4 (minimum)
            &4u32,   // frequency = quarterly
            &50u32,
        );

        let schedule = vault_client.get_vault(&vault_id);

        // === Advance time for exactly 1 packet ===
        let seconds_per_year: u64 = 31_536_000;
        let total_duration = seconds_per_year * schedule.num_years as u64;   // 4 years
        let interval = total_duration / schedule.frequency as u64;           // 1 quarter = 31_536_000 / 4 = 7_884_000 seconds
        let target_elapsed = interval * 1 + 1;                               // Just past the first packet
        let advanced_timestamp = schedule.start_timestamp + target_elapsed;

        env.ledger().set_timestamp(advanced_timestamp);

        // Beneficiary claims 1 main packet
        vault_client.claim_main(&vault_id, &beneficiary);

        // Both claim their MIRAI reward for the 1 vested packet
        vault_client.claim_reward(&vault_id, &sender);
        vault_client.claim_reward(&vault_id, &beneficiary);

        // Cancel the remaining vault
        vault_client.cancel_remaining(&vault_id, &sender);

        // Check sender's final balance
        let sender_balance_after = vesting_client.balance(&sender);

        // Sender should lose only 1 packet (the one claimed by beneficiary)
        let expected_sender_final = sender_balance_before
            - schedule.total_amount
            + (schedule.total_amount - schedule.packet_amount);

        assert_eq!(
            sender_balance_after,
            expected_sender_final,
            "Sender should only lose the amount claimed by beneficiary (1 packet)"
        );
    }
}
