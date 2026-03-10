use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FixedStepSim {
    fixed_dt: f32,
    total_steps: u64,
    current_step: u64,
}

impl FixedStepSim {
    pub fn new(duration: Duration, tick_rate_hz: u32) -> Self {
        let fixed_dt = 1.0 / tick_rate_hz as f32;
        let total_steps = duration.as_secs_f64() * tick_rate_hz as f64;

        Self {
            fixed_dt,
            total_steps: total_steps.round() as u64,
            current_step: 0,
        }
    }

    pub fn fixed_dt(&self) -> f32 {
        self.fixed_dt
    }

    pub fn next_step(&mut self) -> Option<u64> {
        if self.current_step >= self.total_steps {
            return None;
        }

        let step = self.current_step;
        self.current_step += 1;
        Some(step)
    }
}
