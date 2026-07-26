#![no_std]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error, symbol_short, Address,
    Env, String, Symbol, Vec,
};

mod admin;
mod allowance;
mod balance;
mod compliance;
mod kyc;
mod metadata;
mod storage_types;

#[cfg(test)]
mod test;

#[cfg(test)]
mod sep41_compliance;

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RwaError {
    AlreadyInitialized = 1,
    KycNotApproved = 2,
    TransferBlocked = 3,
    InsufficientBalance = 4,
    AllowanceExpired = 5,
    InsufficientAllowance = 6,
    AccountFrozen = 7,
    /// Transfer amount is zero or negative.
    NegativeAmount = 8,
    /// Batch recipient list exceeds the maximum of 10 entries.
    BatchTooLarge = 9,
    /// A transfer is already in progress on this contract invocation path.
    TransferReentrant = 10,
}

// ── Public types ──────────────────────────────────────────────────────────────

pub const META_LEGAL_ENTITY: &str = "legal_ent";
pub const META_GOVERNING_LAW: &str = "gov_law";
pub const META_ISIN: &str = "isin";
pub const META_PROSPECTUS_HASH: &str = "pros_hash";

#[contracttype]
#[derive(Clone)]
pub struct ComplianceMetadata {
    pub legal_entity: Option<String>,
    pub governing_law: Option<String>,
    pub isin: Option<String>,
    pub prospectus_hash: Option<String>,
}

#[contracttype]
#[derive(Clone)]
pub struct RecipientEntry {
    pub to: Address,
    pub amount: i128,
}

// ── Contract ──────────────────────────────────────────────────────────────────
//
// # Transfer safety invariant (reentrancy guard)
//
// Every transfer entry point (`transfer`, `transfer_from`, `batch_transfer`,
// `batch_transfer_from`) wraps its execution in `enter_transfer_guard` /
// `exit_transfer_guard`. The guard stores a boolean flag in instance storage
// (`DataKey::TransferLock`) and panics with `TransferReentrant` if a nested
// entry is attempted while the flag is set.
//
// In Soroban's single-threaded WASM execution model, true reentrancy within
// a single invocation is not possible. This guard exists to make the
// check-effects-interactions ordering *explicit and enforceable* as an
// on-chain invariant, so that future cross-contract extensions that might
// invoke this contract during a transfer are caught at the border rather
// than silently corrupting state.
//
// Sequence every transfer entry point must follow:
//   1. `enter_transfer_guard` — fail fast if already locked.
//   2. Auth check (require_auth).
//   3. **Validation pass** — all checks (frozen, KYC, compliance, balances).
//      No state is mutated during this phase.
//   4. **Mutation pass** — balance updates, holder registration changes.
//   5. Event emission.
//   6. `exit_transfer_guard` — clear the lock.

#[contract]
pub struct RwaToken;

#[contractimpl]
impl RwaToken {
    // ── Constructor ───────────────────────────────────────────────────────────

