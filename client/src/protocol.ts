export type ClientMessage =
  | { type: "join" }
  | {
      type: "input";
      sequence: number;
      moveX: number;
      moveZ: number;
      jumpPressed: boolean;
      jumpHeld: boolean;
      firePressed: boolean;
      yaw: number;
      pitch: number;
      frameDt: number;
    }
  | { type: "reset" };

export type WelcomeMessage = {
  type: "welcome";
  playerId: number;
  tickRateHz: number;
  snapshotRateHz: number;
  eyeHeight: number;
};

export type PlayerSnapshot = {
  position: [number, number, number];
  velocity: [number, number, number];
  onGround: boolean;
  yaw: number;
  pitch: number;
};

export type TargetSnapshot = {
  id: number;
  position: [number, number, number];
  alive: boolean;
};

export type HitEvent = {
  targetId: number;
  score: number;
};

export type SnapshotMessage = {
  type: "snapshot";
  tick: number;
  lastProcessedInput: number;
  player: PlayerSnapshot;
  targets: TargetSnapshot[];
  score: number;
  recentHits: HitEvent[];
  gameOver: boolean;
};

export type GameOverMessage = {
  type: "game_over";
  reason: string;
  score: number;
};

export type ServerFullMessage = {
  type: "server_full";
  message: string;
};

export type ServerMessage =
  | WelcomeMessage
  | SnapshotMessage
  | GameOverMessage
  | ServerFullMessage;

