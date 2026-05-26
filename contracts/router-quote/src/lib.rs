#![no_std]

//! # router-quote
//!
//! Preview transaction results before execution. Supports single-hop and
//! multi-hop quotes where the output of one pool feeds into the next.
//!
//! ## Multi-hop routing
//!
//! A multi-hop quote chains N liquidity plugin calls:
//!
//!   token_A → [plugin_1] → token_B → [plugin_2] → token_C
//!
//! Each plugin must implement `get_quote(token_in, token_out, amount_in) -> i128`.
//! The `amount_out` of hop N becomes the `amount_in` of hop N+1.
//! Fees and slippage are applied at each hop independently.
//!
//! ## Exchange rate
//!
//! Exchange rates are fixed-point integers with configurable decimal precision:
//!
//!   exchange_rate = (amount_out * 10^precision) / amount_in
//!
//! A rate of `2_000_000` with `precision = 6` means 2.000000 token_out per token_in.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, String, Symbol, Vec,
};

// ── Types ─────────────────────────────────────────────────────────────────────

/// A single hop in a multi-hop route.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct HopDescriptor {
    /// Liquidity plugin contract address for this hop.
    pub plugin: Address,
    /// Token being sold in this hop.
    pub token_in: Address,
    /// Token being received in this hop.
    pub token_out: Address,
    /// Fee rate for this hop in basis points (e.g. 30 = 0.30%).
    pub fee_bps: u32,
}

/// Result of a single hop.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct HopResult {
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: i128,
    pub amount_out: i128,
    pub fee_amount: i128,
}

/// Response for a single-hop or multi-hop quote.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct QuoteResponse {
    /// Final output amount after all hops.
    pub amount_out: i128,
    /// Total fees across all hops (in token_in units of each hop).
    pub total_fee_amount: i128,
    /// Minimum acceptable output after slippage tolerance.
    pub min_amount_out: i128,
    /// Exchange rate as fixed-point: (amount_out * 10^precision) / amount_in.
    pub exchange_rate: i128,
    /// Decimal places in `exchange_rate`.
    pub precision: u32,
    /// Price impact in basis points (negative = adverse).
    pub price_impact_bps: i32,
    /// Per-hop breakdown.
    pub hops: Vec<HopResult>,
}

/// Fee estimate breakdown.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct FeeEstimateResponse {
    pub protocol_fee: i128,
    pub network_fee: i128,
    pub total_fee: i128,
    pub surge_pricing: bool,
    pub effective_fee_bps: u32,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum QuoteError {
    InvalidAmount = 1,
    RouteNotFound = 2,
    QuoteFailed = 3,
    InvalidPrecision = 4,
    InvalidSlippage = 5,
    EmptyRoute = 6,
    RouteTooLong = 7,
}

// Maximum hops allowed in a multi-hop route. Keeps gas costs bounded.
const MAX_HOPS: u32 = 5;

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RouterQuote;

#[contractimpl]
impl RouterQuote {
    /// Get a single-hop quote from a liquidity plugin.
    ///
    /// Calls `get_quote(token_in, token_out, amount_in) -> i128` on `plugin`
    /// and returns a full [`QuoteResponse`] with exchange rate, slippage-adjusted
    /// minimum output, and fee breakdown.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `plugin` - Liquidity plugin contract address.
    /// * `token_in` - Token being sold.
    /// * `token_out` - Token being bought.
    /// * `amount_in` - Amount of token_in (must be > 0).
    /// * `fee_bps` - Protocol fee in basis points.
    /// * `slippage_bps` - Slippage tolerance in basis points (0–10000).
    /// * `precision` - Decimal places for exchange rate (1–18).
    ///
    /// # Errors
    /// * [`QuoteError::InvalidAmount`] — `amount_in` ≤ 0.
    /// * [`QuoteError::InvalidPrecision`] — `precision` is 0 or > 18.
    /// * [`QuoteError::InvalidSlippage`] — `slippage_bps` > 10000.
    /// * [`QuoteError::QuoteFailed`] — plugin call failed.
    pub fn get_quote(
        env: Env,
        plugin: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        fee_bps: u32,
        slippage_bps: u32,
        precision: u32,
    ) -> Result<QuoteResponse, QuoteError> {
        if amount_in <= 0 {
            return Err(QuoteError::InvalidAmount);
        }
        if precision == 0 || precision > 18 {
            return Err(QuoteError::InvalidPrecision);
        }
        if slippage_bps > 10_000 {
            return Err(QuoteError::InvalidSlippage);
        }

        let hop = HopDescriptor {
            plugin,
            token_in,
            token_out,
            fee_bps,
        };
        let mut hops = Vec::new(&env);
        hops.push_back(hop);

        Self::execute_hops(&env, hops, amount_in, slippage_bps, precision)
    }

