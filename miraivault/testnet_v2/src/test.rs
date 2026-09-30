#[cfg(test)]
mod test {
    use crate::{ TimeCapsule, TimeCapsuleClient };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, contractimpl, contracttype, symbol_short, contract, String, token,
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

    #[contract]
    pub struct MockProtocol;

    #[contracttype]
    #[derive(Clone)]
    enum MockDataKey {
        Supplied(u64), // vault id → amount
        BaseToken, // address of the mock XLM token
    }

    #[contractimpl]
    impl MockProtocol {
        pub fn initialize(env: Env, base_token: Address) {
            env.storage().persistent().set(&MockDataKey::BaseToken, &base_token);
        }

        pub fn supply(env: Env, vault_id: u64, caller: Address, amount: i128) {
            if amount <= 0 {
                panic!("Invalid amount");
            }
            //caller.require_auth();

            let base_token: Address = env.storage().persistent().get(&MockDataKey::BaseToken).unwrap();

            let token_client = token::Client::new(&env, &base_token);
            token_client.transfer(&caller, &env.current_contract_address(), &amount);

            let key = MockDataKey::Supplied(vault_id);
            let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
            env.storage().persistent().set(&key, &(current + amount));
        }

        pub fn withdraw(env: Env, vault_id: u64, amount: i128, to: Address) {
            if amount <= 0 {
                panic!("Invalid amount");
            }

            let key = MockDataKey::Supplied(vault_id);
            let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
            if amount > current { panic!("Insufficient balance in mock"); }

            env.storage().persistent().set(&key, &(current - amount));

            let base_token: Address = env.storage().persistent().get(&MockDataKey::BaseToken).unwrap();
            let token_client = token::Client::new(&env, &base_token);
            token_client.transfer(&env.current_contract_address(), &to, &amount);
        }

        pub fn get_supplied(env: Env, vault_id: u64) -> i128 {
            let key = MockDataKey::Supplied(vault_id);
            env.storage().persistent().get(&key).unwrap_or(0)
        }
    }

    #[contract]
    pub struct MockDefindex;

    #[contracttype]
    #[derive(Clone)]
    enum MockDefindexDataKey {
        Supplied(u64),   // principal tracked
        BaseToken,
    }

    #[contractimpl]
    impl MockDefindex {
        pub fn initialize(env: Env, base_token: Address) {
            env.storage().persistent().set(&MockDefindexDataKey::BaseToken, &base_token);
        }

        pub fn supply(env: Env, vault_id: u64, caller: Address, amount: i128) {
            if amount <= 0 {
                panic!("Invalid amount");
            }

            let base_token: Address = env.storage().persistent()
                .get(&MockDefindexDataKey::BaseToken).unwrap();

            let token_client = token::Client::new(&env, &base_token);
            token_client.transfer(&caller, &env.current_contract_address(), &amount);

            let key = MockDefindexDataKey::Supplied(vault_id);
            let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
            env.storage().persistent().set(&key, &(current + amount));
        }

        pub fn withdraw(env: Env, vault_id: u64, amount: i128, to: Address) {
            if amount <= 0 {
                panic!("Invalid amount");
            }

            let key = MockDefindexDataKey::Supplied(vault_id);
            let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
            if amount > current {
                panic!("Insufficient balance in mock");
            }

            // Reduce principal
            env.storage().persistent().set(&key, &(current - amount));

            // Simulate 10% yield
            let yield_amount = amount / 10;           // 10% profit
            let total_to_send = amount + yield_amount;

            let base_token: Address = env.storage().persistent()
                .get(&MockDefindexDataKey::BaseToken).unwrap();
            let token_client = token::Client::new(&env, &base_token);

            // The mock needs enough tokens to pay the yield.
            // Easiest way in tests: mint the yield to the mock first,
            // or pre-fund the mock contract with extra tokens.
            token_client.transfer(&env.current_contract_address(), &to, &total_to_send);
        }

        pub fn get_supplied(env: Env, vault_id: u64) -> i128 {
            let key = MockDefindexDataKey::Supplied(vault_id);
            env.storage().persistent().get(&key).unwrap_or(0)
        }
    }


