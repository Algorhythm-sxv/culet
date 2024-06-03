use std::borrow::Cow;

use bevy::{
    core_pipeline::{core_3d::graph::Node3d, fxaa::FxaaNode, upscaling::UpscalingNode},
    ecs::query::QueryItem,
    prelude::*,
    render::{
        camera::{CameraOutputMode, ExtractedCamera},
        mesh::{PrimitiveTopology, VertexAttributeValues},
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, RenderSubGraph,
            ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{
                storage_buffer_read_only, texture_2d, texture_storage_2d, uniform_buffer,
            },
            BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedComputePipelineId,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, ComputePassDescriptor,
            ComputePipelineDescriptor, Extent3d, FragmentState, FrontFace, LoadOp,
            MultisampleState, Operations, PipelineCache, PolygonMode, PrimitiveState,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
            ShaderStages, ShaderType, StorageBuffer, StorageTextureAccess, StoreOp, Texture,
            TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
            TextureUsages, TextureViewDescriptor, TextureViewDimension, UniformBuffer, VertexState,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::ViewTarget,
        Extract, Render, RenderApp, RenderSet,
    },
};

use crate::bvh::{Bvh, BvhNode};

#[derive(Component)]
pub struct CuletMesh;

#[derive(Resource)]
pub struct ExtractedMesh {
    mesh: Option<Mesh>,
}

fn extract_mesh(
    mut commands: Commands,
    meshes: Extract<Res<Assets<Mesh>>>,
    mesh: Extract<Query<&Handle<Mesh>, With<CuletMesh>>>,
) {
    let mesh_id = mesh.get_single().unwrap();
    let extracted_mesh = meshes.get(mesh_id).map(|m| m.to_owned());

    commands.insert_resource(ExtractedMesh {
        mesh: extracted_mesh,
    })
}

#[derive(Resource)]
pub struct PreparedMesh {
    vertices: StorageBuffer<Vec<Vec4>>,
    indices: StorageBuffer<Vec<u32>>,
    triangle_indices: StorageBuffer<Vec<u32>>,
    bvh_nodes: StorageBuffer<Vec<BvhNode>>,
}

fn prepare_mesh(
    mut commands: Commands,
    mesh: Res<ExtractedMesh>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    if let Some(mesh) = &mesh.mesh {
        let vertex_positions: Vec<_> = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .expect("Mesh has no vertex positions")
            .iter()
            .map(|&f3| Vec3::new(f3[0], f3[1], f3[2]))
            .collect();

        let vertex_indices: Vec<_> = mesh
            .indices()
            .expect("Mesh has no vertex indices")
            .iter()
            .map(|x| x as u32)
            .collect();

        let bvh = Bvh::new(&vertex_positions, &vertex_indices);
        let (mut vertices, mut indices, mut triangle_indices, mut bvh_nodes) = bvh.gpu_buffers();

        vertices.write_buffer(&device, &queue);
        indices.write_buffer(&device, &queue);
        triangle_indices.write_buffer(&device, &queue);
        bvh_nodes.write_buffer(&device, &queue);

        commands.insert_resource(PreparedMesh {
            vertices,
            indices,
            triangle_indices,
            bvh_nodes,
        })
    }
}

#[derive(Resource)]
struct OutputTexture {
    texture: Texture,
}
impl FromWorld for OutputTexture {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::all(),
            view_formats: &[],
        });

        Self { texture }
    }
}

#[derive(Copy, Clone, Debug, Default, Resource, ShaderType)]
#[repr(C)]
pub struct CuletCameraParams {
    origin: Vec3,
    _pad0: f32,
    look_dir: Vec3,
    _pad1: f32,
    up: Vec3,
    fov: f32,
    _pad2: f32,
    _pad3: Vec3,
}

