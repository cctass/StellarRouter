/// Metrics collectors for router-execution and router-quote contracts.
///
/// This module defines the Prometheus metric descriptors and the scrape logic
/// for the two off-chain contracts. It is designed to be integrated into the
/// main metrics exporter binary alongside the existing contract collectors.
///
/// ## Metrics exposed
///
/// ### router-execution
/// | Metric | Type | Description |
/// |---|---|---|
/// | `router_execution_total_executions` | Counter | Cumulative successful executions |
/// | `router_execution_total_errors` | Counter | Cumulative execution errors |
/// | `router_execution_error_rate` | Gauge | errors / (executions + errors) |
/// | `router_execution_max_retries` | Gauge | Configured max retry cap |
///
/// ### router-quote
/// | Metric | Type | Description |
/// |---|---|---|
/// | `router_quote_total_quotes` | Counter | Cumulative `get_quote` calls |
/// | `router_quote_total_fee_estimates` | Counter | Cumulative `estimate_fee` calls |
/// | `router_quote_surge_pricing_active` | Gauge | 1 if last estimate had surge pricing |
///
/// ## Integration
///
/// Add the following to your exporter's main scrape loop:
///
/// ```rust,ignore
/// use crate::collector::{ExecutionCollector, QuoteCollector};
///
/// let exec = ExecutionCollector::new(&rpc_client, &execution_contract_id);
/// let quote = QuoteCollector::new(&rpc_client, &quote_contract_id);
///
/// // In your scrape handler:
/// exec.collect(&mut registry).await?;
/// quote.collect(&mut registry).await?;
/// ```
use std::collections::HashMap;

/// Scraped metrics from router-execution.
#[derive(Debug, Default)]
pub struct ExecutionMetrics {
    /// Cumulative successful executions (`TotalExecutions` storage key).
    pub total_executions: u64,
    /// Cumulative errors (`TotalErrors` storage key).
    pub total_errors: u64,
    /// Configured max retry cap (`MaxRetries` storage key).
    pub max_retries: u32,
}

impl ExecutionMetrics {
    /// Error rate as a fraction (0.0–1.0). Returns 0.0 if no calls have been made.
    pub fn error_rate(&self) -> f64 {
        let total = self.total_executions + self.total_errors;
        if total == 0 {
            0.0
        } else {
            self.total_errors as f64 / total as f64
        }
    }

    /// Render metrics in Prometheus text exposition format.
    pub fn to_prometheus(&self) -> String {
        let mut out = String::new();

        out.push_str("# HELP router_execution_total_executions Cumulative successful executions\n");
        out.push_str("# TYPE router_execution_total_executions counter\n");
        out.push_str(&format!(
            "router_execution_total_executions {}\n",
            self.total_executions
        ));

        out.push_str("# HELP router_execution_total_errors Cumulative execution errors\n");
        out.push_str("# TYPE router_execution_total_errors counter\n");
        out.push_str(&format!(
            "router_execution_total_errors {}\n",
            self.total_errors
        ));

        out.push_str("# HELP router_execution_error_rate Fraction of calls that resulted in an error\n");
        out.push_str("# TYPE router_execution_error_rate gauge\n");
        out.push_str(&format!(
            "router_execution_error_rate {:.6}\n",
            self.error_rate()
        ));

        out.push_str("# HELP router_execution_max_retries Configured maximum retry cap\n");
        out.push_str("# TYPE router_execution_max_retries gauge\n");
        out.push_str(&format!(
            "router_execution_max_retries {}\n",
            self.max_retries
        ));

        out
    }
}

/// Scraped metrics from router-quote.
#[derive(Debug, Default)]
pub struct QuoteMetrics {
    /// Cumulative `get_quote` invocations.
    pub total_quotes: u64,
    /// Cumulative `estimate_fee` invocations.
    pub total_fee_estimates: u64,
    /// Whether the most recent fee estimate applied surge pricing.
    pub surge_pricing_active: bool,
}

