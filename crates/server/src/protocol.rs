use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join,
    Input(ClientInput),
    Reset,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInput {
    pub sequence: u64,
    pub move_x: f32,
    pub move_z: f32,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub fire_pressed: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub frame_dt: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome(WelcomeMessage),
    Snapshot(SnapshotMessage),
    GameOver(GameOverMessage),
    ServerFull { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeMessage {
    pub player_id: u64,
    pub tick_rate_hz: u64,
    pub snapshot_rate_hz: u64,
    pub eye_height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMessage {
    pub tick: u64,
    pub last_processed_input: u64,
    pub player: PlayerSnapshot,
    pub targets: Vec<TargetSnapshot>,
    pub projectiles: Vec<ProjectileSnapshot>,
    pub score: u32,
    pub recent_hits: Vec<HitEvent>,
    pub game_over: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub on_ground: bool,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetSnapshot {
    pub id: u32,
    pub position: [f32; 3],
    pub alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectileSnapshot {
    pub id: u32,
    pub position: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HitEvent {
    pub target_id: u32,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameOverMessage {
    pub reason: String,
    pub score: u32,
}
