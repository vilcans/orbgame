use crystalorb_bevy_networking_turbulence::{
    bevy_networking_turbulence::{
        MessageChannelMode, MessageChannelSettings, NetworkResource, ReliableChannelSettings,
    },
    crystalorb::client::{stage::Stage as ClientStage, stage::StageMut as ClientStageMut, Client},
    CommandChannelSettings, CrystalOrbClientPlugin, WrappedNetworkResource,
};
use orbgame_shared::{
    bevy,
    bevy::prelude::*,
    crystalorb_bevy_networking_turbulence::{self, bevy_networking_turbulence, crystalorb},
    game::{GameCommand, GameWorld, PlayerCommand, PlayerId, PlayerInput},
};
use std::{collections::HashSet, iter::FromIterator, net::SocketAddr, time::Duration};

const PLAYER_COLORS: [Color; 5] = [
    Color::rgb(
        0xff as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x44 as f32 / 255.0,
    ),
    Color::rgb(
        0xda as f32 / 255.0,
        0xff as f32 / 255.0,
        0x60 as f32 / 255.0,
    ),
    Color::rgb(
        0xff as f32 / 255.0,
        0x54 as f32 / 255.0,
        0xc3 as f32 / 255.0,
    ),
    Color::rgb(
        0x21 as f32 / 255.0,
        0xff as f32 / 255.0,
        0x73 as f32 / 255.0,
    ),
    Color::rgb(
        0x99 as f32 / 255.0,
        0x2d as f32 / 255.0,
        0xff as f32 / 255.0,
    ),
];

fn player_input(
    mut state: Local<PlayerInput>,
    input: Res<Input<KeyCode>>,
    mut client: ResMut<Client<GameWorld>>,
    mut net: ResMut<NetworkResource>,
) {
    if let ClientStageMut::Ready(mut ready_client) = client.stage_mut() {
        let player_id = PlayerId(ready_client.client_id() as u8);

        let player_input = &PlayerInput {
            jump: input.pressed(KeyCode::Up),
            left: input.pressed(KeyCode::Left),
            right: input.pressed(KeyCode::Right),
        };

        if player_input.jump != state.jump {
            ready_client.issue_command(
                GameCommand::Input(player_id, PlayerCommand::Jump, player_input.jump),
                &mut WrappedNetworkResource(&mut *net),
            );
        }
        if player_input.left != state.left {
            ready_client.issue_command(
                GameCommand::Input(player_id, PlayerCommand::Left, player_input.left),
                &mut WrappedNetworkResource(&mut *net),
            );
        }
        if player_input.right != state.right {
            ready_client.issue_command(
                GameCommand::Input(player_id, PlayerCommand::Right, player_input.right),
                &mut WrappedNetworkResource(&mut *net),
            );
        }
        *state = *player_input;
    }
}

fn main() {
    App::build()
        // You can optionally override some message channel settings
        // There is `CommandChannelSettings`, `SnapshotChannelSettings`, and `ClockSyncChannelSettings`
        // Make sure you apply the same settings for both client and server.
        .insert_resource(CommandChannelSettings(MessageChannelSettings {
            channel: 0,
            channel_mode: MessageChannelMode::Compressed {
                reliability_settings: ReliableChannelSettings {
                    bandwidth: 4096,
                    recv_window_size: 1024,
                    send_window_size: 1024,
                    burst_bandwidth: 1024,
                    init_send: 512,
                    wakeup_time: Duration::from_millis(100),
                    initial_rtt: Duration::from_millis(200),
                    max_rtt: Duration::from_secs(2),
                    rtt_update_factor: 0.1,
                    rtt_resend_factor: 1.5,
                },
                max_chunk_len: 1024,
            },
            message_buffer_size: 64,
            packet_buffer_size: 64,
        }))
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_scene.system())
        .add_plugin(CrystalOrbClientPlugin::<GameWorld>::new(
            orbgame_shared::crystal_orb_config(),
        ))
        .add_startup_system(setup_network.system())
        .add_system(player_input.system())
        //.add_system(print_transforms.system())
        .add_system(bevy::input::system::exit_on_esc_system.system())
        .add_system(show_state.system())
        .add_system(player_view_lifecycle.system())
        .add_system(view.system())
        .run();
}

