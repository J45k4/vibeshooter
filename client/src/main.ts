import "./styles.css";

import * as THREE from "three";

import type {
  ClientMessage,
  ProjectileSnapshot,
  ServerMessage,
  SnapshotMessage,
  TargetSnapshot,
  WelcomeMessage,
} from "./protocol";

const SERVER_URL = import.meta.env.VITE_SERVER_URL ?? "ws://127.0.0.1:3000/ws";
const DEFAULT_EYE_HEIGHT = 0.9;
const SNAPSHOT_INTERVAL_MS = 50;

type SnapshotBuffer = {
  previous: SnapshotMessage | null;
  current: SnapshotMessage | null;
  receivedAt: number;
};

type InputState = {
  forward: boolean;
  backward: boolean;
  left: boolean;
  right: boolean;
  jumpHeld: boolean;
  jumpQueued: boolean;
  fireQueued: boolean;
  yaw: number;
  pitch: number;
  sequence: number;
};

type TargetVisual = {
  root: THREE.Mesh;
  glow: THREE.Mesh;
};

type ProjectileVisual = THREE.Mesh;

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) {
  throw new Error("App root not found.");
}

app.innerHTML = `
  <div class="canvas-shell"></div>
  <div class="hud">
    <div class="hud-panel">
      <div class="hud-label">Score</div>
      <div class="hud-value" data-score>0</div>
      <div class="hud-meta" data-ground>Grounded</div>
    </div>
    <div class="crosshair"></div>
    <div class="status-card">
      <div class="status-title">Status</div>
      <div class="status-copy" data-status>Connecting to <strong>${SERVER_URL}</strong>...</div>
    </div>
  </div>
`;

const canvasHost = app.querySelector<HTMLDivElement>(".canvas-shell");
const scoreEl = app.querySelector<HTMLElement>("[data-score]");
const statusEl = app.querySelector<HTMLElement>("[data-status]");
const groundedEl = app.querySelector<HTMLElement>("[data-ground]");

if (!canvasHost || !scoreEl || !statusEl || !groundedEl) {
  throw new Error("HUD elements missing.");
}

const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.shadowMap.enabled = true;
canvasHost.appendChild(renderer.domElement);

const scene = new THREE.Scene();
scene.fog = new THREE.FogExp2(0x0b0f15, 0.032);

const camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 200);
scene.add(camera);

const hemi = new THREE.HemisphereLight(0x9bb5d6, 0x283038, 1.2);
scene.add(hemi);

const sun = new THREE.DirectionalLight(0xfff3d7, 2.1);
sun.position.set(12, 24, 8);
sun.castShadow = true;
sun.shadow.mapSize.set(1024, 1024);
scene.add(sun);

const floor = new THREE.Mesh(
  new THREE.BoxGeometry(40, 1, 40),
  new THREE.MeshStandardMaterial({ color: 0x242d36, roughness: 0.85 }),
);
floor.position.set(0, -0.5, 0);
floor.receiveShadow = true;
scene.add(floor);

const arenaPieces = [
  { size: [40, 5, 1], position: [0, 2.5, -20] },
  { size: [40, 5, 1], position: [0, 2.5, 20] },
  { size: [1, 5, 40], position: [20, 2.5, 0] },
  { size: [1, 5, 40], position: [-20, 2.5, 0] },
  { size: [3, 3, 2], position: [0, 1.5, -7.5] },
  { size: [2, 2, 2], position: [-6, 1, 1] },
  { size: [2.5, 2.5, 2.5], position: [6, 1.25, -1.5] },
];

const wallMaterial = new THREE.MeshStandardMaterial({ color: 0x54606d, roughness: 0.78 });
for (const piece of arenaPieces) {
  const mesh = new THREE.Mesh(
    new THREE.BoxGeometry(piece.size[0], piece.size[1], piece.size[2]),
    wallMaterial,
  );
  mesh.position.set(piece.position[0], piece.position[1], piece.position[2]);
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  scene.add(mesh);
}

const targetGroup = new THREE.Group();
scene.add(targetGroup);
const projectileGroup = new THREE.Group();
scene.add(projectileGroup);

const targetMaterial = new THREE.MeshStandardMaterial({
  color: 0xff6d4d,
  roughness: 0.38,
  metalness: 0.08,
  emissive: 0x4e1207,
});
const glowMaterial = new THREE.MeshBasicMaterial({
  color: 0xffa072,
  transparent: true,
  opacity: 0.22,
});

const targetVisuals = new Map<number, TargetVisual>();
const projectileVisuals = new Map<number, ProjectileVisual>();
for (let id = 1; id <= 5; id += 1) {
  const root = new THREE.Mesh(new THREE.BoxGeometry(0.9, 1.8, 0.7), targetMaterial.clone());
  root.castShadow = true;
  root.receiveShadow = true;
  const glow = new THREE.Mesh(new THREE.BoxGeometry(1.2, 2.1, 1), glowMaterial.clone());
  glow.position.y = 0;
  root.add(glow);
  targetGroup.add(root);
  targetVisuals.set(id, { root, glow });
}

