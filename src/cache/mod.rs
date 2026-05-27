pub mod advisor;
pub mod stats;

pub use advisor::{CacheAdvisor, MemoryPressure};
pub use stats::{CacheStatsCollector, CacheStatsStore};

/// MCP protocol version for cache strategy API
pub const CACHE_PROTOCOL_VERSION: u64 = 1;