struct Player(PlayerId);

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // light
    commands.spawn_bundle(LightBundle {
        light: Light {
            intensity: 10000.0,
            range: 200.0,
            ..Default::default()
        },
        transform: Transform::from_xyz(90.0, 110.0, 0.0)
            .looking_at(Vec3::new(90.0, 0.0, 0.0), Vec3::Y),
        ..Default::default()
    });
    // camera
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(90.0, 70.0, 180.0)
            .looking_at(Vec3::new(90.0, 50.0, 0.0), Vec3::Y),
        ..Default::default()
    });
    // floor
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 180.0 })),
        material: materials.add(Color::rgb(0.3, 0.4, 0.5).into()),
        transform: Transform::from_xyz(90.0, 0.0, 0.0),
        ..Default::default()
    });
}

#[allow(dead_code)]
fn print_transforms(transforms: Query<(Entity, &Transform)>) {
    for (entity, t) in transforms.iter() {
        println!("Entity {:?} Transform: {:?}", entity, t);
    }
}

fn setup_network(mut net: ResMut<NetworkResource>) {
    let ip_address =
        bevy_networking_turbulence::find_my_ip_address().expect("can't find ip address");
    let socket_address = SocketAddr::new(ip_address, orbgame_shared::SERVER_PORT);
    info!("Connecting to {}", socket_address);
    net.connect(socket_address);
}

fn show_state(mut previous: Local<String>, client: ResMut<Client<GameWorld>>) {
    use crystalorb::client::stage::Stage;
    let text = match client.stage() {
        Stage::SyncingClock(c) => {
            format!("SyncingClock {}/{}", c.sample_count(), c.samples_needed())
        }
        Stage::SyncingInitialState(_) => "SyncingInitialState".to_string(),
        Stage::Ready(_) => "Ready".to_string(),
    };
    if *previous != text {
        info!("State: {}", text);
        *previous = text;
    }
}

/// Make sure we have views for all players, and no views for nonexistant players.
fn player_view_lifecycle(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    client: Res<Client<GameWorld>>,
    players: Query<&Player>,
) {
    match client.stage() {
        ClientStage::Ready(client) => {
            let display_state = client.display_state();

            let current_views = HashSet::<PlayerId>::from_iter(players.iter().map(|p| p.0));
            let player_ids = HashSet::from_iter(display_state.player_positions.keys().copied());

            for player_id in current_views.difference(&player_ids) {
                warn!("Should remove player {} - not implemented", player_id);
                //commands.remove_resource()
            }
            for player_id in player_ids.difference(&current_views) {
                info!("Creating view for player {}", player_id);
                commands
                    .spawn_bundle(PbrBundle {
                        mesh: meshes.add(Mesh::from(shape::Icosphere {
                            radius: 10.0,
                            subdivisions: 3,
                        })),
                        material: materials
                            .add(PLAYER_COLORS[player_id.as_usize() % PLAYER_COLORS.len()].into()),
                        transform: Transform::from_xyz(0.0, 0.5, 0.0),
                        ..Default::default()
                    })
                    .insert(Player(*player_id));
            }
        }
        _ => (),
    }
}

fn view(client: Res<Client<GameWorld>>, mut query: Query<(&Player, &mut Transform)>) {
    if let ClientStage::Ready(client) = client.stage() {
        let display_state = client.display_state();
        for (player, mut transform) in query.iter_mut() {
            let pos = display_state.player_positions[&player.0];
            transform.translation =
                Vec3::new(pos.translation.vector.x, pos.translation.vector.y, 0.0);
            transform.rotation = Quat::from_rotation_z(pos.rotation.angle());
        }
    }
}
