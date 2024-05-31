use bevy::{
    core_pipeline::{
        fxaa::{Fxaa, Sensitivity},
        prepass::{DepthPrepass, NormalPrepass},
    },
    pbr::wireframe::WireframePlugin,
    prelude::*, render::camera::CameraRenderGraph,
};
use bevy_mod_edge_detection::{EdgeDetectionCamera, EdgeDetectionConfig, EdgeDetectionPlugin};
use bevy_panorbit_camera::*;
use bevy_stl::StlPlugin;
use ray_tracing::{CuletCamera, CuletGraph, CuletMesh, CuletPlugin};

mod ray_tracing;

fn main() {
    App::new()
        .insert_resource(Msaa::Off)
        .add_plugins(DefaultPlugins)
        .add_plugins(EdgeDetectionPlugin)
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(StlPlugin)
        .add_plugins(WireframePlugin)
        .add_plugins(CuletPlugin)
        .init_resource::<EdgeDetectionConfig>()
        .add_systems(Startup, setup)
        .add_systems(Update, switch_cameras)
        .run();
}

#[derive(Component)]
pub struct CadCamera;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // commands.insert_resource(WireframeConfig {
    //     global: true,
    //     default_color: Color::BLACK.into(),
    // });
    // commands.spawn(DirectionalLightBundle {
    //     directional_light: DirectionalLight {
    //         color: Color::WHITE,
    //         illuminance: light_consts::lux::OVERCAST_DAY,
    //         shadows_enabled: false,
    //         ..default()
    //     },
    //     transform: Transform {
    //         translation: Vec3::new(0.0, 0.0, -10.0),
    //         ..default()
    //     },
    //     ..default()
    // });
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
    });

    let mesh = asset_server.load("lowboy.stl");
    commands.spawn((
        PbrBundle {
            mesh: mesh.clone(),
            material: materials.add(Color::rgb(0.9, 0.9, 0.9)),
            ..default()
        },
        CuletMesh,
    ));

    // CAD wireframe camera
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                order: 1,
                ..default()
            },
            ..default()
        },
        PanOrbitCamera {
            focus: Vec3::new(0.0, 0.0, 0.0),
            radius: Some(5.0),
            orbit_sensitivity: 0.5,
            pan_sensitivity: 0.0,
            ..default()
        },
        DepthPrepass,
        NormalPrepass,
        Fxaa {
            enabled: true,
            edge_threshold: Sensitivity::Extreme,
            edge_threshold_min: Sensitivity::Extreme,
        },
        EdgeDetectionCamera,
        CadCamera,
    ));

    // Ray-tracing camera
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                order: 0,
                is_active: false,
                ..default()
            },
            camera_render_graph: CameraRenderGraph::new(CuletGraph),
            ..default()
        },
        PanOrbitCamera { ..default() },
        CuletCamera,
    ));
}

#[allow(clippy::type_complexity)]
fn switch_cameras(
    keys: Res<ButtonInput<KeyCode>>,
    mut cad_cam: Query<
        (&mut Camera, &mut PanOrbitCamera, &mut Transform),
        (With<CadCamera>, Without<CuletCamera>),
    >,
    mut culet_cam: Query<(&mut Camera, &mut PanOrbitCamera, &mut Transform), With<CuletCamera>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        let (cad_cam, cad_pan, cad_transform) = cad_cam.single_mut();
        let (ray_cam, ray_pan, ray_transform) = culet_cam.single_mut();

        let (
            mut active_cam,
            active_pan,
            active_transform,
            mut inactive_cam,
            mut inactive_pan,
            mut inactive_transform,
        ) = if cad_cam.is_active {
            (
                cad_cam,
                cad_pan,
                cad_transform,
                ray_cam,
                ray_pan,
                ray_transform,
            )
        } else {
            (
                ray_cam,
                ray_pan,
                ray_transform,
                cad_cam,
                cad_pan,
                cad_transform,
            )
        };

        // copy active camera params to inactive camera
        *inactive_pan = *active_pan;
        *inactive_cam = active_cam.clone();
        *inactive_transform = *active_transform;

        // swap cameras
        active_cam.is_active = false;
        inactive_cam.is_active = true;

        // make sure active cam is on top
        inactive_cam.order = 1;
        active_cam.order = 0;
    }
}
