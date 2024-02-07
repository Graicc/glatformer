mod player;

use std::f32::consts::PI;

use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    window::PrimaryWindow,
    asset::AssetMetaCheck
};
use bevy_xpbd_2d::{math::Vector, prelude::*};

fn main() {
    App::new()
        .insert_resource(AssetMetaCheck::Never)
        .add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: false,
                        ..default()
                    }),
                    ..default()
                }),
            PhysicsPlugins::default(),
            PhysicsDebugPlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Startup, player::setup)
        .add_systems(Update, player::movement)
        .add_systems(Update, player::hook)
        .add_systems(Update, player::is_grounded)
        .add_systems(Update, debug)
        .add_systems(Update, pan_camera)
        .add_systems(Update, zoom_camera)
        .add_systems(Update, keep_upright)
        .add_systems(Update, world_cursor)
        .insert_resource(SubstepCount(50))
        .insert_resource(Gravity(Vector::NEG_Y * 1000.0))
        .insert_resource(PhysicsDebugConfig {
            contact_color: Some(Color::default()),
            ..default()
        })
        .run();
}

fn make_cube(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    hue: f32,
) -> (SpriteBundle, RigidBody, Collider, Friction, Restitution) {
    (
        SpriteBundle {
            sprite: Sprite {
                color: Color::hsl(hue, 0.8, 0.4),
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            },
            transform: Transform::from_xyz(x, y, 0.),
            ..default()
        },
        RigidBody::Static,
        Collider::cuboid(w, h),
        Friction::new(1.),
        Restitution::new(0.).with_combine_rule(CoefficientCombine::Multiply),
    )
}

#[derive(Component, Default)]
struct KeepUpright {}

/// We will store the world position of the mouse cursor here.
#[derive(Resource, Default)]
struct MyWorldCoords(Vec2);

/// Used to help identify our main camera
#[derive(Component)]
struct MainCamera;

fn setup(mut commands: Commands) {
    commands.init_resource::<MyWorldCoords>();
    commands.spawn((Camera2dBundle::default(), MainCamera));

    commands.spawn(make_cube(0., 0., 100., 100., 0.));
    commands.spawn(make_cube(0., 0., 1000., 10., 50.));

    let mut rot_cube = make_cube(-100., 0., 1000., 10., 50.);
    rot_cube.0.transform.rotate_z(PI / 4.0);

    commands.spawn(rot_cube);
}

fn pan_camera(
    mut q_camera: Query<&mut Transform, With<MainCamera>>,
    mut motion_evr: EventReader<MouseMotion>,
    buttons: Res<Input<MouseButton>>,
) {
    if !buttons.pressed(MouseButton::Middle) {
        return;
    }

    let mut transform = q_camera.single_mut();
    let delta = motion_evr.read().fold(Vec2::ZERO, |sum, x| sum + x.delta);

    let delta = Vec3::new(-delta.x, delta.y, 0.0) * transform.scale.x;

    transform.translation += delta;
}

fn zoom_camera(
    mut q_camera: Query<&mut Transform, With<MainCamera>>,
    mut scroll_evr: EventReader<MouseWheel>,
) {
    let mut transform = q_camera.single_mut();

    use bevy::input::mouse::MouseScrollUnit;
    let amount: f32 = scroll_evr
        .read()
        .map(|ev| match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y * 0.1, // TODO: Tune
        })
        .sum();

    let amount = -amount; // invert

    let unit = transform.scale.normalize();

    let new: Vec3 = transform.scale + (unit * amount * 0.1);
    if new.dot(unit) > 0.0 {
        transform.scale = new;
    }
}

fn world_cursor(
    mut mycoords: ResMut<MyWorldCoords>,
    // query to get the window (so we can read the current cursor position)
    q_window: Query<&Window, With<PrimaryWindow>>,
    // query to get camera transform
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    // get the camera info and transform
    // assuming there is exactly one main camera entity, so Query::single() is OK
    let (camera, camera_transform) = q_camera.single();

    // There is only one primary window, so we can similarly get it from the query:
    let window = q_window.single();

    // check if the cursor is inside the window and get its position
    // then, ask bevy to convert into world coordinates, and truncate to discard Z
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
        .map(|ray| ray.origin.truncate())
    {
        mycoords.0 = world_position;
        // eprintln!("World coords: {}/{}", world_position.x, world_position.y);
    }
}

fn keep_upright_impl(ent: &mut Transform, normal: Vec2) {
    let angle = f32::atan2(normal.y, normal.x);
    let mut angle = angle + PI / 2.0;

    if angle.abs() < 0.01 {
        angle = 0.0;
    }

    let new_angle = Quat::from_rotation_z(angle);

    if ent.rotation.angle_between(new_angle) > 0.005 {
        ent.rotation = new_angle;
    }
}

fn keep_upright(
    mut entities: Query<&mut Transform, With<KeepUpright>>,
    mut collisions: EventReader<Collision>,
) {
    for Collision(contacts) in collisions.read() {
        assert!(contacts.manifolds.len() == 1);
        let contact = contacts.manifolds.first().unwrap();

        if let Ok(mut ent) = entities.get_mut(contacts.entity1) {
            let normal = contact.global_normal1(&Rotation::from(ent.rotation));
            keep_upright_impl(&mut ent, normal);
        } else if let Ok(mut ent) = entities.get_mut(contacts.entity2) {
            let normal = contact.global_normal2(&Rotation::from(ent.rotation));
            keep_upright_impl(&mut ent, normal);
        }
    }
}

fn debug(
    mut player: Query<&mut Transform, With<player::Player>>,
    mut last_click_pos: Local<Option<Vec2>>,
    mouse: Res<Input<MouseButton>>,
    coords: Res<MyWorldCoords>,
    keys: Res<Input<KeyCode>>,
    mut commands: Commands,
) {
    let coords = coords.0;

    if !keys.pressed(KeyCode::ControlLeft) {
        return;
    }

    // Make geo
    if mouse.just_pressed(MouseButton::Left) {
        match *last_click_pos {
            Some(pos) => {
                let center = (pos + coords) / 2.0;
                let len = pos.distance(coords);

                let mut cube = make_cube(center.x, center.y, len, 10.0, 50.0);

                let diff = coords - pos;
                let rotation = f32::atan2(diff.y, diff.x);

                cube.0.transform.rotate_z(rotation);

                commands.spawn(cube);

                *last_click_pos = None;
            }
            None => *last_click_pos = Some(coords),
        }
    }

    let mut transform = match player.iter_mut().next() {
        Some(x) => x,
        None => return,
    };

    // Teleport
    if mouse.just_pressed(MouseButton::Right) {
        transform.translation = Vec3::new(coords.x, coords.y, transform.translation.z);
    }
}

// fn sprite_movement(time: Res<Time>, mut sprite_position: Query<&mut Transform, With<Sprite>>) {
//     for mut transform in &mut sprite_position {
//         transform.rotate_z(3. * time.delta_seconds());
//     }
// }
