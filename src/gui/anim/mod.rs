pub mod avatar;
pub mod presence;
pub mod status;
pub mod tools;

use std::time::Duration;

/// Standard animation durations across Snarta GUI.
pub const DUR_PULSE: Duration = Duration::from_millis(900);
pub const DUR_BREATHING: Duration = Duration::from_millis(1800);
pub const DUR_AMBIENT: Duration = Duration::from_millis(3000);
