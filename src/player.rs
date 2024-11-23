use avian2d::prelude::*;
use bevy::prelude::*;

use crate::{KeepUpright, MyWorldCoords};

#[derive(Component, Default)]
pub(crate) struct Player {
    is_grounded: bool,
}

#[derive(Component, Default)]
pub(crate) struct Bomb {}

pub(crate) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let ball_r = 50.;
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("bevy_pixel_dark.png"),
            transform: Transform::from_xyz(100., 100., 0.),
            sprite: Sprite {
                custom_size: Some(Vec2::new(ball_r * 2., ball_r * 2.)),
                ..default()
            },
            ..default()
        },
        Player::default(),
        RigidBody::Dynamic,
        Collider::circle(ball_r),
        LockedAxes::ROTATION_LOCKED,
        Friction::new(0.).with_combine_rule(CoefficientCombine::Multiply),
        KeepUpright::default(),
    ));
}

pub(crate) fn is_grounded(
    mut players: Query<(&Transform, &mut Player)>,
    mut collisions: EventReader<Collision>,
) {
    for (_, mut player) in &mut players {
        player.is_grounded = false;
    }

    for Collision(contacts) in collisions.read() {
        assert!(contacts.manifolds.len() == 1);
        let contact = contacts.manifolds.first().unwrap();

        if let Ok(mut ent) = players.get_mut(contacts.entity1) {
            let normal = -contact.global_normal1(&Rotation::from(ent.0.rotation));
            ent.1.is_grounded |= normal.dot(Vec2::Y) > 0.5;
        } else if let Ok(mut ent) = players.get_mut(contacts.entity2) {
            let normal = -contact.global_normal2(&Rotation::from(ent.0.rotation));
            ent.1.is_grounded |= normal.dot(Vec2::Y) > 0.5;
        }
    }
}

pub(crate) fn movement(
    mut player: Query<(&mut Transform, &mut Friction, &mut LinearVelocity, &Player)>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let (_, mut friction, mut velocity, player) = match player.iter_mut().next() {
        Some(x) => x,
        None => return,
    };

    // Keyboard input
    let mut input = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        input -= Vec2::X;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        input += Vec2::X;
    }

    // Jump
    // TODO: Detect ground
    if keys.just_pressed(KeyCode::Space) && player.is_grounded {
        **velocity += Vec2::Y * 600.0;
    }

    // Slide
    // TODO: put on timer
    if keys.pressed(KeyCode::ShiftLeft) {
        friction.static_coefficient = 0.;
        friction.dynamic_coefficient = 0.;
    } else {
        friction.static_coefficient = 1.;
        friction.dynamic_coefficient = 1.;
    }

    let accel = 100.0;

    let delta_v = input * accel;

    let max_speed = 1000.0;

    if input.dot(**velocity) < 0.0 {
        // slow down
        **velocity += delta_v;
    } else if velocity.x.abs() < max_speed {
        **velocity += delta_v;
        velocity.x = velocity.x.clamp(-max_speed, max_speed);
    }
}

pub(crate) fn hook(
    mut player: Query<(Entity, &Transform), With<Player>>,
    mouse: Res<ButtonInput<MouseButton>>,
    coords: Res<MyWorldCoords>,
    spatial_query: SpatialQuery,
    mut current: Local<Option<(Entity, Entity)>>,
    mut commands: Commands,
) {
    let (player, transform) = match player.iter_mut().next() {
        Some(x) => x,
        None => return,
    };

    match (*current, mouse.pressed(MouseButton::Right)) {
        (None, true) => {
            let coords = coords.0;
            let pos = Vec2::new(transform.translation.x, transform.translation.y);

            let dir = (coords - pos).normalize();

            let filter = SpatialQueryFilter::default().with_excluded_entities([player]);

            if let Some(hit) =
                spatial_query.cast_ray(pos, Dir2::try_from(dir).unwrap(), 5000.0, true, filter)
            {
                let hit_point = pos + (dir * hit.time_of_impact);

                let hook = commands
                    .spawn((
                        RigidBody::Static,
                        Position::from_xy(hit_point.x, hit_point.y),
                    ))
                    .id();

                let rope = commands
                    .spawn(DistanceJoint::new(player, hook).with_rest_length(hit.time_of_impact))
                    .id();

                *current = Some((hook, rope));
            }
        }
        (Some((hook, rope)), false) => {
            // despawn
            commands.entity(rope).despawn();
            commands.entity(hook).despawn();
            *current = None;
        }
        _ => (),
    }
}

// pub(crate) fn bomb(
//     mut player: Query<(Entity, &Transform), With<Player>>,
//     mut bombs: Query<With<Bomb>>,
//     mouse: Res<Input<MouseButton>>,
//     coords: Res<MyWorldCoords>,
//     mut commands: Commands,
// ) {
// }
