use nalgebra::{point, vector};
use rapier3d::{
    control::{CharacterAutostep, CharacterLength, KinematicCharacterController},
    prelude::*,
};

use crate::protocol::{HitEvent, PlayerSnapshot, SnapshotMessage, TargetSnapshot};

const ARENA_HALF_EXTENT: f32 = 20.0;
const WALL_HALF_HEIGHT: f32 = 2.5;
const WALL_THICKNESS: f32 = 0.5;
const PLAYER_RADIUS: f32 = 0.35;
const PLAYER_HALF_SEGMENT: f32 = 0.55;
const PLAYER_SPEED: f32 = 8.0;
const JUMP_SPEED: f32 = 5.2;
const GRAVITY: f32 = -18.0;
const FIRE_RANGE: f32 = 80.0;
const RESPAWN_SECONDS: f32 = 1.5;
const OUT_OF_BOUNDS_Y: f32 = -8.0;
const PLAYER_START: Vector<Real> = vector![0.0, 1.6, 12.0];
const PLAYER_EYE_HEIGHT: f32 = 0.9;

pub struct GameWorld {
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    query_pipeline: QueryPipeline,
    character_controller: KinematicCharacterController,
    player: PlayerState,
    targets: Vec<TargetState>,
    score: u32,
    game_over: bool,
    fixed_dt: f32,
}

struct PlayerState {
    body: RigidBodyHandle,
    collider: ColliderHandle,
    yaw: f32,
    pitch: f32,
    vertical_velocity: f32,
    on_ground: bool,
}

struct TargetState {
    id: u32,
    body: RigidBodyHandle,
    collider: ColliderHandle,
    base_position: Vector<Real>,
    motion_axis: Vector<Real>,
    motion_amplitude: f32,
    motion_speed: f32,
    phase: f32,
    alive: bool,
    respawn_timer: f32,
}

#[derive(Debug, Clone, Default)]
pub struct StepOutcome {
    pub recent_hits: Vec<HitEvent>,
    pub game_over_just_triggered: bool,
}