fn extract_camera_params(
    mut commands: Commands,
    camera: Extract<Query<(&GlobalTransform, &Projection), With<CuletCamera>>>,
) {
    let (transform, projection) = camera.single();

    let fov = match projection {
        Projection::Perspective(p) => p.fov,
        Projection::Orthographic(_) => 0.0,
    };

    let params = CuletCameraParams {
        origin: transform.translation(),
        look_dir: transform.forward(),
        up: transform.up(),
        fov,
        ..default()
    };

    commands.insert_resource(params);
}

#[derive(Resource)]
struct PreparedCameraParams {
    uniform: UniformBuffer<CuletCameraParams>,
}

fn prepare_camera_params(
    mut commands: Commands,
    params: Res<CuletCameraParams>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let mut uniform = UniformBuffer::from(*params);
    uniform.write_buffer(&device, &queue);

    commands.insert_resource(PreparedCameraParams { uniform });
}

#[derive(Resource)]
struct PreparedViewportDims {
    dims: UVec2,
    uniform: UniformBuffer<UVec2>,
}

fn prepare_viewport_dims(
    mut commands: Commands,
    camera: Query<&ExtractedCamera>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let camera = camera.single();

    let mut uniform = UniformBuffer::from(camera.physical_viewport_size.unwrap());
    uniform.write_buffer(&device, &queue);

    commands.insert_resource(PreparedViewportDims {
        dims: camera.physical_viewport_size.unwrap(),
        uniform,
    });
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, RenderSubGraph)]
pub struct CuletGraph;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, RenderLabel)]
pub struct CuletLabel;

#[derive(Component)]
pub struct CuletCamera;

#[derive(Default)]
pub struct CuletNode;

impl ViewNode for CuletNode {
    type ViewQuery = (&'static ViewTarget, Option<&'static ExtractedCamera>);

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (target, camera): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let culet_pipeline = world.resource::<CuletPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let compute_pipeline = pipeline_cache
            .get_compute_pipeline(culet_pipeline.compute_pipeline_id)
            .unwrap();