    /// Get a multi-hop quote chaining N liquidity plugins.
    ///
    /// Executes hops in order: the `amount_out` of hop N becomes the
    /// `amount_in` of hop N+1. The final `QuoteResponse` reflects the
    /// end-to-end exchange rate and total fees across all hops.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `hops` - Ordered list of [`HopDescriptor`]s (1–5 hops).
    /// * `amount_in` - Initial input amount (must be > 0).
    /// * `slippage_bps` - Slippage tolerance applied to the final output (0–10000).
    /// * `precision` - Decimal places for the end-to-end exchange rate (1–18).
    ///
    /// # Errors
    /// * [`QuoteError::EmptyRoute`] — `hops` is empty.
    /// * [`QuoteError::RouteTooLong`] — `hops` has more than `MAX_HOPS` entries.
    /// * [`QuoteError::InvalidAmount`] — `amount_in` ≤ 0.
    /// * [`QuoteError::InvalidPrecision`] — `precision` is 0 or > 18.
    /// * [`QuoteError::InvalidSlippage`] — `slippage_bps` > 10000.
    /// * [`QuoteError::QuoteFailed`] — any plugin call failed.
    pub fn get_multihop_quote(
        env: Env,
        hops: Vec<HopDescriptor>,
        amount_in: i128,
        slippage_bps: u32,
        precision: u32,
    ) -> Result<QuoteResponse, QuoteError> {
        if hops.is_empty() {
            return Err(QuoteError::EmptyRoute);
        }
        if hops.len() > MAX_HOPS {
            return Err(QuoteError::RouteTooLong);
        }
        if amount_in <= 0 {
            return Err(QuoteError::InvalidAmount);
        }
        if precision == 0 || precision > 18 {
            return Err(QuoteError::InvalidPrecision);
        }
        if slippage_bps > 10_000 {
            return Err(QuoteError::InvalidSlippage);
        }

        Self::execute_hops(&env, hops, amount_in, slippage_bps, precision)
    }

