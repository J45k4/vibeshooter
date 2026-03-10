mod sim;
mod world;

use std::time::Duration;

use sim::FixedStepSim;
use world::GameWorld;

fn main() {
    let mut sim = FixedStepSim::new(Duration::from_secs(5), 60);
    let mut world = GameWorld::new(sim.fixed_dt());

    println!("Starting deterministic server simulation (5s @ 60Hz)...");

    while let Some(step) = sim.next_step() {
        world.step();

        if step % 30 == 0 {
            if let Some(pos) = world.cube_position() {
                println!(
                    "step={step:>3} t={:.2}s cube=({:.3}, {:.3}, {:.3})",
                    step as f32 * sim.fixed_dt(),
                    pos.x,
                    pos.y,
                    pos.z
                );
            }
        }
    }

    println!("Simulation complete.");
}
