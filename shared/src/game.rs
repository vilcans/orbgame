//! Main game logic.
//! Based on https://github.com/ErnWong/crystalorb/blob/master/examples/demo/src/lib.rs

use bevy::prelude::{debug, info};
use crystalorb::{
    command::Command,
    fixed_timestepper::Stepper,
    world::{DisplayState, World},
};
use rapier2d::{na::Vector2, prelude::*};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
    iter::FromIterator,
};
use wasm_bindgen::prelude::*;

use crate::TIMESTEP;

const GRAVITY: Vector2<Real> = Vector2::new(0.0, -9.81 * 30.0);

pub struct GameWorld {
    pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    joints: JointSet,
    ccd_solver: CCDSolver,
    players: HashMap<PlayerId, Player>,
}

pub struct Player {
    body_handle: RigidBodyHandle,
    _collider_handle: ColliderHandle,
    input: PlayerInput,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GameCommand {
    SpawnPlayer { client_handle: u32 },
    Input(PlayerId, PlayerCommand, bool),
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq)]
pub struct PlayerInput {
    pub jump: bool,
    pub left: bool,
    pub right: bool,
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u8);

impl Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{}", self.0)
    }
}

impl PlayerId {
    pub fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum PlayerCommand {
    Jump,
    Left,
    Right,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameSnapshot {
    players: Vec<(PlayerId, PlayerSnapshot)>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerSnapshot {
    position: Isometry<Real>,
    linvel: Vector2<Real>,
    angvel: Real,
    input: PlayerInput,
}

#[derive(Clone, Debug)]
pub struct GameDisplayState {
    pub player_positions: HashMap<PlayerId, Isometry<Real>>,
}

impl Default for GameWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl GameWorld {
    pub fn new() -> Self {
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();

        // Walls and floor
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(1.0, 100.0).restitution(0.5).build(),
            bodies.insert(
                RigidBodyBuilder::new_static()
                    .translation(vector![0.0, 0.0])
                    .ccd_enabled(true)
                    .build(),
            ),
            &mut bodies,
        );
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(1.0, 100.0).restitution(0.5).build(),
            bodies.insert(
                RigidBodyBuilder::new_static()
                    .translation(vector![180.0, 0.0])
                    .ccd_enabled(true)
                    .build(),
            ),
            &mut bodies,
        );
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(180.0, 1.0).restitution(0.5).build(),
            bodies.insert(
                RigidBodyBuilder::new_static()
                    .translation(vector![0.0, 0.0])
                    .ccd_enabled(true)
                    .build(),
            ),
            &mut bodies,
        );
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(180.0, 1.0).restitution(0.5).build(),
            bodies.insert(
                RigidBodyBuilder::new_static()
                    .translation(vector![0.0, 100.0])
                    .ccd_enabled(true)
                    .build(),
            ),
            &mut bodies,
        );

        Self {
            pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies,
            colliders,
            joints: JointSet::new(),
            ccd_solver: CCDSolver::new(),
            players: HashMap::new(),
        }
    }

    /// Create a new Player object, insert it into physics world and the [`GameWorld::players`] map.
    fn create_player(&mut self, player_id: PlayerId) {
        let body_handle = self.bodies.insert(
            RigidBodyBuilder::new_dynamic()
                .translation(vector![10.0, 80.0])
                .ccd_enabled(true)
                .build(),
        );
        let collider_handle = self.colliders.insert_with_parent(
            ColliderBuilder::ball(10.0)
                .density(0.1)
                .restitution(0.5)
                .build(),
            body_handle,
            &mut self.bodies,
        );
        let player = Player {
            body_handle,
            _collider_handle: collider_handle,
            input: Default::default(),
        };
        self.players.insert(player_id, player);
    }

    /// Remove a player from the physics world and from [`GameWorld::players`].
    fn remove_player(&mut self, player_id: PlayerId) {
        if let Some(player) = self.players.remove(&player_id) {
            self.bodies.remove(
                player.body_handle,
                &mut self.island_manager,
                &mut self.colliders,
                &mut self.joints,
            );
        }
    }
}

impl World for GameWorld {
    type CommandType = GameCommand;
    type SnapshotType = GameSnapshot;
    type DisplayStateType = GameDisplayState;