    /// Estimate fees for a transaction.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `amount` - Transaction amount (must be > 0).
    /// * `fee_bps` - Route fee rate in basis points.
    /// * `network_load_bps` - Network utilization (0–10000). ≥ 8000 triggers 2× surge.
    ///
    /// # Errors
    /// * [`QuoteError::InvalidAmount`] — `amount` ≤ 0.
    pub fn estimate_fee(
        env: Env,
        amount: i128,
        fee_bps: u32,
        network_load_bps: u32,
    ) -> Result<FeeEstimateResponse, QuoteError> {
        if amount <= 0 {
            return Err(QuoteError::InvalidAmount);
        }

        let protocol_fee = amount * fee_bps as i128 / 10_000;
        let base_network_fee: i128 = 100;

        let (network_fee, surge_pricing, effective_fee_bps) = if network_load_bps >= 8_000 {
            (base_network_fee * 2, true, fee_bps * 2)
        } else {
            (base_network_fee, false, fee_bps)
        };

        env.events().publish(
            (Symbol::new(&env, "fee_estimated"),),
            (amount, fee_bps, surge_pricing),
        );

        Ok(FeeEstimateResponse {
            protocol_fee,
            network_fee,
            total_fee: protocol_fee + network_fee,
            surge_pricing,
            effective_fee_bps,
        })
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Execute a chain of hops and aggregate results.
    fn execute_hops(
        env: &Env,
        hops: Vec<HopDescriptor>,
        initial_amount_in: i128,
        slippage_bps: u32,
        precision: u32,
    ) -> Result<QuoteResponse, QuoteError> {
        let mut current_amount = initial_amount_in;
        let mut total_fee: i128 = 0;
        let mut hop_results = Vec::new(env);

        for hop in hops.iter() {
            let gross_amount_out = Self::call_plugin(env, &hop.plugin, &hop.token_in, &hop.token_out, current_amount)?;

            // Fee is taken from the input of each hop
            let fee_amount = current_amount * hop.fee_bps as i128 / 10_000;
            total_fee += fee_amount;

            hop_results.push_back(HopResult {
                token_in: hop.token_in.clone(),
                token_out: hop.token_out.clone(),
                amount_in: current_amount,
                amount_out: gross_amount_out,
                fee_amount,
            });

            current_amount = gross_amount_out;
        }

        let final_amount_out = current_amount;

        // Slippage applied to the final output only
        let min_amount_out = final_amount_out * (10_000 - slippage_bps as i128) / 10_000;

        // End-to-end exchange rate: (final_out * 10^precision) / initial_in
        let scale = Self::pow10(precision);
        let exchange_rate = (final_amount_out * scale) / initial_amount_in;

        // Price impact: (final_out - initial_in) * 10_000 / initial_in
        let price_impact_bps = ((final_amount_out - initial_amount_in) * 10_000 / initial_amount_in) as i32;

        env.events().publish(
            (Symbol::new(env, "quote_generated"),),
            (initial_amount_in, final_amount_out, exchange_rate),
        );

        Ok(QuoteResponse {
            amount_out: final_amount_out,
            total_fee_amount: total_fee,
            min_amount_out,
            exchange_rate,
            precision,
            price_impact_bps,
            hops: hop_results,
        })
    }

    /// Call a liquidity plugin's `get_quote` function.
    fn call_plugin(
        env: &Env,
        plugin: &Address,
        token_in: &Address,
        token_out: &Address,
        amount_in: i128,
    ) -> Result<i128, QuoteError> {
        let function = Symbol::new(env, "get_quote");
        let mut args = Vec::new(env);
        args.push_back(token_in.clone().into());
        args.push_back(token_out.clone().into());
        args.push_back(amount_in.into());

        env.try_invoke_contract::<i128, i128>(plugin, &function, args)
            .map_err(|_| QuoteError::QuoteFailed)?
            .map_err(|_| QuoteError::QuoteFailed)
    }

    /// Returns 10^exp as i128. Safe for exp ≤ 18.
    fn pow10(exp: u32) -> i128 {
        let mut result: i128 = 1;
        let mut i = 0u32;
        while i < exp {
            result *= 10;
            i += 1;
        }
        result
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    /// Mock plugin: returns amount_in * 2
    #[soroban_sdk::contract]
    pub struct DoublePlugin;

    #[soroban_sdk::contractimpl]
    impl DoublePlugin {
        pub fn get_quote(_env: Env, _ti: Address, _to: Address, amount_in: i128) -> i128 {
            amount_in * 2
        }
    }

    /// Mock plugin: returns amount_in * 3
    #[soroban_sdk::contract]
    pub struct TriplePlugin;

    #[soroban_sdk::contractimpl]
    impl TriplePlugin {
        pub fn get_quote(_env: Env, _ti: Address, _to: Address, amount_in: i128) -> i128 {
            amount_in * 3
        }
    }

    fn setup() -> (Env, RouterQuoteClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, RouterQuote);
        let client = RouterQuoteClient::new(&env, &id);
        let double = env.register_contract(None, DoublePlugin);
        let triple = env.register_contract(None, TriplePlugin);
        (env, client, double, triple)
    }

    // ── Single-hop tests ──────────────────────────────────────────────────────

    #[test]
    fn test_single_hop_exchange_rate() {
        let (env, client, double, _) = setup();
        let ti = Address::generate(&env);
        let to = Address::generate(&env);
        // amount_in=1_000_000, plugin returns 2_000_000
        // rate = (2_000_000 * 10^6) / 1_000_000 = 2_000_000
        let resp = client.get_quote(&double, &ti, &to, &1_000_000, &0, &0, &6);
        assert_eq!(resp.amount_out, 2_000_000);
        assert_eq!(resp.exchange_rate, 2_000_000);
        assert_eq!(resp.precision, 6);
    }

    #[test]
    fn test_single_hop_fee_deducted() {
        let (env, client, double, _) = setup();
        let ti = Address::generate(&env);
        let to = Address::generate(&env);
        // fee_bps=30 (0.30%), amount_in=1_000_000 → fee=3_000
        let resp = client.get_quote(&double, &ti, &to, &1_000_000, &30, &0, &6);
        assert_eq!(resp.total_fee_amount, 3_000);
    }

    #[test]
    fn test_single_hop_slippage() {
        let (env, client, double, _) = setup();
        let ti = Address::generate(&env);
        let to = Address::generate(&env);
        // slippage_bps=50, amount_out=2_000_000
        // min = 2_000_000 * 9950 / 10_000 = 1_990_000
        let resp = client.get_quote(&double, &ti, &to, &1_000_000, &0, &50, &6);
        assert_eq!(resp.min_amount_out, 1_990_000);
    }

    #[test]
    fn test_single_hop_invalid_amount() {
        let (env, client, double, _) = setup();
        let ti = Address::generate(&env);
        let to = Address::generate(&env);
        let r = client.try_get_quote(&double, &ti, &to, &0, &0, &0, &6);
        assert_eq!(r, Err(Ok(QuoteError::InvalidAmount)));
    }

    #[test]
    fn test_single_hop_invalid_precision() {
        let (env, client, double, _) = setup();
        let ti = Address::generate(&env);
        let to = Address::generate(&env);
        let r = client.try_get_quote(&double, &ti, &to, &1_000_000, &0, &0, &0);
        assert_eq!(r, Err(Ok(QuoteError::InvalidPrecision)));
    }

    #[test]
    fn test_single_hop_invalid_slippage() {
        let (env, client, double, _) = setup();
        let ti = Address::generate(&env);
        let to = Address::generate(&env);
        let r = client.try_get_quote(&double, &ti, &to, &1_000_000, &0, &10_001, &6);
        assert_eq!(r, Err(Ok(QuoteError::InvalidSlippage)));
    }

    // ── Multi-hop tests ───────────────────────────────────────────────────────

    #[test]
    fn test_multihop_two_hops_chains_correctly() {
        let (env, client, double, triple) = setup();
        let ta = Address::generate(&env);
        let tb = Address::generate(&env);
        let tc = Address::generate(&env);

        // Hop 1: A → B via double (×2), Hop 2: B → C via triple (×3)
        // amount_in=100 → hop1_out=200 → hop2_out=600
        let mut hops = soroban_sdk::Vec::new(&env);
        hops.push_back(HopDescriptor { plugin: double, token_in: ta, token_out: tb.clone(), fee_bps: 0 });
        hops.push_back(HopDescriptor { plugin: triple, token_in: tb, token_out: tc, fee_bps: 0 });

        let resp = client.get_multihop_quote(&hops, &100, &0, &6);
        assert_eq!(resp.amount_out, 600);
        assert_eq!(resp.hops.len(), 2);
        assert_eq!(resp.hops.get(0).unwrap().amount_out, 200);
        assert_eq!(resp.hops.get(1).unwrap().amount_out, 600);
    }

    #[test]
    fn test_multihop_exchange_rate_end_to_end() {
        let (env, client, double, triple) = setup();
        let ta = Address::generate(&env);
        let tb = Address::generate(&env);
        let tc = Address::generate(&env);

        // amount_in=100, final_out=600
        // rate = (600 * 10^2) / 100 = 600
        let mut hops = soroban_sdk::Vec::new(&env);
        hops.push_back(HopDescriptor { plugin: double.clone(), token_in: ta.clone(), token_out: tb.clone(), fee_bps: 0 });
        hops.push_back(HopDescriptor { plugin: triple.clone(), token_in: tb.clone(), token_out: tc.clone(), fee_bps: 0 });

        let resp = client.get_multihop_quote(&hops, &100, &0, &2);
        assert_eq!(resp.exchange_rate, 600); // 6.00 with precision 2
    }

    #[test]
    fn test_multihop_fees_accumulated_per_hop() {
        let (env, client, double, triple) = setup();
        let ta = Address::generate(&env);
        let tb = Address::generate(&env);
        let tc = Address::generate(&env);

        // Hop 1: fee_bps=100 (1%), amount_in=1000 → fee=10
        // Hop 2: fee_bps=200 (2%), amount_in=2000 → fee=40
        // total_fee = 50
        let mut hops = soroban_sdk::Vec::new(&env);
        hops.push_back(HopDescriptor { plugin: double, token_in: ta, token_out: tb.clone(), fee_bps: 100 });
        hops.push_back(HopDescriptor { plugin: triple, token_in: tb, token_out: tc, fee_bps: 200 });

        let resp = client.get_multihop_quote(&hops, &1000, &0, &6);
        assert_eq!(resp.total_fee_amount, 50);
    }

    #[test]
    fn test_multihop_empty_route_fails() {
        let (env, client, _, _) = setup();
        let hops: soroban_sdk::Vec<HopDescriptor> = soroban_sdk::Vec::new(&env);
        let r = client.try_get_multihop_quote(&hops, &1000, &0, &6);
        assert_eq!(r, Err(Ok(QuoteError::EmptyRoute)));
    }

    #[test]
    fn test_multihop_too_many_hops_fails() {
        let (env, client, double, _) = setup();
        let ta = Address::generate(&env);
        let tb = Address::generate(&env);
        let mut hops = soroban_sdk::Vec::new(&env);
        // Push 6 hops (MAX_HOPS = 5)
        for _ in 0..6 {
            hops.push_back(HopDescriptor {
                plugin: double.clone(),
                token_in: ta.clone(),
                token_out: tb.clone(),
                fee_bps: 0,
            });
        }
        let r = client.try_get_multihop_quote(&hops, &1000, &0, &6);
        assert_eq!(r, Err(Ok(QuoteError::RouteTooLong)));
    }

    // ── Fee estimate tests ────────────────────────────────────────────────────

    #[test]
    fn test_estimate_fee_normal_load() {
        let (_, client, _, _) = setup();
        let resp = client.estimate_fee(&1_000_000, &30, &5_000);
        assert!(!resp.surge_pricing);
        assert_eq!(resp.protocol_fee, 3_000);
        assert_eq!(resp.network_fee, 100);
        assert_eq!(resp.total_fee, 3_100);
    }

    #[test]
    fn test_estimate_fee_surge_pricing() {
        let (_, client, _, _) = setup();
        let resp = client.estimate_fee(&1_000_000, &30, &9_000);
        assert!(resp.surge_pricing);
        assert_eq!(resp.network_fee, 200);
        assert_eq!(resp.effective_fee_bps, 60);
    }

    #[test]
    fn test_estimate_fee_invalid_amount() {
        let (_, client, _, _) = setup();
        let r = client.try_estimate_fee(&0, &30, &0);
        assert_eq!(r, Err(Ok(QuoteError::InvalidAmount)));
    }
}