    #[test]
    fn test_full_vault_lifecycle() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));




        // Mock Treasury
        let treasury = Address::generate(&env);

        //let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock MIRAI token
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));


        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);


        // Mock vesting token (XLM-like)
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        vesting_client.mint(&sender, &100_000_000_000_i128);

        // Deploy mocks
        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockProtocol, ());

        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        let p2_client = MockDefindexClient::new(&env, &p2_addr);

        p1_client.initialize(&vesting_id);
        p2_client.initialize(&vesting_id);



        // Create vault
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,
            &10_000_000_000_i128,
            &5u32,
            &4u32,
            &50u32,
            &Some(p1_addr.clone()),
            &40u32,
            &Some(p2_addr.clone()),
            &30u32,
        );

        // Supply
        vault_client.supply_to_p1(&vault_id, &sender, &4_000_000_000_i128);

        // Advance time
        let init_schedule = vault_client.get_vault(&vault_id);
        let seconds_per_year: u64 = 31_536_000;
        let total_duration = seconds_per_year * init_schedule.num_years as u64;
        let interval = total_duration / init_schedule.frequency as u64;
        env.ledger().set_timestamp(init_schedule.start_timestamp + interval * 5 + 1);

        // Claim
        vault_client.claim_main(&vault_id, &beneficiary);

        let this_schedule = vault_client.get_vault(&vault_id);
        assert_eq!(this_schedule.supplied_p1, 4_000_000_000_i128);
        assert_eq!(this_schedule.supplied_p2, 0);
        assert_eq!(this_schedule.claimed_packets, 5);


        // Final claim
        env.ledger().set_timestamp(100_000_000_000_000);
        vault_client.claim_main(&vault_id, &beneficiary);

        let final_schedule = vault_client.get_vault(&vault_id);
        assert_eq!(final_schedule.claimed_packets, 20);
        assert_eq!(final_schedule.supplied_p1, 0);
        assert_eq!(final_schedule.supplied_p2, 0);
    }

    #[test]
    #[should_panic(expected = "No main packets due yet")]
    fn test_nothing_to_claim_immediately() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        //let vault_admin = Address::generate(&env);
        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));


        // Mock Treasury
        let treasury = Address::generate(&env);

        // Mock MIRAI token
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);


        // Mock vesting token (XLM-like)
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        vesting_client.mint(&sender, &100_000_000_000_i128);

        // Deploy mocks
        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockDefindex, ());

        // Create vault
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,
            &10_000_000_000_i128,
            &5u32,
            &4u32,
            &50u32,
            &Some(p1_addr.clone()),
            &40u32,
            &Some(p2_addr.clone()),
            &30u32,
        );


        vault_client.claim_main(&vault_id, &beneficiary);
    }


    #[test]
    fn test_cancel_immediately() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));


        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        // Create vault
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,
            &total_amount,
            &5u32,
            &4u32,
            &50u32,
            &None,
            &0u32,
            &None,
            &0u32,
        );

        // Immediately cancel
        vault_client.cancel_remaining(&vault_id, &sender);

        let final_balance = vesting_client.balance(&sender);
        assert_eq!(final_balance, initial_balance, "Sender should get full amount back on immediate cancel");

        let final_schedule = vault_client.get_vault(&vault_id);
        assert!(final_schedule.is_cancelled);
        assert_eq!(final_schedule.supplied_p1, 0);
        assert_eq!(final_schedule.supplied_p2, 0);
    }

    #[test]
    fn test_cancel_after_supplying_to_protocols() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));



        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        // Deploy mocks
        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockProtocol, ());

        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p1_client.initialize(&vesting_id);
        p2_client.initialize(&vesting_id);

        // Create vault
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,
            &total_amount,
            &5u32,
            &4u32,
            &50u32,
            &Some(p1_addr.clone()),
            &40u32,
            &Some(p2_addr.clone()),
            &30u32,
        );

        // Supply 30% total (e.g. 15% each or whatever fits under ratios)
        let supply_amount = 1_500_000_000_i128; // 15% of 10B
        vault_client.supply_to_p1(&vault_id, &sender, &supply_amount);
        vault_client.supply_to_p2(&vault_id, &sender, &supply_amount);

        // Cancel
        vault_client.cancel_remaining(&vault_id, &sender);

        let final_balance = vesting_client.balance(&sender);
        assert_eq!(final_balance, initial_balance, "Sender should get full amount back after cancel");

        let final_schedule = vault_client.get_vault(&vault_id);
        assert!(final_schedule.is_cancelled);
        assert_eq!(final_schedule.supplied_p1, 0);
        assert_eq!(final_schedule.supplied_p2, 0);
        assert_eq!(final_schedule.idle_amount, 0);
    }


    #[test]
    fn test_cancel_after_partial_claim_and_supply() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));


        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        // Deploy mocks
        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockProtocol, ());

        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p1_client.initialize(&vesting_id);
        p2_client.initialize(&vesting_id);

        // Create vault
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,
            &total_amount,
            &5u32,
            &4u32,
            &50u32,
            &Some(p1_addr.clone()),
            &40u32,
            &Some(p2_addr.clone()),
            &30u32,
        );

        // Supply 30% total (e.g. 15% each or whatever fits under ratios)
        let supply_amount = 1_500_000_000_i128; // 15% of 10B
        vault_client.supply_to_p1(&vault_id, &sender, &supply_amount);
        vault_client.supply_to_p2(&vault_id, &sender, &supply_amount);

        // Advance time
        let init_schedule = vault_client.get_vault(&vault_id);
        let seconds_per_year: u64 = 31_536_000;
        let total_duration = seconds_per_year * init_schedule.num_years as u64;
        let interval = total_duration / init_schedule.frequency as u64;
        env.ledger().set_timestamp(init_schedule.start_timestamp + interval * 5 + 1);

        // Claim
        vault_client.claim_main(&vault_id, &beneficiary);

        let this_schedule = vault_client.get_vault(&vault_id);
        assert_eq!(this_schedule.supplied_p1, 1_500_000_000_i128);
        assert_eq!(this_schedule.supplied_p2, 1_500_000_000_i128);
        assert_eq!(this_schedule.claimed_packets, 5);

        let packets_claimed = this_schedule.claimed_packets;
        let packet_amount = this_schedule.packet_amount;


        // Cancel
        vault_client.cancel_remaining(&vault_id, &sender);

        let final_balance_sender = vesting_client.balance(&sender);
        let final_balance_beneficiary = packet_amount * packets_claimed as i128;
        assert_eq!(final_balance_sender + final_balance_beneficiary, total_amount, "Sender and Beneficiary total should remain unchanged.");

        let final_schedule = vault_client.get_vault(&vault_id);
        assert!(final_schedule.is_cancelled);
        assert_eq!(final_schedule.supplied_p1, 0);
        assert_eq!(final_schedule.supplied_p2, 0);
        assert_eq!(final_schedule.idle_amount, 0);
    }


    #[test]
    fn test_claim_reward_sender_and_beneficiary() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Reward tokens (P1/P2 not used in this test but required by initialize)
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "BLND"), &String::from_str(&env, "BLND"));

        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        // Create vault (no protocols needed)
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,
            &total,
            &5u32,   // 5 years
            &4u32,   // quarterly → 20 packets
            &50u32,  // 50/50 split
            &None,
            &0u32,
            &None,
            &0u32,
        );

        let reward_info = vault_client.get_reward(&vault_id);
        let total_packets = 20i128;
        let sender_per_packet = reward_info.sender_share / total_packets;
        let receiver_per_packet = reward_info.receiver_share / total_packets;

        // Advance time so 5 packets are vested
        let schedule = vault_client.get_vault(&vault_id);
        let seconds_per_year: u64 = 31_536_000;
        let interval = (seconds_per_year * schedule.num_years as u64) / schedule.frequency as u64;
        env.ledger().set_timestamp(schedule.start_timestamp + interval * 5 + 1);

        // --- Sender claims ---
        vault_client.claim_reward(&vault_id, &sender);
        let expected_sender = sender_per_packet * 5;
        assert_eq!(mirai_client.balance(&sender), expected_sender);

        let updated_reward = vault_client.get_reward(&vault_id);
        assert_eq!(updated_reward.claimed_sender, expected_sender);

        // --- Beneficiary claims ---
        vault_client.claim_reward(&vault_id, &beneficiary);
        let expected_receiver = receiver_per_packet * 5;
        assert_eq!(mirai_client.balance(&beneficiary), expected_receiver);

        let final_reward = vault_client.get_reward(&vault_id);
        assert_eq!(final_reward.claimed_receiver, expected_receiver);
    }

    #[test]
    #[should_panic(expected = "No sender reward due yet")]
    fn test_claim_reward_too_early_sender() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        vesting_client.mint(&sender, &10_000_000_000);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &10_000_000_000,
            &5, &4, &50,
            &None, &0, &None, &0,
        );

        // Immediately try to claim → should panic
        vault_client.claim_reward(&vault_id, &sender);
    }

    #[test]
    #[should_panic(expected = "No receiver reward due yet")]
    fn test_claim_reward_too_early_beneficiary() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        vesting_client.mint(&sender, &10_000_000_000);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &10_000_000_000,
            &5, &4, &50,
            &None, &0, &None, &0,
        );

        vault_client.claim_reward(&vault_id, &beneficiary);
    }

    #[test]
    #[should_panic(expected = "Unauthorized claimant")]
    fn test_claim_reward_unauthorized() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let random = Address::generate(&env);
        vesting_client.mint(&sender, &10_000_000_000);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &10_000_000_000,
            &5, &4, &50,
            &None, &0, &None, &0,
        );

        // Advance time so rewards exist
        let schedule = vault_client.get_vault(&vault_id);
        let interval = (31_536_000u64 * schedule.num_years as u64) / schedule.frequency as u64;
        env.ledger().set_timestamp(schedule.start_timestamp + interval * 5 + 1);

        vault_client.claim_reward(&vault_id, &random); // should panic
    }

    #[test]
    #[should_panic(expected = "Vault is cancelled")]
    fn test_claim_reward_after_cancel() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        vesting_client.mint(&sender, &10_000_000_000);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &10_000_000_000,
            &5, &4, &50,
            &None, &0, &None, &0,
        );

        // Cancel immediately
        vault_client.cancel_remaining(&vault_id, &sender);

        // Trying to claim after cancel should panic
        vault_client.claim_reward(&vault_id, &sender);
    }


    #[test]
    fn test_claim_reward_final_remainder() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,          // 50/50 split, 20 packets
            &None, &0, &None, &0,
        );

        let reward_info = vault_client.get_reward(&vault_id);

        // Jump all the way to the end
        env.ledger().set_timestamp(100_000_000_000_000);

        // Sender takes everything due (including any remainder)
        vault_client.claim_reward(&vault_id, &sender);
        assert_eq!(mirai_client.balance(&sender), reward_info.sender_share);

        // Beneficiary takes everything due
        vault_client.claim_reward(&vault_id, &beneficiary);
        assert_eq!(mirai_client.balance(&beneficiary), reward_info.receiver_share);

        // Nothing left
        let final_reward = vault_client.get_reward(&vault_id);
        assert_eq!(final_reward.claimed_sender, reward_info.sender_share);
        assert_eq!(final_reward.claimed_receiver, reward_info.receiver_share);
    }



    #[test]
    #[should_panic(expected = "Cannot allocate more than 50% to one protocol")]
    fn test_supply_exceeds_50_percent_limit() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));



        // Mock Treasury
        let treasury = Address::generate(&env);

        // Minimal setup (MIRAI + vesting token)
        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p1_addr = env.register(MockProtocol, ());
        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        p1_client.initialize(&vesting_id);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5u32, &4u32, &50u32,
            &Some(p1_addr), &40u32, &None, &0u32,
        );

        // Try to supply 51% → should panic
        vault_client.supply_to_p1(&vault_id, &sender, &(total * 51 / 100));
    }

    #[test]
    #[should_panic(expected = "Not more than 70% of the funds might be used up in third party protocols.")]
    fn test_total_protocol_allocation_exceeds_70_percent() {
        let env = Env::default();

        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));



        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockDefindex, ());
        // initialize both mocks...

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total_amount,
            &5u32, &4u32, &50u32,
            &Some(p1_addr.clone()), &40u32,
            &Some(p2_addr.clone()), &40u32,
        );

        // Supply 40% to P1 (ok)
        vault_client.supply_to_p1(&vault_id, &sender, &(total_amount * 40 / 100));

        // Try another 40% to P2 → 80% total → should panic
        vault_client.supply_to_p2(&vault_id, &sender, &(total_amount * 40 / 100));
    }

    #[test]
    #[should_panic(expected = "frequency must be 1, 2, 4 or 365")]
    fn test_improper_frequency() {
        let env = Env::default();

        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));


        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockDefindex, ());
        // initialize both mocks...

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total_amount,
            &5u32, &3u32, &50u32,
            &Some(p1_addr.clone()), &40u32,
            &Some(p2_addr.clone()), &40u32,
        );

    }


    #[test]
    #[should_panic(expected = "num_years must be 4-30")]
    fn test_too_long_period() {
        let env = Env::default();

        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));


        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockDefindex, ());
        // initialize both mocks...

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total_amount,
            &50u32, &1u32, &50u32,
            &Some(p1_addr.clone()), &40u32,
            &Some(p2_addr.clone()), &40u32,
        );

    }


    #[test]
    #[should_panic(expected = "reward_split 0-100")]
    fn test_invalid_split() {
        let env = Env::default();

        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));



        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockDefindex, ());
        // initialize both mocks...

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total_amount,
            &10u32, &1u32, &150u32,
            &Some(p1_addr.clone()), &40u32,
            &Some(p2_addr.clone()), &40u32,
        );

    }


    #[test]
    fn test_accounting_invariant_after_supply() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));



        // Mock Treasury
        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_contract_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        let initial_balance = vesting_client.balance(&sender);

        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockProtocol, ());

        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p1_client.initialize(&vesting_id);
        p2_client.initialize(&vesting_id);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total_amount,
            &5u32, &4u32, &50u32,
            &Some(p1_addr.clone()), &40u32,
            &Some(p2_addr.clone()), &30u32,
        );

        let supply_p1 = total_amount * 20 / 100;
        let supply_p2 = total_amount * 20 / 100;

        vault_client.supply_to_p1(&vault_id, &sender, &supply_p1);
        vault_client.supply_to_p2(&vault_id, &sender, &supply_p2);

        let schedule = vault_client.get_vault(&vault_id);

        assert_eq!(schedule.supplied_p1, supply_p1);
        assert_eq!(schedule.supplied_p2, supply_p2);
        assert_eq!(
            schedule.supplied_p1 + schedule.supplied_p2 + schedule.idle_amount,
            total_amount,
            "Accounting invariant broken"
        );
    }


    #[test]
    #[should_panic(expected = "Only sender can supply")]
    fn test_supply_is_sender_only() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock P1 and P2 token
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Blend"), &String::from_str(&env, "BLND"));


        // Mock Treasury
        let treasury = Address::generate(&env);

        // Minimal setup (MIRAI + vesting token)
        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000_i128, &5_000_000_i128, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p1_addr = env.register(MockProtocol, ());
        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        p1_client.initialize(&vesting_id);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5u32, &4u32, &50u32,
            &Some(p1_addr), &40u32, &None, &0u32,
        );

        // Try to supply 51% → should panic
        vault_client.supply_to_p1(&vault_id, &beneficiary, &(total * 51 / 100));
    }

    #[test]
    fn test_claim_p1_rewards() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Mock reward tokens
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(
            &Address::generate(&env),
            &7,
            &String::from_str(&env, "Blend"),
            &String::from_str(&env, "BLND"),
        );



        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(
            &Address::generate(&env),
            &7,
            &String::from_str(&env, "Mirai"),
            &String::from_str(&env, "MIRAI"),
        );
        vault_client.initialize(
            &mirai_id,
            &10_000_000_i128,
            &5_000_000_i128,
            &false,
            &p1_token_id,
            &treasury,
        );
        mirai_client.mint(&vault_contract_id, &5_000_000_i128);

        // Vesting token (XLM)
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(
            &Address::generate(&env),
            &7u32,
            &String::from_str(&env, "Test XLM"),
            &String::from_str(&env, "XLM"),
        );

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total_amount = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total_amount);

        // Mock protocols
        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockDefindex, ());
        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p1_client.initialize(&vesting_id);
        p2_client.initialize(&vesting_id);

        // Create vault and supply to P1
        let vault_id = vault_client.create_fund(
            &sender,
            &beneficiary,
            &vesting_id,
            &total_amount,
            &5u32,
            &4u32,
            &50u32,
            &Some(p1_addr.clone()),
            &40u32,
            &Some(p2_addr.clone()),
            &30u32,
        );

        let supply_amount = 2_000_000_000_i128; // 20%
        vault_client.supply_to_p1(&vault_id, &sender, &supply_amount);

        let final_schedule = vault_client.get_vault(&vault_id);
        let acc1 = vault_client.get_acc_reward_per_share_p1();
        //panic!("Acc = {}", acc1);


        // === Simulate Blend sending rewards to the vault ===
        let reward_amount = 1_000_000_i128; // 1 BLND (adjust decimals if needed)
        p1_token_client.mint(&vault_contract_id, &reward_amount);

        // Claim
        vault_client.claim_p1_rewards(&vault_id, &sender);

        // Assertions
        let expected_fee = (reward_amount * 5) / 100;
        let expected_user = reward_amount - expected_fee;

        assert_eq!(p1_token_client.balance(&sender), expected_user, "User Reward did not match.");
        assert_eq!(p1_token_client.balance(&treasury), expected_fee, "Protocol Fee did not match.");
        assert_eq!(p1_token_client.balance(&vault_contract_id), 0, "Contract failed to be yield neutral");
    }


    #[test]
    fn test_reward_fairness_two_vaults() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        // Reward tokens
        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "BLND"), &String::from_str(&env, "BLND"));


        let treasury = Address::generate(&env);

        // MIRAI
        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        // Vesting token
        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender1 = Address::generate(&env);
        let sender2 = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let total1 = 8_000_000_000_i128; // whale
        let total2 = 2_000_000_000_i128; // small

        vesting_client.mint(&sender1, &total1);
        vesting_client.mint(&sender2, &total2);

        let p1_addr = env.register(MockProtocol, ());
        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        p1_client.initialize(&vesting_id);

        // Create two vaults
        let vault1 = vault_client.create_fund(
            &sender1, &beneficiary, &vesting_id, &total1,
            &5, &4, &50,
            &Some(p1_addr.clone()), &40, &None, &0,
        );
        let vault2 = vault_client.create_fund(
            &sender2, &beneficiary, &vesting_id, &total2,
            &5, &4, &50,
            &Some(p1_addr.clone()), &40, &None, &0,
        );

        // Both supply 40% of their amount
        let supply1 = total1 * 40 / 100; // 4B
        let supply2 = total2 * 40 / 100; // 1B
        vault_client.supply_to_p1(&vault1, &sender1, &supply1);
        vault_client.supply_to_p1(&vault2, &sender2, &supply2);

        // Simulate rewards arriving
        let total_rewards = 1_000_000_i128;
        p1_token_client.mint(&vault_contract_id, &total_rewards);

        // Vault 1 claims first
        vault_client.claim_p1_rewards(&vault1, &sender1);

        let expected1 = (total_rewards * 4 / 5) * 95 / 100; // 80% of rewards * 95%
        let fee1 = (total_rewards * 4 / 5) * 5 / 100;

        assert_eq!(p1_token_client.balance(&sender1), expected1);
        assert_eq!(p1_token_client.balance(&treasury), fee1);

        // Vault 2 claims later
        vault_client.claim_p1_rewards(&vault2, &sender2);

        let expected2 = (total_rewards * 1 / 5) * 95 / 100; // 20% of rewards * 95%
        let fee2 = (total_rewards * 1 / 5) * 5 / 100;

        assert_eq!(p1_token_client.balance(&sender2), expected2);
        assert_eq!(p1_token_client.balance(&treasury), fee1 + fee2);
        assert_eq!(p1_token_client.balance(&vault_contract_id), 0);
    }


    #[test]
    #[should_panic(expected = "No rewards to claim")]
    fn test_claim_p1_rewards_when_none() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "BLND"), &String::from_str(&env, "BLND"));

        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        vesting_client.mint(&sender, &10_000_000_000);

        let p1_addr = env.register(MockProtocol, ());
        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        p1_client.initialize(&vesting_id);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &10_000_000_000,
            &5, &4, &50,
            &Some(p1_addr), &40, &None, &0,
        );

        vault_client.supply_to_p1(&vault_id, &sender, &2_000_000_000);

        // No rewards minted → should panic
        vault_client.claim_p1_rewards(&vault_id, &sender);
    }


    #[test]
    fn test_claim_after_full_withdraw() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let p1_token_client = MockTokenClient::new(&env, &p1_token_id);
        p1_token_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "BLND"), &String::from_str(&env, "BLND"));

        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p1_addr = env.register(MockProtocol, ());
        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        p1_client.initialize(&vesting_id);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &Some(p1_addr), &40, &None, &0,
        );

        let supply_amount = 2_000_000_000_i128;
        vault_client.supply_to_p1(&vault_id, &sender, &supply_amount);

        // Rewards arrive while still supplied
        let reward_amount = 1_000_000_i128;
        p1_token_client.mint(&vault_contract_id, &reward_amount);

        // Full withdraw from protocol
        vault_client.withdraw_from_p1(&vault_id, &sender, &supply_amount);

        // The rewards should already be paid.

        let expected_user = reward_amount * 95 / 100;
        let expected_fee = reward_amount * 5 / 100;

        assert_eq!(p1_token_client.balance(&sender), expected_user);
        assert_eq!(p1_token_client.balance(&treasury), expected_fee);
    }

    #[test]
    fn test_p2_supply_tracks_shares_and_principal() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p2_addr = env.register(MockDefindex, ());
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p2_client.initialize(&vesting_id);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &None, &0,
            &Some(p2_addr), &30,
        );

        let amount = 2_000_000_000_i128;
        vault_client.supply_to_p2(&vault_id, &sender, &amount);

        let s = vault_client.get_vault(&vault_id);
        assert_eq!(s.supplied_p2, amount);
        assert_eq!(s.p2_shares, amount);       // 1:1 in current mock
        assert_eq!(s.p2_principal, amount);
        assert_eq!(s.idle_amount, total - amount);
    }

    #[test]
    fn test_p2_partial_and_full_withdraw() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p2_addr = env.register(MockDefindex, ());
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p2_client.initialize(&vesting_id);
        vesting_client.mint(&p2_addr, &5_000_000_000_i128);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &None, &0,
            &Some(p2_addr), &40,
        );

        let amount = 3_000_000_000_i128;
        vault_client.supply_to_p2(&vault_id, &sender, &amount);

        // Partial withdraw
        let partial = 1_000_000_000_i128;
        vault_client.withdraw_from_p2(&vault_id, &sender, &partial);

        let s = vault_client.get_vault(&vault_id);
        assert_eq!(s.supplied_p2, amount - partial);
        assert_eq!(s.p2_shares, amount - partial);
        assert_eq!(s.p2_principal, amount - partial);
        assert_eq!(s.idle_amount, total - amount + partial);

        // Full remaining withdraw
        vault_client.withdraw_from_p2(&vault_id, &sender, &(amount - partial));

        let s2 = vault_client.get_vault(&vault_id);
        assert_eq!(s2.supplied_p2, 0);
        assert_eq!(s2.p2_shares, 0);
        assert_eq!(s2.p2_principal, 0);
        assert_eq!(s2.idle_amount, total);
    }

    #[test]
    fn test_final_claim_pulls_from_p2() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p2_addr = env.register(MockDefindex, ());
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p2_client.initialize(&vesting_id);
        vesting_client.mint(&p2_addr, &5_000_000_000_i128);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &None, &0,
            &Some(p2_addr), &40,
        );

        // Put 40% into P2
        let supply = total * 40 / 100;
        vault_client.supply_to_p2(&vault_id, &sender, &supply);

        // Jump to the very end
        env.ledger().set_timestamp(100_000_000_000_000);

        vault_client.claim_main(&vault_id, &beneficiary);

        let s = vault_client.get_vault(&vault_id);
        assert_eq!(s.claimed_packets, 20);
        assert_eq!(s.supplied_p2, 0);
        assert_eq!(s.p2_shares, 0);
        assert_eq!(s.p2_principal, 0);
        assert_eq!(s.idle_amount, 0);

        let expected_yield = supply / 10;
        let expected_user_yield = ( expected_yield * 95 ) / 100;
        let expected_final = expected_user_yield;

        let beneficiary_balance = vesting_client.balance(&sender);
        assert_eq!(beneficiary_balance, expected_final, "Sender should receive 95% of the yield rewards.");
    }


    #[test]
    fn test_cancel_after_p2_yield() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p2_addr = env.register(MockDefindex, ());
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p2_client.initialize(&vesting_id);

        // Give mock enough tokens to pay 10% yield
        vesting_client.mint(&p2_addr, &2_000_000_000);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &None, &0,
            &Some(p2_addr), &40,
        );

        let supply = 4_000_000_000_i128; // 40%
        vault_client.supply_to_p2(&vault_id, &sender, &supply);

        let sender_before = vesting_client.balance(&sender);

        // Cancel
        vault_client.cancel_remaining(&vault_id, &sender);

        let expected_yield = supply / 10;                 // 400_000_000
        let expected_user_yield = expected_yield * 95 / 100;
        let expected_fee = expected_yield - expected_user_yield;

        let sender_after = vesting_client.balance(&sender);
        let treasury_bal = vesting_client.balance(&treasury);

        // Sender should get back full principal + 95% of yield
        assert_eq!(sender_after, sender_before + total + expected_user_yield);
        assert_eq!(treasury_bal, expected_fee);

        let s = vault_client.get_vault(&vault_id);
        assert!(s.is_cancelled);
        assert_eq!(s.supplied_p2, 0);
        assert_eq!(s.p2_shares, 0);
        assert_eq!(s.p2_principal, 0);
        assert_eq!(s.idle_amount, 0);
    }

    // ============================================================
    // 2. Final claim with both P1 + P2
    // ============================================================
    #[test]
    fn test_final_claim_both_protocols() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p1_addr = env.register(MockProtocol, ());
        let p2_addr = env.register(MockDefindex, ());
        let p1_client = MockProtocolClient::new(&env, &p1_addr);
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p1_client.initialize(&vesting_id);
        p2_client.initialize(&vesting_id);

        // Fund MockDefindex for yield
        vesting_client.mint(&p2_addr, &1_000_000_000);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &Some(p1_addr), &30,
            &Some(p2_addr), &30,
        );

        let supply_p1 = 2_000_000_000_i128;
        let supply_p2 = 2_000_000_000_i128;
        vault_client.supply_to_p1(&vault_id, &sender, &supply_p1);
        vault_client.supply_to_p2(&vault_id, &sender, &supply_p2);

        // Jump to the end
        env.ledger().set_timestamp(100_000_000_000_000);
        vault_client.claim_main(&vault_id, &beneficiary);

        let s = vault_client.get_vault(&vault_id);
        assert_eq!(s.claimed_packets, 20);
        assert_eq!(s.supplied_p1, 0);
        assert_eq!(s.supplied_p2, 0);
        assert_eq!(s.p2_shares, 0);
        assert_eq!(s.p2_principal, 0);
        assert_eq!(s.idle_amount, 0);

        // Beneficiary got the full principal
        assert_eq!(vesting_client.balance(&beneficiary), total);

        // Sender got the P2 yield (95%)
        let expected_yield = supply_p2 / 10;
        let expected_user_yield = expected_yield * 95 / 100;
        assert_eq!(vesting_client.balance(&sender), expected_user_yield);
    }

    // ============================================================
    // 3. Non-final claim that forces a pull from P2 (deficit path)
    // ============================================================
    #[test]
    fn test_deficit_pull_from_p2() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p2_addr = env.register(MockDefindex, ());
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p2_client.initialize(&vesting_id);
        vesting_client.mint(&p2_addr, &2_000_000_000); // enough for yield

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &None, &0,
            &Some(p2_addr), &50,          // allow up to 50%
        );

        // Supply 50% (maximum)
        let supply = 5_000_000_000_i128;
        vault_client.supply_to_p2(&vault_id, &sender, &supply);

        // Now idle = 5B. We need a deficit, so let's make many packets due
        // so that amount_due > idle.
        let schedule = vault_client.get_vault(&vault_id);
        let interval = (31_536_000u64 * schedule.num_years as u64) / schedule.frequency as u64;

        // Advance enough time for 12 packets (12 * 500M = 6B > 5B idle)
        env.ledger().set_timestamp(schedule.start_timestamp + interval * 12 + 1);

        let sender_before = vesting_client.balance(&sender);

        vault_client.claim_main(&vault_id, &beneficiary);

        let sender_after = vesting_client.balance(&sender);

        // Now a pull from P2 must have happened → sender should have received yield
        assert!(
            sender_after > sender_before,
            "Sender should have received yield from the forced P2 withdrawal"
        );

        // Optional stronger check
        let s = vault_client.get_vault(&vault_id);
        assert!(s.supplied_p2 < supply, "Some funds should have been pulled from P2");
    }


    // ============================================================
    // 4. Double supply then full withdraw (share math)
    // ============================================================
    #[test]
    fn test_p2_double_supply_then_full_withdraw() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let vault_contract_id = env.register(TimeCapsule, ());
        let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

        let p1_token_id = env.register(MockToken, ());
        let treasury = Address::generate(&env);

        let mirai_id = env.register(MockToken, ());
        let mirai_client = MockTokenClient::new(&env, &mirai_id);
        mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "MIRAI"), &String::from_str(&env, "MIRAI"));
        vault_client.initialize(&mirai_id, &10_000_000, &5_000_000, &false, &p1_token_id, &treasury);
        mirai_client.mint(&vault_contract_id, &5_000_000);

        let vesting_id = env.register(MockToken, ());
        let vesting_client = MockTokenClient::new(&env, &vesting_id);
        vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

        let sender = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let total = 10_000_000_000_i128;
        vesting_client.mint(&sender, &total);

        let p2_addr = env.register(MockDefindex, ());
        let p2_client = MockDefindexClient::new(&env, &p2_addr);
        p2_client.initialize(&vesting_id);
        vesting_client.mint(&p2_addr, &2_000_000_000);

        let vault_id = vault_client.create_fund(
            &sender, &beneficiary, &vesting_id, &total,
            &5, &4, &50,
            &None, &0,
            &Some(p2_addr), &50,
        );

        let amount1 = 2_000_000_000_i128;
        let amount2 = 3_000_000_000_i128;

        vault_client.supply_to_p2(&vault_id, &sender, &amount1);
        vault_client.supply_to_p2(&vault_id, &sender, &amount2);

        let s = vault_client.get_vault(&vault_id);
        assert_eq!(s.supplied_p2, amount1 + amount2);
        assert_eq!(s.p2_shares, amount1 + amount2);
        assert_eq!(s.p2_principal, amount1 + amount2);

        // Full withdraw
        vault_client.withdraw_from_p2(&vault_id, &sender, &(amount1 + amount2));

        let s2 = vault_client.get_vault(&vault_id);
        assert_eq!(s2.supplied_p2, 0);
        assert_eq!(s2.p2_shares, 0);
        assert_eq!(s2.p2_principal, 0);
        assert_eq!(s2.idle_amount, total);
    }




}