    fn command_is_valid(command: &Self::CommandType, client_id: usize) -> bool {
        match command {
            GameCommand::SpawnPlayer { .. } => {
                info!("AssignPlayer not allowed on client {}", client_id);
                false
            }
            GameCommand::Input(player_id, _, _) => player_id.as_usize() == client_id,
        }
    }

    fn apply_command(&mut self, command: &Self::CommandType) {
        match command {
            GameCommand::SpawnPlayer { client_handle } => {
                let player_id = self
                    .players
                    .iter()
                    .max_by_key(|(id, _)| id.0)
                    .map(|(n, _)| PlayerId(n.0 + 1))
                    .unwrap_or(PlayerId(0u8));
                info!(
                    "Assigning player id {} to client {}",
                    player_id, client_handle
                );
                self.create_player(player_id);
            }
            GameCommand::Input(player_id, command, value) => {
                let player_input = &mut self.players.get_mut(player_id).unwrap().input;
                match command {
                    PlayerCommand::Jump => player_input.jump = *value,
                    PlayerCommand::Left => player_input.left = *value,
                    PlayerCommand::Right => player_input.right = *value,
                }
            }
        }
    }

    fn apply_snapshot(&mut self, snapshot: Self::SnapshotType) {
        let snapshot_players =
            HashSet::<PlayerId>::from_iter(snapshot.players.iter().map(|(n, _)| *n));
        let current_players = HashSet::from_iter(self.players.keys().copied());

        // Create objects for all players in the snapshot which are not already in the game world
        for player_id in snapshot_players.difference(&current_players) {
            debug!("Creating player {} from snapshot", player_id);
            self.create_player(*player_id);
        }

        // Remove objects for all players that are in the game world but not in the snapshot
        for player_id in current_players.difference(&snapshot_players) {
            debug!("Removing player {} not in snapshot", player_id);
            self.remove_player(*player_id);
        }

        // Update players
        for (player_id, player_snapshot) in snapshot.players.iter() {
            let player = self.players.get_mut(player_id).unwrap();
            let body = self.bodies.get_mut(player.body_handle).unwrap();
            body.set_position(player_snapshot.position, true);
            body.set_linvel(player_snapshot.linvel, true);
            body.set_angvel(player_snapshot.angvel, true);
            player.input = player_snapshot.input;
        }
    }

    fn snapshot(&self) -> Self::SnapshotType {
        let players = self
            .players
            .iter()
            .map(|(player_id, player)| {
                let body = self.bodies.get(player.body_handle).unwrap();
                (
                    *player_id,
                    PlayerSnapshot {
                        position: *body.position(),
                        linvel: *body.linvel(),
                        angvel: body.angvel(),
                        input: player.input,
                    },
                )
            })
            .collect();
        GameSnapshot { players }
    }

    fn display_state(&self) -> Self::DisplayStateType {
        let player_positions = self
            .players
            .iter()
            .map(|(player_id, player)| {
                (
                    *player_id,
                    *self.bodies.get(player.body_handle).unwrap().position(),
                )
            })
            .collect();
        GameDisplayState { player_positions }
    }
}

impl Stepper for GameWorld {
    fn step(&mut self) {
        for player in &mut self.players.values_mut() {
            let body = self.bodies.get_mut(player.body_handle).unwrap();
            body.apply_force(
                Vector2::new(
                    ((player.input.right as i32) - (player.input.left as i32)) as f32 * 4000.0,
                    0.0,
                ),
                true,
            );
            if player.input.jump {
                body.apply_impulse(Vector2::new(0.0, 4000.0), true);
                player.input.jump = false;
            }
        }
        self.pipeline.step(
            &GRAVITY,
            &IntegrationParameters {
                dt: TIMESTEP as f32,
                ..Default::default()
            },
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.joints,
            &mut self.ccd_solver,
            &(),
            &(),
        );
    }
}

impl Command for GameCommand {}

impl DisplayState for GameDisplayState {
    fn from_interpolation(state1: &Self, state2: &Self, t: f64) -> Self {
        // Use all players from state1. If there is a player in state2 but not in state1, it will not be included.
        let mut interpolated_positions = state1.player_positions.clone();
        for (player_id, p2) in state2.player_positions.iter() {
            interpolated_positions.get_mut(&player_id).map(|p1| {
                // Update in place
                *p1 = p1.lerp_slerp(&p2, t as f32);
            });
        }
        GameDisplayState {
            player_positions: interpolated_positions,
        }
    }
}
