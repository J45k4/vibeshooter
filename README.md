# wibeshooter

Initial Rust server foundation for a future networked 3D game world.

## Quick start

1. Install Rust (stable) via `rustup`.
2. From repo root, run:

```bash
cargo run -p server
```

This runs a deterministic fixed-step (60 Hz) Rapier 3D simulation for about 5 seconds and prints the dynamic cube position periodically.

## Project structure

- `Cargo.toml`: Workspace root manifest.
- `crates/server`: Authoritative server simulation binary.
- `crates/server/src/world.rs`: Physics world state and step function.
- `crates/server/src/sim.rs`: Fixed-step timing loop utility.
- `crates/server/src/main.rs`: Bootstrap entry point wiring sim + world.

## Notes

- Server-side simulation is structured to become authoritative for multiplayer networking.
- Client code is intentionally not included yet.