const rifle = new THREE.Group();
const rifleBody = new THREE.Mesh(
  new THREE.BoxGeometry(0.22, 0.18, 1.1),
  new THREE.MeshStandardMaterial({ color: 0x1e2229, roughness: 0.45, metalness: 0.45 }),
);
rifleBody.position.set(0.24, -0.2, -0.55);
const rifleBarrel = new THREE.Mesh(
  new THREE.CylinderGeometry(0.03, 0.03, 0.9, 12),
  new THREE.MeshStandardMaterial({ color: 0x3f4652, roughness: 0.3, metalness: 0.65 }),
);
rifleBarrel.rotation.z = Math.PI / 2;
rifleBarrel.position.set(0.35, -0.18, -1.0);
const muzzleFlash = new THREE.PointLight(0xffd18a, 0, 6, 2);
muzzleFlash.position.set(0.62, -0.16, -1.42);
rifle.add(rifleBody, rifleBarrel, muzzleFlash);
camera.add(rifle);

const clock = new THREE.Clock();
const snapshotBuffer: SnapshotBuffer = {
  previous: null,
  current: null,
  receivedAt: performance.now(),
};

const input: InputState = {
  forward: false,
  backward: false,
  left: false,
  right: false,
  jumpHeld: false,
  jumpQueued: false,
  fireQueued: false,
  yaw: 0,
  pitch: 0,
  sequence: 0,
};

let socket: WebSocket | null = null;
let welcome: WelcomeMessage | null = null;
let reconnectTimer = 0;
let score = 0;
let gameOver = false;
let cameraEyeHeight = DEFAULT_EYE_HEIGHT;
let muzzleUntil = 0;

function setStatus(copy: string, alert = false): void {
  statusEl.innerHTML = copy;
  statusEl.classList.toggle("is-alert", alert);
}

function connect(): void {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }

  setStatus(`Connecting to <strong>${SERVER_URL}</strong>...`);
  socket = new WebSocket(SERVER_URL);

  socket.addEventListener("open", () => {
    setStatus("Connected. Click inside the view to lock the pointer.");
    send({ type: "join" });
  });

  socket.addEventListener("message", (event) => {
    const message = JSON.parse(String(event.data)) as ServerMessage;
    handleServerMessage(message);
  });

  socket.addEventListener("close", () => {
    welcome = null;
    setStatus("Disconnected from the Rust server. Reconnecting...", true);
    window.clearTimeout(reconnectTimer);
    reconnectTimer = window.setTimeout(connect, 1500);
  });
}

function handleServerMessage(message: ServerMessage): void {
  if (message.type === "welcome") {
    welcome = message;
    cameraEyeHeight = message.eyeHeight;
    setStatus("Live. WASD move, mouse look, Space jump, click to fire, R to reset after a fall.");
    return;
  }

  if (message.type === "snapshot") {
    snapshotBuffer.previous = snapshotBuffer.current;
    snapshotBuffer.current = message;
    snapshotBuffer.receivedAt = performance.now();
    score = message.score;
    gameOver = message.gameOver;
    scoreEl.textContent = String(message.score);
    groundedEl.textContent = message.player.onGround ? "Grounded" : "Airborne";
    if (message.recentHits.length > 0) {
      muzzleUntil = performance.now() + 40;
    }
    if (message.gameOver) {
      setStatus("You fell out of bounds. Press <strong>R</strong> to reset.", true);
    }
    return;
  }

  if (message.type === "game_over") {
    gameOver = true;
    setStatus(`Game over: ${message.reason}. Press <strong>R</strong> to respawn.`, true);
    return;
  }

  if (message.type === "server_full") {
    setStatus(message.message, true);
  }
}

function send(message: ClientMessage): void {
  if (!socket || socket.readyState !== WebSocket.OPEN) {
    return;
  }

  socket.send(JSON.stringify(message));
}

function axis(negative: boolean, positive: boolean): number {
  return (positive ? 1 : 0) - (negative ? 1 : 0);
}

function updateCameraFromSnapshot(snapshot: SnapshotMessage): void {
  const [x, y, z] = snapshot.player.position;
  camera.position.set(x, y + cameraEyeHeight, z);
  camera.rotation.order = "YXZ";
  camera.rotation.y = snapshot.player.yaw;
  camera.rotation.x = snapshot.player.pitch;
}

function applyTargets(snapshot: SnapshotMessage | null, alpha: number): void {
  const previousTargets = new Map<number, TargetSnapshot>();
  snapshotBuffer.previous?.targets.forEach((target) => {
    previousTargets.set(target.id, target);
  });

  snapshot?.targets.forEach((target) => {
    const visual = targetVisuals.get(target.id);
    if (!visual) {
      return;
    }

    const previous = previousTargets.get(target.id);
    const source = previous && previous.alive === target.alive ? previous.position : target.position;
    const position = [
      THREE.MathUtils.lerp(source[0], target.position[0], alpha),
      THREE.MathUtils.lerp(source[1], target.position[1], alpha),
      THREE.MathUtils.lerp(source[2], target.position[2], alpha),
    ];

    visual.root.visible = target.alive;
    visual.root.position.set(position[0], position[1], position[2]);
    visual.glow.visible = target.alive;
  });
}

