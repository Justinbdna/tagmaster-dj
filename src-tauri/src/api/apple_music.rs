//! Client Apple Music / iTunes Search API (région forcée `US` / `UK`).
//!
//! Sprint 1 : Implémentation avec `reqwest` et anti-ban (rate limiting 1 à 2s).

#[allow(dead_code)]
pub const DEFAULT_STOREFRONT: &str = "US";

#[allow(dead_code)]
pub const RATE_LIMIT_MIN_MS: u64 = 1000;

#[allow(dead_code)]
pub const RATE_LIMIT_MAX_MS: u64 = 2000;
