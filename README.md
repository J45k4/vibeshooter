# vibeshooter

Simple web FPS prototype with a Rust-authoritative Rapier backend and a Three.js browser client.

## Quick start

1. Install Rust (stable) via `rustup`.
2. Install client dependencies:

```bash
cd client
npm install
```

3. Start the authoritative server from the repo root:

```bash
cargo run -p server
```

4. In a second shell, start the Vite client:

```bash
cd client
npm run dev
```

5. Open the printed Vite URL in the browser and connect to `ws://127.0.0.1:3000/ws`.

## Project structure

- `Cargo.toml`: Workspace root manifest.
- `crates/server`: Authoritative Rust game server.
- `client`: TypeScript + Three.js browser client.
- `crates/server/src/world.rs`: FPS arena, player controller, targets, and hitscan logic.
- `crates/server/src/server.rs`: Axum WebSocket server and session loop.
- `crates/server/src/protocol.rs`: Shared JSON protocol shapes.

## Notes

- v1 supports one active player session at a time.
- The browser renders a graybox arena while the Rust backend owns movement, collision, and hit detection.
