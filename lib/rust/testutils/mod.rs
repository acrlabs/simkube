pub mod clock;
pub mod fake;
pub mod pods;
pub mod store;

pub const EMPTY_OBJ_HASH: u64 = 15130871412783076140;
pub const EMPTY_POD_SPEC_HASH: u64 = 17506812802394981455;
pub const TEST_DEPLOYMENT: &str = "the-deployment";
pub const TEST_NAMESPACE: &str = "test";
pub const TEST_SIM_NAME: &str = "test-sim";
pub const TEST_SIM_ROOT_NAME: &str = "test-sim-root";

pub use clock::MockUtcClock;
pub use pods::test_pod;
use rstest::*;
pub use store::MockTraceStore;
