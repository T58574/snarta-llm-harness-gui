pub mod ambient;
pub mod presence;
pub mod status;
pub mod tools;

use std::time::Duration;

/// Standard animation durations across Snarta GUI.
#[allow(dead_code)]
pub const DUR_FAST: Duration = Duration::from_millis(150);
#[allow(dead_code)]
pub const DUR_NORMAL: Duration = Duration::from_millis(250);
pub const DUR_PULSE: Duration = Duration::from_millis(900);
pub const DUR_BREATHING: Duration = Duration::from_millis(1800);
pub const DUR_AMBIENT: Duration = Duration::from_millis(3000);

#[allow(dead_code)]
pub const AMBIENT_MAX_FPS: f32 = 30.0;
