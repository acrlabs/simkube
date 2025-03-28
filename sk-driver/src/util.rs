use std::cmp::max;

use crate::DriverContext;

pub fn compute_step_size(ctx: &DriverContext, start_ts: i64, end_ts: i64) -> u64 {
    let speed = ctx.sim.spec.driver.speed;
    let normal_step_duration = max(0, end_ts - start_ts) as f64;
    (normal_step_duration / speed) as u64
}
