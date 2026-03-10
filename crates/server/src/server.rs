use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::{
    net::TcpListener,
    sync::{Mutex, mpsc},
    time,
};

use crate::{
    protocol::{ClientInput, ClientMessage, GameOverMessage, ServerMessage, WelcomeMessage},
    sim::{snapshot_interval_ticks, tick_duration, SNAPSHOT_RATE_HZ, TICK_RATE_HZ},
    world::GameWorld,
};

#[derive(Clone)]
pub struct AppState {
    shared: Arc<Mutex<ServerState>>,
}

struct ServerState {
    tick: u64,
    world: GameWorld,
    active_session: Option<ActiveSession>,
    next_player_id: u64,
}

struct ActiveSession {
    player_id: u64,
    latest_input: ClientInput,
    outbound: mpsc::UnboundedSender<String>,
    game_over_sent: bool,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let shared = Arc::new(Mutex::new(ServerState {
        tick: 0,
        world: GameWorld::new(1.0 / TICK_RATE_HZ as f32),
        active_session: None,
        next_player_id: 1,
    }));

    let sim_state = Arc::clone(&shared);
    tokio::spawn(async move {
        simulation_loop(sim_state).await;
    });

    let app_state = AppState { shared };
    let app = Router::new()
        .route("/healthz", get(healthcheck))
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let addr: SocketAddr = "127.0.0.1:3000".parse()?;
    let listener = TcpListener::bind(addr).await?;

    println!("Rust game server listening on ws://{addr}/ws");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn simulation_loop(shared: Arc<Mutex<ServerState>>) {
    let mut interval = time::interval(tick_duration());
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        let mut outbound = Vec::new();
        let mut dropped_player = None;

        {
            let mut state = shared.lock().await;
            state.tick = state.tick.saturating_add(1);
            let tick = state.tick;
            let snapshot_tick = tick % snapshot_interval_ticks() == 0;

            let (input, player_id, already_sent) = if let Some(session) = state.active_session.as_ref() {
                (
                    session.latest_input.clone(),
                    session.player_id,
                    session.game_over_sent,
                )
            } else {
                (ClientInput::default(), 0, false)
            };

            let step = state.world.step(tick, &input);

            if let Some(session) = state.active_session.as_mut() {
                let outbound_tx = session.outbound.clone();
                let last_input_sequence = session.latest_input.sequence;
                let should_send_game_over = step.game_over_just_triggered && !already_sent;

                if should_send_game_over {
                    session.game_over_sent = true;
                }

                if should_send_game_over {
                    outbound.push((
                        player_id,
                        outbound_tx.clone(),
                        ServerMessage::GameOver(GameOverMessage {
                            reason: "fell out of bounds".to_string(),
                            score: state.world.score(),
                        }),
                    ));
                }

                if snapshot_tick {
                    let snapshot = state.world.snapshot(
                        tick,
                        last_input_sequence,
                        step.recent_hits.clone(),
                    );
                    outbound.push((
                        player_id,
                        outbound_tx,
                        ServerMessage::Snapshot(snapshot),
                    ));
                }
            }

            if let Some(session) = state.active_session.as_ref() {
                if session.outbound.is_closed() {
                    dropped_player = Some(session.player_id);
                }
            }
        }

        for (_, sender, message) in outbound {
            if send_message(&sender, &message).is_err() {
                dropped_player = Some(dropped_player.unwrap_or(0));
            }
        }

        if dropped_player.is_some() {
            let mut state = shared.lock().await;
            state.active_session = None;
            state.world.full_reset();
        }
    }
}

async fn healthcheck() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "tickRateHz": TICK_RATE_HZ,
        "snapshotRateHz": SNAPSHOT_RATE_HZ,
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: AppState, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<String>();

    let maybe_player = {
        let mut server = state.shared.lock().await;

        if server.active_session.is_some() {
            None
        } else {
            let player_id = server.next_player_id;
            server.next_player_id = server.next_player_id.saturating_add(1);
            server.world.full_reset();
            server.active_session = Some(ActiveSession {
                player_id,
                latest_input: ClientInput::default(),
                outbound: outbound_tx.clone(),
                game_over_sent: false,
            });
            Some(player_id)
        }
    };

    let Some(player_id) = maybe_player else {
        let payload = serde_json::to_string(&ServerMessage::ServerFull {
            message: "Only one active player is supported in v1.".to_string(),
        })
        .expect("server full payload");
        let _ = sender.send(Message::Text(payload.into())).await;
        let _ = sender.close().await;
        return;
    };

    let welcome = ServerMessage::Welcome(WelcomeMessage {
        player_id,
        tick_rate_hz: TICK_RATE_HZ,
        snapshot_rate_hz: SNAPSHOT_RATE_HZ,
        eye_height: GameWorld::eye_height(),
    });
    let _ = send_message(&outbound_tx, &welcome);

    let writer = tokio::spawn(async move {
        while let Some(payload) = outbound_rx.recv().await {
            if sender.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = receiver.next().await {
        match message {
            Message::Text(text) => {
                if let Ok(client_message) = serde_json::from_str::<ClientMessage>(&text) {
                    process_client_message(&state, player_id, client_message).await;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    writer.abort();

    let mut server = state.shared.lock().await;
    if server
        .active_session
        .as_ref()
        .is_some_and(|session| session.player_id == player_id)
    {
        server.active_session = None;
        server.world.full_reset();
    }
}

async fn process_client_message(state: &AppState, player_id: u64, message: ClientMessage) {
    let mut server = state.shared.lock().await;
    let Some(session) = server.active_session.as_mut() else {
        return;
    };

    if session.player_id != player_id {
        return;
    }

    match message {
        ClientMessage::Join => {}
        ClientMessage::Input(input) => {
            session.latest_input = input;
        }
        ClientMessage::Reset => {
            session.game_over_sent = false;
            server.world.reset_after_game_over();
        }
    }
}

fn send_message(
    sender: &mpsc::UnboundedSender<String>,
    message: &ServerMessage,
) -> Result<(), serde_json::Error> {
    let payload = serde_json::to_string(message)?;
    let _ = sender.send(payload);
    Ok(())
}
