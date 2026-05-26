pub mod advisor;
pub mod stats;

#[allow(unused_imports)]
pub use advisor::{CacheAdvisor, CacheMode, CacheStrategy, MemoryPressure};
#[allow(unused_imports)]
pub use stats::{CacheHitReport, CacheStats, CacheStatsCollector, CacheStatsStore};

/// MCP protocol version for cache strategy API
pub const CACHE_PROTOCOL_VERSION: u64 = 1;