        let output_texture = world.resource::<OutputTexture>();
        let output_texture_view = output_texture.texture.create_view(&TextureViewDescriptor {
            label: Some("compute shader output texture"),
            format: Some(TextureFormat::Rgba32Float),
            dimension: Some(TextureViewDimension::D2),
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let prepared_mesh = world.resource::<PreparedMesh>();
        let camera_params = world.resource::<PreparedCameraParams>();

        let compute_bind_group = render_context.render_device().create_bind_group(
            None,
            &culet_pipeline.compute_layout,
            &BindGroupEntries::sequential((
                prepared_mesh.vertices.binding().unwrap(),
                prepared_mesh.indices.binding().unwrap(),
                prepared_mesh.triangle_indices.binding().unwrap(),
                prepared_mesh.bvh_nodes.binding().unwrap(),
                camera_params.uniform.binding().unwrap(),
                &output_texture_view,
            )),
        );

        let render_pipeline = pipeline_cache
            .get_render_pipeline(culet_pipeline.render_pipeline_id)
            .unwrap();

        let viewport_dims = world.resource::<PreparedViewportDims>();

        let render_bind_group = render_context.render_device().create_bind_group(
            None,
            &culet_pipeline.render_layout,
            &BindGroupEntries::sequential((
                &output_texture_view,
                viewport_dims.uniform.binding().unwrap(),
            )),
        );

        let command_encoder = render_context.command_encoder();
        let mut compute_pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Compute pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(compute_pipeline);
        compute_pass.set_bind_group(0, &compute_bind_group, &[]);
        compute_pass.dispatch_workgroups(
            (viewport_dims.dims.x + 7) / 8,
            (viewport_dims.dims.y + 7) / 8,
            1,
        );
        drop(compute_pass);

        let color_attachment_load_op = if let Some(camera) = camera {
            match camera.output_mode {
                CameraOutputMode::Write {
                    color_attachment_load_op,
                    ..
                } => color_attachment_load_op,
                CameraOutputMode::Skip => return Ok(()),
            }
        } else {
            LoadOp::Clear(Default::default())
        };

        let mut render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Blitting pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target.main_texture_view(),
                resolve_target: None,
                ops: Operations {
                    load: color_attachment_load_op,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(render_pipeline);
        render_pass.set_bind_group(0, &render_bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Resource)]
struct CuletPipeline {
    compute_layout: BindGroupLayout,
    compute_pipeline_id: CachedComputePipelineId,
    render_layout: BindGroupLayout,
    render_pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for CuletPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let compute_layout = render_device.create_bind_group_layout(
            None,
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    storage_buffer_read_only::<Vec<Vec4>>(false), // vertices
                    storage_buffer_read_only::<Vec<u32>>(false),  // indices
                    storage_buffer_read_only::<Vec<u32>>(false),  // triangle indices
                    storage_buffer_read_only::<Vec<BvhNode>>(false), // BVH nodes
                    uniform_buffer::<CuletCameraParams>(false),
                    texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::ReadWrite), // output texture
                ),
            ),
        );
        let render_layout = render_device.create_bind_group_layout(
            None,
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX_FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    uniform_buffer::<UVec2>(false),
                ),
            ),
        );

        let compute_shader = world.resource::<AssetServer>().load("ray_tracing.wgsl");
        let render_shader = world.resource::<AssetServer>().load("blitting.wgsl");

        let compute_pipeline_id = world
            .resource_mut::<PipelineCache>()
            .queue_compute_pipeline(ComputePipelineDescriptor {
                label: None,
                layout: vec![compute_layout.clone()],
                push_constant_ranges: vec![],
                shader: compute_shader,
                shader_defs: vec![],
                entry_point: Cow::from("main"),
            });

        let render_pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: None,
                    layout: vec![render_layout.clone()],
                    push_constant_ranges: vec![],
                    vertex: VertexState {
                        shader: render_shader.clone(),
                        shader_defs: vec![],
                        entry_point: Cow::from("vertex"),
                        buffers: vec![],
                    },
                    primitive: PrimitiveState {
                        topology: PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: FrontFace::Ccw,
                        cull_mode: None,
                        unclipped_depth: false,
                        polygon_mode: PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    fragment: Some(FragmentState {
                        shader: render_shader,
                        shader_defs: vec![],
                        entry_point: Cow::from("fragment"),
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::Rgba8UnormSrgb,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                });

        Self {
            compute_layout,
            compute_pipeline_id,
            render_layout,
            render_pipeline_id,
        }
    }
}

pub struct CuletPlugin;

impl Plugin for CuletPlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.get_sub_app_mut(RenderApp).unwrap();

        render_app.add_systems(
            ExtractSchedule,
            (
                extract_mesh.in_set(RenderSet::ExtractCommands),
                extract_camera_params.in_set(RenderSet::ExtractCommands),
            ),
        );
        render_app.add_systems(
            Render,
            (
                prepare_mesh.in_set(RenderSet::Prepare),
                prepare_camera_params.in_set(RenderSet::Prepare),
                prepare_viewport_dims.in_set(RenderSet::PrepareResources),
            ),
        );

        render_app
            .add_render_sub_graph(CuletGraph)
            .add_render_graph_node::<ViewNodeRunner<CuletNode>>(CuletGraph, CuletLabel)
            .add_render_graph_node::<ViewNodeRunner<UpscalingNode>>(CuletGraph, Node3d::Upscaling)
            .add_render_graph_node::<ViewNodeRunner<FxaaNode>>(CuletGraph, Node3d::Fxaa)
            .add_render_graph_edges(CuletGraph, (CuletLabel, Node3d::Upscaling));
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.get_sub_app_mut(RenderApp).unwrap();

        render_app.init_resource::<CuletPipeline>();
        render_app.init_resource::<OutputTexture>();
    }
}