    /// Deploy-time constructor — eliminates the deploy→initialize front-running window.
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        env: Env,
        admin: Address,
        decimal: u32,
        name: String,
        symbol: String,
        asset_type: String,
        kyc_registry: Address,
        compliance_engine: Address,
        compliance_metadata: Option<ComplianceMetadata>,
    ) {
        if asset_type != String::from_str(&env, "invoice")
            && asset_type != String::from_str(&env, "property")
            && asset_type != String::from_str(&env, "carbon_credit")
        {
            panic!("invalid asset_type: must be 'invoice', 'property', or 'carbon_credit'");
        }
        admin::write_admin(&env, &admin);
        metadata::write_metadata(&env, decimal, name, symbol);
        metadata::write_asset_type(&env, asset_type);
        kyc::write_kyc_registry(&env, &kyc_registry);
        compliance::write_compliance_engine(&env, &compliance_engine);
        balance::write_total_supply(&env, 0);
        if let Some(meta) = compliance_metadata {
            if let Some(v) = meta.legal_entity {
                compliance::write_metadata(&env, Symbol::new(&env, META_LEGAL_ENTITY), v);
            }
            if let Some(v) = meta.governing_law {
                compliance::write_metadata(&env, Symbol::new(&env, META_GOVERNING_LAW), v);
            }
            if let Some(v) = meta.isin {
                compliance::write_metadata(&env, Symbol::new(&env, META_ISIN), v);
            }
            if let Some(v) = meta.prospectus_hash {
                compliance::write_metadata(&env, Symbol::new(&env, META_PROSPECTUS_HASH), v);
            }
        }
    }

    /// Legacy entry point — always panics.
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        env: Env,
        _admin: Address,
        _decimal: u32,
        _name: String,
        _symbol: String,
        _asset_type: String,
        _kyc_registry: Address,
        _compliance_engine: Address,
    ) {
        panic_with_error!(env, RwaError::AlreadyInitialized);
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    #[deprecated(since = "0.2.0", note = "Use propose_admin and accept_admin instead")]
    pub fn set_admin(env: Env, new_admin: Address) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        admin::write_admin(&env, &new_admin);
        env.events()
            .publish((symbol_short!("admin"),), (admin, new_admin));
    }

    pub fn update_kyc_registry(env: Env, new_registry: Address) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        kyc::write_kyc_registry(&env, &new_registry);
        env.events()
            .publish((symbol_short!("upd_kyc"),), new_registry);
    }

    pub fn update_compliance_engine(env: Env, new_engine: Address) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        compliance::write_compliance_engine(&env, &new_engine);
        env.events()
            .publish((symbol_short!("upd_ce"),), new_engine);
    }

    // ── Freeze / Unfreeze ─────────────────────────────────────────────────────

    pub fn freeze(env: Env, addr: Address) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        compliance::set_frozen(&env, &addr, true);
        env.events().publish((symbol_short!("frozen"),), addr);
    }

    pub fn unfreeze(env: Env, addr: Address) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        compliance::set_frozen(&env, &addr, false);
        env.events().publish((symbol_short!("unfrozen"),), addr);
    }

    pub fn is_frozen(env: Env, addr: Address) -> bool {
        compliance::is_frozen(&env, &addr)
    }

    // ── SEP-41 Token Interface ────────────────────────────────────────────────

    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        allowance::read_allowance(&env, from, spender).amount
    }

    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        allowance::write_allowance(&env, from.clone(), spender.clone(), amount, expiration_ledger);
        env.events().publish(
            (symbol_short!("approve"), from, spender),
            (amount, expiration_ledger),
        );
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        balance::read_balance(&env, id)
    }

    /// Single transfer: validates sender + recipient against all invariants, then
    /// applies balance changes and holder registration/deregistration atomically.
    ///
    /// Sequence: guard → auth → validate_sender → validate_recipient →
    ///           apply_transfer_leg → unregister_holder (if drained) → emit → unlock.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        Self::enter_transfer_guard(&env);
        from.require_auth();
        Self::validate_sender(&env, &from);
        Self::validate_recipient(&env, &from, &to, amount);
        let from_bal = balance::read_balance(&env, from.clone());
        Self::apply_transfer_leg(&env, &from, &to, amount);
        if from != to && from_bal == amount {
            compliance::unregister_holder(&env, &from);
        }
        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
        Self::exit_transfer_guard(&env);
    }

    /// Delegated single transfer: identical invariants to `transfer`, plus allowance deduction.
    ///
    /// Sequence: guard → auth → validate_sender → validate_recipient →
    ///           spend_allowance → apply_transfer_leg → unregister → emit → unlock.
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        Self::enter_transfer_guard(&env);
        spender.require_auth();
        Self::validate_sender(&env, &from);
        Self::validate_recipient(&env, &from, &to, amount);
        allowance::spend_allowance(&env, from.clone(), spender, amount);
        let from_bal = balance::read_balance(&env, from.clone());
        Self::apply_transfer_leg(&env, &from, &to, amount);
        if from != to && from_bal == amount {
            compliance::unregister_holder(&env, &from);
        }
        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
        Self::exit_transfer_guard(&env);
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        if amount <= 0 {
            panic_with_error!(env, RwaError::NegativeAmount);
        }
        if compliance::is_frozen(&env, &from) {
            panic_with_error!(env, RwaError::AccountFrozen);
        }
        kyc::require_kyc(&env, &from);
        let from_balance_before = balance::read_balance(&env, from.clone());
        balance::spend_balance(&env, from.clone(), amount);
        if from_balance_before == amount {
            compliance::unregister_holder(&env, &from);
        }
        let supply = balance::read_total_supply(&env);
        balance::write_total_supply(&env, supply - amount);
        env.events().publish((symbol_short!("burn"), from), amount);
    }

    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        if amount <= 0 {
            panic_with_error!(env, RwaError::NegativeAmount);
        }
        if compliance::is_frozen(&env, &from) {
            panic_with_error!(env, RwaError::AccountFrozen);
        }
        kyc::require_kyc(&env, &from);
        let from_balance_before = balance::read_balance(&env, from.clone());
        allowance::spend_allowance(&env, from.clone(), spender, amount);
        balance::spend_balance(&env, from.clone(), amount);
        if from_balance_before == amount {
            compliance::unregister_holder(&env, &from);
        }
        let supply = balance::read_total_supply(&env);
        balance::write_total_supply(&env, supply - amount);
        env.events().publish((symbol_short!("burn"), from), amount);
    }

    pub fn decimals(env: Env) -> u32 {
        metadata::read_decimal(&env)
    }

    pub fn name(env: Env) -> String {
        metadata::read_name(&env)
    }

    pub fn symbol(env: Env) -> String {
        metadata::read_symbol(&env)
    }

    pub fn total_supply(env: Env) -> i128 {
        balance::read_total_supply(&env)
    }

    // ── Minting ───────────────────────────────────────────────────────────────

    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        if amount <= 0 {
            panic_with_error!(env, RwaError::NegativeAmount);
        }
        kyc::require_kyc(&env, &to);
        let previous_balance = balance::read_balance(&env, to.clone());
        if previous_balance == 0 {
            compliance::check_transfer(&env, &to, &to, amount);
        }
        balance::receive_balance(&env, to.clone(), amount);
        if previous_balance == 0 {
            compliance::register_holder(&env, &to);
        }
        let supply = balance::read_total_supply(&env);
        balance::write_total_supply(&env, supply + amount);
        env.events().publish((symbol_short!("mint"), to), amount);
    }

    // ── Batch Transfer ────────────────────────────────────────────────────────

    /// Atomic batch transfer from `from` to up to 10 recipients.
    ///
    /// Invariants enforced before any state change:
    /// - Recipient list must not exceed 10 entries.
    /// - `from` must not be frozen and must have KYC approval.
    /// - Each entry amount must be positive.
    /// - Each recipient must not be frozen and must have KYC approval.
    /// - Each leg must pass the compliance engine's `can_transfer` check.
    /// - `from` must hold at least the sum of all entry amounts (total balance check).
    ///
    /// If any check fails, the entire batch is rejected with no state changes.
    pub fn batch_transfer(env: Env, from: Address, recipients: Vec<RecipientEntry>) {
        Self::enter_transfer_guard(&env);
        let len = recipients.len();
        if len > 10 {
            panic_with_error!(env, RwaError::BatchTooLarge);
        }
        from.require_auth();
        Self::validate_sender(&env, &from);

        // ── Validation pass (no state changes) ───────────────────────────────
        let mut total_amount: i128 = 0;
        for i in 0..len {
            let entry = recipients.get(i).expect("recipient index out of bounds");
            Self::validate_recipient(&env, &from, &entry.to, entry.amount);
            total_amount = total_amount
                .checked_add(entry.amount)
                .unwrap_or(i128::MAX);
        }

        // Total balance check — guarantees the execution pass cannot fail due
        // to an insufficient-balance panic midway through, which would leave
        // recipients-before-the-failure with tokens while later ones have none.
        let from_balance = balance::read_balance(&env, from.clone());
        if from_balance < total_amount {
            panic_with_error!(env, RwaError::InsufficientBalance);
        }

        // ── Execution pass (guaranteed to succeed after validation) ───────────
        for i in 0..len {
            let entry = recipients.get(i).expect("recipient index out of bounds");
            Self::apply_transfer_leg(&env, &from, &entry.to, entry.amount);
            env.events().publish(
                (symbol_short!("transfer"), from.clone(), entry.to.clone()),
                entry.amount,
            );
        }

        // Deregister sender if their balance was fully drained.
        if balance::read_balance(&env, from.clone()) == 0 {
            compliance::unregister_holder(&env, &from);
        }
        Self::exit_transfer_guard(&env);
    }

    /// Atomic delegated batch transfer: identical invariants to `batch_transfer`,
    /// plus a single upfront allowance deduction for the total transferred amount.
    pub fn batch_transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        recipients: Vec<RecipientEntry>,
    ) {
        Self::enter_transfer_guard(&env);
        let len = recipients.len();
        if len > 10 {
            panic_with_error!(env, RwaError::BatchTooLarge);
        }
        spender.require_auth();
        Self::validate_sender(&env, &from);

        // ── Validation pass ───────────────────────────────────────────────────
        let mut total_amount: i128 = 0;
        for i in 0..len {
            let entry = recipients.get(i).expect("recipient index out of bounds");
            Self::validate_recipient(&env, &from, &entry.to, entry.amount);
            total_amount = total_amount
                .checked_add(entry.amount)
                .unwrap_or(i128::MAX);
        }

        // Balance check before any mutation.
        let from_balance = balance::read_balance(&env, from.clone());
        if from_balance < total_amount {
            panic_with_error!(env, RwaError::InsufficientBalance);
        }

        // Consume the entire allowance for the batch upfront.
        allowance::spend_allowance(&env, from.clone(), spender, total_amount);

        // ── Execution pass ────────────────────────────────────────────────────
        for i in 0..len {
            let entry = recipients.get(i).expect("recipient index out of bounds");
            Self::apply_transfer_leg(&env, &from, &entry.to, entry.amount);
            env.events().publish(
                (symbol_short!("transfer"), from.clone(), entry.to.clone()),
                entry.amount,
            );
        }

        if balance::read_balance(&env, from.clone()) == 0 {
            compliance::unregister_holder(&env, &from);
        }
        Self::exit_transfer_guard(&env);
    }

    // ── RWA Compliance Metadata ───────────────────────────────────────────────

    pub fn asset_type(env: Env) -> String {
        metadata::read_asset_type(&env)
    }

    pub fn kyc_registry(env: Env) -> Address {
        kyc::read_kyc_registry(&env)
    }

    pub fn compliance_engine(env: Env) -> Address {
        compliance::read_compliance_engine(&env)
    }

    pub fn set_compliance_metadata(env: Env, key: Symbol, value: String) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        compliance::write_metadata(&env, key, value);
    }

    pub fn get_compliance_metadata(env: Env, key: Symbol) -> String {
        compliance::read_metadata(&env, key)
    }

    pub fn get_all_compliance_metadata(env: Env) -> ComplianceMetadata {
        let read = |key: &str| {
            let v = compliance::read_metadata(&env, Symbol::new(&env, key));
            if v.len() > 0 { Some(v) } else { None }
        };
        ComplianceMetadata {
            legal_entity: read(META_LEGAL_ENTITY),
            governing_law: read(META_GOVERNING_LAW),
            isin: read(META_ISIN),
            prospectus_hash: read(META_PROSPECTUS_HASH),
        }
    }

    pub fn version(env: Env) -> String {
        String::from_str(&env, env!("CARGO_PKG_VERSION"))
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Validates the sending side of a transfer: frozen check + KYC.
    /// Called once per transfer (or once per batch, not per entry).
    fn validate_sender(env: &Env, from: &Address) {
        if compliance::is_frozen(env, from) {
            panic_with_error!(env, RwaError::AccountFrozen);
        }
        kyc::require_kyc(env, from);
    }

    /// Validates one recipient entry: positive amount, frozen check, KYC, and
    /// the compliance engine's `can_transfer` rule set.
    fn validate_recipient(env: &Env, from: &Address, to: &Address, amount: i128) {
        if amount <= 0 {
            panic_with_error!(env, RwaError::NegativeAmount);
        }
        if compliance::is_frozen(env, to) {
            panic_with_error!(env, RwaError::AccountFrozen);
        }
        kyc::require_kyc(env, to);
        compliance::check_transfer(env, from, to, amount);
    }

    /// Applies the balance mutations and recipient holder registration for one
    /// transfer leg. Does NOT handle sender deregistration — callers must do
    /// that after all legs are applied so batch semantics remain correct.
    fn apply_transfer_leg(env: &Env, from: &Address, to: &Address, amount: i128) {
        let to_balance_before = balance::read_balance(env, to.clone());
        balance::spend_balance(env, from.clone(), amount);
        balance::receive_balance(env, to.clone(), amount);
        if from != to && to_balance_before == 0 {
            compliance::register_holder(env, to);
        }
    }

    // ── Reentrancy guard helpers (#345) ───────────────────────────────────────

    /// Acquires the transfer lock. Panics with `TransferReentrant` if the lock
    /// is already held — indicating a nested transfer call on the same path.
    fn enter_transfer_guard(env: &Env) {
        if env
            .storage()
            .instance()
            .get::<storage_types::DataKey, bool>(&storage_types::DataKey::TransferLock)
            .unwrap_or(false)
        {
            panic_with_error!(env, RwaError::TransferReentrant);
        }
        env.storage()
            .instance()
            .set(&storage_types::DataKey::TransferLock, &true);
    }

    /// Releases the transfer lock. Always called at the end of every guarded
    /// transfer entry point, regardless of success or failure (Soroban panics
    /// roll back storage, so the lock is automatically cleared on panic — this
    /// call only handles the normal completion path).
    fn exit_transfer_guard(env: &Env) {
        env.storage()
            .instance()
            .remove(&storage_types::DataKey::TransferLock);
    }
}