impl QuoteMetrics {
    /// Render metrics in Prometheus text exposition format.
    pub fn to_prometheus(&self) -> String {
        let mut out = String::new();

        out.push_str("# HELP router_quote_total_quotes Cumulative get_quote invocations\n");
        out.push_str("# TYPE router_quote_total_quotes counter\n");
        out.push_str(&format!(
            "router_quote_total_quotes {}\n",
            self.total_quotes
        ));

        out.push_str("# HELP router_quote_total_fee_estimates Cumulative estimate_fee invocations\n");
        out.push_str("# TYPE router_quote_total_fee_estimates counter\n");
        out.push_str(&format!(
            "router_quote_total_fee_estimates {}\n",
            self.total_fee_estimates
        ));

        out.push_str("# HELP router_quote_surge_pricing_active 1 if the last fee estimate applied surge pricing\n");
        out.push_str("# TYPE router_quote_surge_pricing_active gauge\n");
        out.push_str(&format!(
            "router_quote_surge_pricing_active {}\n",
            if self.surge_pricing_active { 1 } else { 0 }
        ));

        out
    }
}

/// Scrapes router-execution metrics from the Soroban RPC.
///
/// Reads `TotalExecutions`, `TotalErrors`, and `MaxRetries` from the
/// contract's instance storage via `getLedgerEntries`.
pub struct ExecutionCollector {
    rpc_url: String,
    contract_id: String,
}

impl ExecutionCollector {
    pub fn new(rpc_url: impl Into<String>, contract_id: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            contract_id: contract_id.into(),
        }
    }

    /// Scrape the contract and return the current metrics.
    ///
    /// In a production implementation this calls `getLedgerEntries` with the
    /// XDR-encoded storage keys for `TotalExecutions`, `TotalErrors`, and
    /// `MaxRetries`. The placeholder below returns zeroed metrics and should
    /// be replaced with real RPC calls using the `stellar-xdr` crate.
    pub async fn scrape(&self) -> Result<ExecutionMetrics, String> {
        // TODO: replace with real getLedgerEntries call
        // Keys to fetch:
        //   DataKey::TotalExecutions  → u64
        //   DataKey::TotalErrors      → u64
        //   DataKey::MaxRetries       → u32
        Ok(ExecutionMetrics::default())
    }
}

/// Scrapes router-quote metrics from the Soroban RPC.
pub struct QuoteCollector {
    rpc_url: String,
    contract_id: String,
}

impl QuoteCollector {
    pub fn new(rpc_url: impl Into<String>, contract_id: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            contract_id: contract_id.into(),
        }
    }

    /// Scrape the contract and return the current metrics.
    ///
    /// router-quote does not currently persist counters in storage — it emits
    /// `quote_generated` and `fee_estimated` events instead. A production
    /// implementation should subscribe to these events via `getEvents` and
    /// maintain counters off-chain, or add storage counters to the contract.
    pub async fn scrape(&self) -> Result<QuoteMetrics, String> {
        // TODO: subscribe to quote_generated and fee_estimated events via getEvents
        // and maintain running counters.
        Ok(QuoteMetrics::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_metrics_error_rate_zero_when_no_calls() {
        let m = ExecutionMetrics::default();
        assert_eq!(m.error_rate(), 0.0);
    }

    #[test]
    fn test_execution_metrics_error_rate_calculated() {
        let m = ExecutionMetrics {
            total_executions: 90,
            total_errors: 10,
            max_retries: 2,
        };
        assert!((m.error_rate() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_execution_metrics_prometheus_output_contains_all_metrics() {
        let m = ExecutionMetrics {
            total_executions: 100,
            total_errors: 5,
            max_retries: 3,
        };
        let output = m.to_prometheus();
        assert!(output.contains("router_execution_total_executions 100"));
        assert!(output.contains("router_execution_total_errors 5"));
        assert!(output.contains("router_execution_error_rate"));
        assert!(output.contains("router_execution_max_retries 3"));
    }

    #[test]
    fn test_quote_metrics_prometheus_output_contains_all_metrics() {
        let m = QuoteMetrics {
            total_quotes: 42,
            total_fee_estimates: 17,
            surge_pricing_active: true,
        };
        let output = m.to_prometheus();
        assert!(output.contains("router_quote_total_quotes 42"));
        assert!(output.contains("router_quote_total_fee_estimates 17"));
        assert!(output.contains("router_quote_surge_pricing_active 1"));
    }

    #[test]
    fn test_surge_pricing_inactive_renders_as_zero() {
        let m = QuoteMetrics {
            surge_pricing_active: false,
            ..Default::default()
        };
        assert!(m.to_prometheus().contains("router_quote_surge_pricing_active 0"));
    }
}