impl GameWorld {
    pub fn new(fixed_dt: f32) -> Self {
        let mut world = Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            query_pipeline: QueryPipeline::new(),
            character_controller: KinematicCharacterController {
                offset: CharacterLength::Absolute(0.02),
                slide: true,
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Absolute(0.5),
                    min_width: CharacterLength::Absolute(0.3),
                    include_dynamic_bodies: false,
                }),
                snap_to_ground: Some(CharacterLength::Absolute(0.2)),
                ..KinematicCharacterController::default()
            },
            player: PlayerState {
                body: RigidBodyHandle::invalid(),
                collider: ColliderHandle::invalid(),
                yaw: 0.0,
                pitch: 0.0,
                vertical_velocity: 0.0,
                on_ground: false,
            },
            targets: Vec::new(),
            score: 0,
            game_over: false,
            fixed_dt,
        };

        world.build_arena();
        world.spawn_player();
        world.spawn_targets();
        world.sync_queries();

        world
    }

    pub fn eye_height() -> f32 {
        PLAYER_EYE_HEIGHT
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn step(&mut self, tick: u64, input: &crate::protocol::ClientInput) -> StepOutcome {
        self.update_targets(tick);

        if self.game_over {
            return StepOutcome::default();
        }

        self.player.yaw = input.yaw;
        self.player.pitch = input.pitch.clamp(-1.2, 1.2);

        if input.jump_pressed && self.player.on_ground {
            self.player.vertical_velocity = JUMP_SPEED;
            self.player.on_ground = false;
        }

        self.player.vertical_velocity += GRAVITY * self.fixed_dt;

        let movement = self.desired_movement(input.move_x, input.move_z);
        let desired_translation =
            vector![movement.x, self.player.vertical_velocity * self.fixed_dt, movement.z];

        let body = &self.rigid_body_set[self.player.body];
        let collider = &self.collider_set[self.player.collider];
        let effective = self.character_controller.move_shape(
            self.fixed_dt,
            &self.rigid_body_set,
            &self.collider_set,
            &self.query_pipeline,
            collider.shape(),
            body.position(),
            desired_translation,
            QueryFilter {
                flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_rigid_body: Some(self.player.body),
                ..QueryFilter::default()
            },
            |_| {},
        );

        let new_translation = *body.translation() + effective.translation;
        if new_translation.y < OUT_OF_BOUNDS_Y {
            self.game_over = true;
            return StepOutcome {
                recent_hits: Vec::new(),
                game_over_just_triggered: true,
            };
        }

        if effective.grounded && self.player.vertical_velocity < 0.0 {
            self.player.vertical_velocity = 0.0;
        } else if effective.translation.y < desired_translation.y && self.player.vertical_velocity > 0.0
        {
            self.player.vertical_velocity = 0.0;
        }

        self.player.on_ground = effective.grounded;
        self.rigid_body_set[self.player.body].set_position(
            Isometry::translation(new_translation.x, new_translation.y, new_translation.z),
            true,
        );
        self.sync_queries();

        let recent_hits = if input.fire_pressed {
            self.fire_hitscan()
        } else {
            Vec::new()
        };

        StepOutcome {
            recent_hits,
            game_over_just_triggered: false,
        }
    }

    pub fn snapshot(
        &self,
        tick: u64,
        last_processed_input: u64,
        recent_hits: Vec<HitEvent>,
    ) -> SnapshotMessage {
        let position = self.player_position();
        SnapshotMessage {
            tick,
            last_processed_input,
            player: PlayerSnapshot {
                position: [position.x, position.y, position.z],
                velocity: [0.0, self.player.vertical_velocity, 0.0],
                on_ground: self.player.on_ground,
                yaw: self.player.yaw,
                pitch: self.player.pitch,
            },
            targets: self
                .targets
                .iter()
                .map(|target| {
                    let position = self.rigid_body_set[target.body].translation();
                    TargetSnapshot {
                        id: target.id,
                        position: [position.x, position.y, position.z],
                        alive: target.alive,
                    }
                })
                .collect(),
            score: self.score,
            recent_hits,
            game_over: self.game_over,
        }
    }

    pub fn reset_after_game_over(&mut self) {
        self.game_over = false;
        self.player.vertical_velocity = 0.0;
        self.player.on_ground = false;
        self.rigid_body_set[self.player.body].set_position(
            Isometry::translation(PLAYER_START.x, PLAYER_START.y, PLAYER_START.z),
            true,
        );
        self.sync_queries();
    }

    pub fn full_reset(&mut self) {
        self.score = 0;
        self.game_over = false;
        self.player.yaw = 0.0;
        self.player.pitch = 0.0;
        self.player.vertical_velocity = 0.0;
        self.player.on_ground = false;
        self.rigid_body_set[self.player.body].set_position(
            Isometry::translation(PLAYER_START.x, PLAYER_START.y, PLAYER_START.z),
            true,
        );

        for target in &mut self.targets {
            target.alive = true;
            target.respawn_timer = 0.0;
            let pos = target.base_position;
            self.rigid_body_set[target.body]
                .set_position(Isometry::translation(pos.x, pos.y, pos.z), true);
        }

        self.sync_queries();
    }

    fn build_arena(&mut self) {
        self.add_fixed_box(vector![0.0, -0.5, 0.0], vector![ARENA_HALF_EXTENT, 0.5, ARENA_HALF_EXTENT]);
        self.add_fixed_box(
            vector![0.0, WALL_HALF_HEIGHT, -ARENA_HALF_EXTENT],
            vector![ARENA_HALF_EXTENT, WALL_HALF_HEIGHT, WALL_THICKNESS],
        );
        self.add_fixed_box(
            vector![0.0, WALL_HALF_HEIGHT, ARENA_HALF_EXTENT],
            vector![ARENA_HALF_EXTENT, WALL_HALF_HEIGHT, WALL_THICKNESS],
        );
        self.add_fixed_box(
            vector![ARENA_HALF_EXTENT, WALL_HALF_HEIGHT, 0.0],
            vector![WALL_THICKNESS, WALL_HALF_HEIGHT, ARENA_HALF_EXTENT],
        );
        self.add_fixed_box(
            vector![-ARENA_HALF_EXTENT, WALL_HALF_HEIGHT, 0.0],
            vector![WALL_THICKNESS, WALL_HALF_HEIGHT, ARENA_HALF_EXTENT],
        );

        self.add_fixed_box(vector![0.0, 1.5, -7.5], vector![1.5, 1.5, 1.0]);
        self.add_fixed_box(vector![-6.0, 1.0, 1.0], vector![1.0, 1.0, 1.0]);
        self.add_fixed_box(vector![6.0, 1.25, -1.5], vector![1.25, 1.25, 1.25]);
    }

    fn spawn_player(&mut self) {
        let body = RigidBodyBuilder::kinematic_position_based()
            .translation(PLAYER_START)
            .enabled_rotations(false, false, false)
            .build();
        let body_handle = self.rigid_body_set.insert(body);
        let collider = ColliderBuilder::capsule_y(PLAYER_HALF_SEGMENT, PLAYER_RADIUS).build();
        let collider_handle =
            self.collider_set
                .insert_with_parent(collider, body_handle, &mut self.rigid_body_set);

        self.player.body = body_handle;
        self.player.collider = collider_handle;
    }

    fn spawn_targets(&mut self) {
        let definitions = [
            (1, vector![-8.0, 1.2, -12.0], vector![0.0, 0.0, 0.0], 0.0, 0.0, 0.0),
            (2, vector![0.0, 1.2, -12.5], vector![0.0, 0.0, 0.0], 0.0, 0.0, 0.0),
            (3, vector![8.0, 1.2, -12.0], vector![0.0, 0.0, 0.0], 0.0, 0.0, 0.0),
            (4, vector![-4.0, 1.2, -17.0], vector![1.0, 0.0, 0.0], 2.5, 1.5, 0.5),
            (5, vector![4.0, 1.2, -17.0], vector![1.0, 0.0, 0.0], 2.5, 1.5, 2.0),
        ];

        for (id, base_position, motion_axis, motion_amplitude, motion_speed, phase) in definitions {
            let body = RigidBodyBuilder::kinematic_position_based()
                .translation(base_position)
                .build();
            let body_handle = self.rigid_body_set.insert(body);
            let collider = ColliderBuilder::cuboid(0.45, 0.9, 0.35).build();
            let collider_handle =
                self.collider_set
                    .insert_with_parent(collider, body_handle, &mut self.rigid_body_set);

            self.targets.push(TargetState {
                id,
                body: body_handle,
                collider: collider_handle,
                base_position,
                motion_axis,
                motion_amplitude,
                motion_speed,
                phase,
                alive: true,
                respawn_timer: 0.0,
            });
        }
    }

    fn update_targets(&mut self, tick: u64) {
        let t = tick as f32 * self.fixed_dt;

        for target in &mut self.targets {
            if !target.alive {
                target.respawn_timer -= self.fixed_dt;
                if target.respawn_timer <= 0.0 {
                    target.alive = true;
                }
            }

            let position = if target.alive {
                target.base_position
                    + target.motion_axis
                        * (target.motion_amplitude * (t * target.motion_speed + target.phase).sin())
            } else {
                vector![target.base_position.x, -50.0, target.base_position.z]
            };

            self.rigid_body_set[target.body].set_position(
                Isometry::translation(position.x, position.y, position.z),
                true,
            );
        }

        self.sync_queries();
    }

    fn desired_movement(&self, move_x: f32, move_z: f32) -> Vector<Real> {
        let (sin_yaw, cos_yaw) = self.player.yaw.sin_cos();
        let forward = vector![-sin_yaw, 0.0, -cos_yaw];
        let right = vector![cos_yaw, 0.0, -sin_yaw];
        let movement = right * move_x + forward * move_z;
        let horizontal = movement.cap_magnitude(1.0) * PLAYER_SPEED * self.fixed_dt;
        vector![horizontal.x, 0.0, horizontal.z]
    }

    fn fire_hitscan(&mut self) -> Vec<HitEvent> {
        let origin = self.eye_position();
        let direction = self.look_direction();
        let ray = Ray::new(point![origin.x, origin.y, origin.z], direction);

        let hit = self.query_pipeline.cast_ray_and_get_normal(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            FIRE_RANGE,
            true,
            QueryFilter {
                flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_rigid_body: Some(self.player.body),
                ..QueryFilter::default()
            },
        );

        let Some((collider_handle, _)) = hit else {
            return Vec::new();
        };

        let Some(target_index) = self
            .targets
            .iter()
            .position(|target| target.collider == collider_handle && target.alive)
        else {
            return Vec::new();
        };

        let (target_id, body, base_position) = {
            let target = &mut self.targets[target_index];
            target.alive = false;
            target.respawn_timer = RESPAWN_SECONDS;
            (target.id, target.body, target.base_position)
        };

        self.rigid_body_set[body].set_position(
            Isometry::translation(base_position.x, -50.0, base_position.z),
            true,
        );
        self.sync_queries();

        self.score = self.score.saturating_add(1);
        vec![HitEvent {
            target_id,
            score: self.score,
        }]
    }

    fn player_position(&self) -> Vector<Real> {
        *self.rigid_body_set[self.player.body].translation()
    }

    fn eye_position(&self) -> Vector<Real> {
        let body_pos = self.player_position();
        vector![body_pos.x, body_pos.y + PLAYER_EYE_HEIGHT, body_pos.z]
    }

    fn look_direction(&self) -> Vector<Real> {
        let (sin_yaw, cos_yaw) = self.player.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.player.pitch.sin_cos();
        vector![-sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch]
    }

    fn add_fixed_box(&mut self, position: Vector<Real>, half_extents: Vector<Real>) {
        let body = RigidBodyBuilder::fixed().translation(position).build();
        let body_handle = self.rigid_body_set.insert(body);
        let collider = ColliderBuilder::cuboid(half_extents.x, half_extents.y, half_extents.z).build();
        self.collider_set
            .insert_with_parent(collider, body_handle, &mut self.rigid_body_set);
    }

    fn sync_queries(&mut self) {
        self.rigid_body_set
            .propagate_modified_body_positions_to_colliders(&mut self.collider_set);
        self.query_pipeline.update(&self.collider_set);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ClientInput;

    impl GameWorld {
        fn set_player_pose_for_test(&mut self, position: Vector<Real>, yaw: f32, pitch: f32) {
            self.player.yaw = yaw;
            self.player.pitch = pitch;
            self.rigid_body_set[self.player.body]
                .set_position(Isometry::translation(position.x, position.y, position.z), true);
            self.sync_queries();
        }

        fn target_state(&self, id: u32) -> &TargetState {
            self.targets.iter().find(|target| target.id == id).unwrap()
        }
    }

    fn aim_from_to(origin: Vector<Real>, target: Vector<Real>) -> (f32, f32) {
        let delta = target - origin;
        let yaw = (-delta.x).atan2(-delta.z);
        let horizontal = (delta.x * delta.x + delta.z * delta.z).sqrt();
        let pitch = delta.y.atan2(horizontal);
        (yaw, pitch)
    }

    #[test]
    fn movement_stops_at_arena_wall() {
        let mut world = GameWorld::new(1.0 / 60.0);
        world.set_player_pose_for_test(vector![0.0, 1.6, 17.5], std::f32::consts::PI, 0.0);

        let input = ClientInput {
            move_z: 1.0,
            yaw: std::f32::consts::PI,
            ..ClientInput::default()
        };

        for tick in 0..180 {
            world.step(tick, &input);
        }

        assert!(world.player_position().z <= 19.2);
    }

    #[test]
    fn jump_only_applies_while_grounded() {
        let mut world = GameWorld::new(1.0 / 60.0);
        world.player.on_ground = true;

        let jump = ClientInput {
            jump_pressed: true,
            ..ClientInput::default()
        };
        world.step(6, &jump);
        let velocity_after_first_jump = world.player.vertical_velocity;

        world.step(7, &jump);

        assert!(velocity_after_first_jump > 0.0);
        assert!(world.player.vertical_velocity < JUMP_SPEED);
    }

    #[test]
    fn forward_input_uses_current_yaw() {
        let mut world = GameWorld::new(1.0 / 60.0);
        world.set_player_pose_for_test(vector![0.0, 1.6, 0.0], std::f32::consts::FRAC_PI_2, 0.0);

        world.step(
            1,
            &ClientInput {
                move_z: 1.0,
                yaw: std::f32::consts::FRAC_PI_2,
                ..ClientInput::default()
            },
        );

        assert!(world.player_position().x < 0.0);
        assert!(world.player_position().z.abs() < 0.2);
    }

    #[test]
    fn hitscan_respects_visible_targets_and_cover() {
        let mut world = GameWorld::new(1.0 / 60.0);

        let visible_origin = vector![8.0, 1.6, 8.0];
        let visible_target = *world.rigid_body_set[world.target_state(3).body].translation();
        let (yaw, pitch) = aim_from_to(
            vector![visible_origin.x, visible_origin.y + PLAYER_EYE_HEIGHT, visible_origin.z],
            visible_target,
        );
        world.set_player_pose_for_test(visible_origin, yaw, pitch);
        let step = world.step(
            1,
            &ClientInput {
                fire_pressed: true,
                yaw,
                pitch,
                ..ClientInput::default()
            },
        );
        assert_eq!(step.recent_hits, vec![HitEvent { target_id: 3, score: 1 }]);
        assert!(!world.target_state(3).alive);

        let mut occluded_world = GameWorld::new(1.0 / 60.0);
        let occluded_origin = vector![0.0, 1.6, 8.0];
        let occluded_target = *occluded_world.rigid_body_set[occluded_world.target_state(2).body].translation();
        let (yaw, pitch) = aim_from_to(
            vector![occluded_origin.x, occluded_origin.y + PLAYER_EYE_HEIGHT, occluded_origin.z],
            occluded_target,
        );
        occluded_world.set_player_pose_for_test(occluded_origin, yaw, pitch);
        let step = occluded_world.step(
            1,
            &ClientInput {
                fire_pressed: true,
                yaw,
                pitch,
                ..ClientInput::default()
            },
        );
        assert!(step.recent_hits.is_empty());
        assert!(occluded_world.target_state(2).alive);
    }

    #[test]
    fn target_respawns_after_timer() {
        let mut world = GameWorld::new(1.0 / 60.0);
        let target = *world.rigid_body_set[world.target_state(1).body].translation();
        let origin = vector![-8.0, 1.6, 8.0];
        let (yaw, pitch) = aim_from_to(
            vector![origin.x, origin.y + PLAYER_EYE_HEIGHT, origin.z],
            target,
        );
        world.set_player_pose_for_test(origin, yaw, pitch);
        world.step(
            1,
            &ClientInput {
                fire_pressed: true,
                yaw,
                pitch,
                ..ClientInput::default()
            },
        );

        let steps_to_respawn = (RESPAWN_SECONDS / world.fixed_dt) as u64 + 2;
        for tick in 2..(2 + steps_to_respawn) {
            world.step(tick, &ClientInput::default());
        }

        assert!(world.target_state(1).alive);
    }

    #[test]
    fn reset_after_fall_preserves_score() {
        let mut world = GameWorld::new(1.0 / 60.0);
        world.score = 2;
        world.set_player_pose_for_test(vector![0.0, -9.0, 0.0], 0.0, 0.0);

        let outcome = world.step(1, &ClientInput::default());
        assert!(outcome.game_over_just_triggered);

        world.reset_after_game_over();
        assert_eq!(world.score(), 2);
        assert!(!world.game_over);
        assert!(world.player_position().y > 0.0);
    }
}
