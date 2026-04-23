#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

#[contract]
pub struct MiraiPromo;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    NextIndex,                    // u32 - next available slot (0..100)
    Registered(Address),          // address => bool
    Registrant(u32),              // index => address
    Admin,
}

#[contractimpl]
impl MiraiPromo {

    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::NextIndex, &0u32);
    }

    fn get_admin(env: &Env) -> Address {
        env.storage().persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"))
    }

    // ====================== REGISTER ======================
    pub fn register(env: Env, user: Address) {
        user.require_auth();

        // Prevent double registration
        if env.storage().persistent().has(&DataKey::Registered(user.clone())) {
            panic!("Address already registered");
        }

        let mut next_index: u32 = env.storage().persistent()
            .get(&DataKey::NextIndex)
            .unwrap_or(0);

        if next_index >= 100 {
            panic!("Registration is full. Only 100 early access slots available.");
        }

        // Save registrant
        env.storage().persistent().set(&DataKey::Registrant(next_index), &user);

        // Mark as registered
        env.storage().persistent().set(&DataKey::Registered(user.clone()), &true);

        next_index += 1;
        env.storage().persistent().set(&DataKey::NextIndex, &next_index);
    }

    // ====================== VIEW FUNCTIONS ======================

    pub fn is_registered(env: Env, user: Address) -> bool {
        env.storage().persistent()
            .get(&DataKey::Registered(user))
            .unwrap_or(false)
    }

    pub fn get_registrant(env: Env, index: u32) -> Address {
        if index >= 100 {
            panic!("Index out of range");
        }
        env.storage().persistent()
            .get(&DataKey::Registrant(index))
            .unwrap_or_else(|| panic!("No registrant at this index"))
    }

    pub fn total_registered(env: Env) -> u32 {
        env.storage().persistent()
            .get(&DataKey::NextIndex)
            .unwrap_or(0)
    }

    // NEW: Return all registered addresses as a Vec
    pub fn get_all_registered(env: Env) -> Vec<Address> {
        let total = Self::total_registered(env.clone());
        let mut list = Vec::new(&env);

        for i in 0..total {
            if let Some(addr) = env.storage().persistent().get(&DataKey::Registrant(i)) {
                list.push_back(addr);
            }
        }
        list
    }

    // Optional: Admin-only function to reset (useful for testing)
    pub fn reset(env: Env) {
        let admin = Self::get_admin(&env);
        admin.require_auth();

        // Clear everything (only for testing phase)
        env.storage().persistent().set(&DataKey::NextIndex, &0u32);
    }
}