function applyProjectiles(snapshot: SnapshotMessage | null, alpha: number): void {
  const previousProjectiles = new Map<number, ProjectileSnapshot>();
  snapshotBuffer.previous?.projectiles.forEach((projectile) => {
    previousProjectiles.set(projectile.id, projectile);
  });

  const activeIds = new Set<number>();
  snapshot?.projectiles.forEach((projectile) => {
    activeIds.add(projectile.id);

    let mesh = projectileVisuals.get(projectile.id);
    if (!mesh) {
      mesh = new THREE.Mesh(
        new THREE.SphereGeometry(0.12, 12, 12),
        new THREE.MeshStandardMaterial({
          color: 0xffd18a,
          emissive: 0xf28345,
          emissiveIntensity: 1.1,
          roughness: 0.18,
          metalness: 0.05,
        }),
      );
      mesh.castShadow = true;
      projectileGroup.add(mesh);
      projectileVisuals.set(projectile.id, mesh);
    }

    const previous = previousProjectiles.get(projectile.id);
    const source = previous?.position ?? projectile.position;
    mesh.position.set(
      THREE.MathUtils.lerp(source[0], projectile.position[0], alpha),
      THREE.MathUtils.lerp(source[1], projectile.position[1], alpha),
      THREE.MathUtils.lerp(source[2], projectile.position[2], alpha),
    );
    mesh.visible = true;
  });

  for (const [id, mesh] of projectileVisuals) {
    if (activeIds.has(id)) {
      continue;
    }

    projectileGroup.remove(mesh);
    mesh.geometry.dispose();
    (mesh.material as THREE.Material).dispose();
    projectileVisuals.delete(id);
  }
}

function sendInput(frameDt: number): void {
  if (!welcome || gameOver) {
    send({ type: "input", sequence: ++input.sequence, moveX: 0, moveZ: 0, jumpPressed: false, jumpHeld: false, firePressed: false, yaw: input.yaw, pitch: input.pitch, frameDt });
    return;
  }

  const moveX = axis(input.left, input.right);
  const moveZ = axis(input.backward, input.forward);
  const message: ClientMessage = {
    type: "input",
    sequence: ++input.sequence,
    moveX,
    moveZ,
    jumpPressed: input.jumpQueued,
    jumpHeld: input.jumpHeld,
    firePressed: input.fireQueued,
    yaw: input.yaw,
    pitch: input.pitch,
    frameDt,
  };

  send(message);
  input.jumpQueued = false;
  input.fireQueued = false;
}

function animate(): void {
  const frameDt = Math.min(clock.getDelta(), 0.05);
  sendInput(frameDt);

  if (snapshotBuffer.current) {
    const alpha = THREE.MathUtils.clamp(
      (performance.now() - snapshotBuffer.receivedAt) / SNAPSHOT_INTERVAL_MS,
      0,
      1,
    );
    updateCameraFromSnapshot(snapshotBuffer.current);
    applyTargets(snapshotBuffer.current, alpha);
    applyProjectiles(snapshotBuffer.current, alpha);
  }

  muzzleFlash.intensity = performance.now() < muzzleUntil ? 4.5 : 0;
  rifle.position.z = muzzleFlash.intensity > 0 ? -0.08 : 0;

  renderer.render(scene, camera);
  requestAnimationFrame(animate);
}

function handleResize(): void {
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
}

window.addEventListener("resize", handleResize);

window.addEventListener("keydown", (event) => {
  switch (event.code) {
    case "KeyW":
      input.forward = true;
      break;
    case "KeyS":
      input.backward = true;
      break;
    case "KeyA":
      input.left = true;
      break;
    case "KeyD":
      input.right = true;
      break;
    case "Space":
      if (!input.jumpHeld) {
        input.jumpQueued = true;
      }
      input.jumpHeld = true;
      event.preventDefault();
      break;
    case "KeyR":
      if (gameOver) {
        send({ type: "reset" });
        gameOver = false;
        setStatus("Resetting round...");
      }
      break;
  }
});

window.addEventListener("keyup", (event) => {
  switch (event.code) {
    case "KeyW":
      input.forward = false;
      break;
    case "KeyS":
      input.backward = false;
      break;
    case "KeyA":
      input.left = false;
      break;
    case "KeyD":
      input.right = false;
      break;
    case "Space":
      input.jumpHeld = false;
      break;
  }
});

renderer.domElement.addEventListener("click", async () => {
  if (document.pointerLockElement !== renderer.domElement) {
    await renderer.domElement.requestPointerLock();
  }
});

window.addEventListener("mousedown", (event) => {
  if (event.button === 0 && document.pointerLockElement === renderer.domElement) {
    input.fireQueued = true;
    muzzleUntil = performance.now() + 40;
  }
});

window.addEventListener("mousemove", (event) => {
  if (document.pointerLockElement !== renderer.domElement) {
    return;
  }

  input.yaw -= event.movementX * 0.0022;
  input.pitch = THREE.MathUtils.clamp(input.pitch - event.movementY * 0.0018, -1.2, 1.2);
});

handleResize();
connect();
animate();
