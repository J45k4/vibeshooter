use nalgebra::vector;
use rapier3d::prelude::*;

pub struct GameWorld {
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    integration_parameters: IntegrationParameters,
    island_manager: IslandManager,
    broad_phase: BroadPhaseMultiSap,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    physics_pipeline: PhysicsPipeline,
    query_pipeline: QueryPipeline,
    gravity: Vector<Real>,
    cube_handle: RigidBodyHandle,
}

impl GameWorld {
    pub fn new(fixed_dt: f32) -> Self {
        let mut rigid_body_set = RigidBodySet::new();
        let mut collider_set = ColliderSet::new();

        let ground_body = RigidBodyBuilder::fixed().build();
        let ground_handle = rigid_body_set.insert(ground_body);
        let ground_collider = ColliderBuilder::cuboid(50.0, 0.5, 50.0)
            .translation(vector![0.0, -0.5, 0.0])
            .build();
        collider_set.insert_with_parent(ground_collider, ground_handle, &mut rigid_body_set);

        let cube_body = RigidBodyBuilder::dynamic()
            .translation(vector![0.0, 5.0, 0.0])
            .build();
        let cube_handle = rigid_body_set.insert(cube_body);
        let cube_collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
            .restitution(0.2)
            .build();
        collider_set.insert_with_parent(cube_collider, cube_handle, &mut rigid_body_set);

        let integration_parameters = IntegrationParameters {
            dt: fixed_dt,
            ..IntegrationParameters::default()
        };

        Self {
            rigid_body_set,
            collider_set,
            integration_parameters,
            island_manager: IslandManager::new(),
            broad_phase: BroadPhaseMultiSap::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            physics_pipeline: PhysicsPipeline::new(),
            query_pipeline: QueryPipeline::new(),
            gravity: vector![0.0, -9.81, 0.0],
            cube_handle,
        }
    }

    pub fn step(&mut self) {
        // Authoritative server simulation will run here.
        // Networking can later send compact snapshots/deltas derived from this state.
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
    }

    pub fn cube_position(&self) -> Option<Vector<Real>> {
        self.rigid_body_set
            .get(self.cube_handle)
            .map(|body| *body.translation())
    }
}
