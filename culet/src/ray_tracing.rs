use std::borrow::Cow;

use bevy::{
    core_pipeline::{core_3d::graph::Node3d, fxaa::FxaaNode, upscaling::UpscalingNode},
    ecs::query::QueryItem,
    prelude::*,
    render::{
        mesh::VertexAttributeValues,
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, RenderSubGraph,
            ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{storage_buffer, storage_buffer_read_only},
            BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedComputePipelineId,
            ComputePassDescriptor, ComputePipelineDescriptor, PipelineCache, ShaderStages,
            StorageBuffer,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::ViewTarget,
        Extract, Render, RenderApp, RenderSet,
    },
};

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
    vertices: StorageBuffer<Vec<Vec3>>,
    indices: StorageBuffer<Vec<u32>>,
}

fn prepare_mesh(
    mut commands: Commands,
    mesh: Res<ExtractedMesh>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    if let Some(mesh) = mesh.mesh {
        let vertex_positions: Vec<_> = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .expect("Mesh has no vertex positions")
            .iter()
            .map(|&f3| Vec3::from_array(f3))
            .collect();

        let vertex_indices: Vec<_> = mesh
            .indices()
            .expect("Mesh has no vertex indices")
            .iter()
            .map(|x| x as u32)
            .collect();

        let mut vertices = StorageBuffer::from(vertex_positions);
        let mut indices = StorageBuffer::from(vertex_indices);

        vertices.write_buffer(&device, &queue);
        indices.write_buffer(&device, &queue);

        commands.insert_resource(PreparedMesh { vertices, indices })
    }
}

#[derive(Resource)]
struct OutputBuffer {
    buffer: StorageBuffer<Vec<Vec3>>,
}

fn prepare_output_buffer(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let texture = vec![Vec3::ZERO; 1024 * 1024];

    let mut buffer = StorageBuffer::from(texture);
    buffer.write_buffer(&device, &queue);

    commands.insert_resource(OutputBuffer { buffer })
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
    type ViewQuery = (&'static ViewTarget, &'static CuletCamera);

    fn run(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        view_query: QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let culet_pipeline = world.resource::<CuletPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let pipeline = pipeline_cache
            .get_compute_pipeline(culet_pipeline.pipeline_id)
            .unwrap();

        let prepared_mesh = world.resource::<PreparedMesh>();
        let output_buffer = world.resource::<OutputBuffer>();
        let bind_group = render_context.render_device().create_bind_group(
            None,
            &culet_pipeline.layout,
            &BindGroupEntries::sequential((
                prepared_mesh.vertices.binding().unwrap(),
                prepared_mesh.indices.binding().unwrap(),
                output_buffer.buffer.binding().unwrap(),
            )),
        );

        let command_encoder = render_context.command_encoder();
        let mut compute_pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(8, 8, 1);

        // TODO: Vertex and fragment shader to move down the chain
    }
}

#[derive(Resource)]
struct CuletPipeline {
    layout: BindGroupLayout,
    pipeline_id: CachedComputePipelineId,
}

impl FromWorld for CuletPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            None,
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    storage_buffer_read_only::<Vec<Vec3>>(false), // vertices
                    storage_buffer_read_only::<Vec<u32>>(false),  // indices
                    storage_buffer::<Vec<Vec3>>(false),           // output buffer
                                                                  // TODO: camera, render settings?
                ),
            ),
        );

        let shader = world
            .resource::<AssetServer>()
            .load("shaders/ray_tracing.wgsl");

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            .queue_compute_pipeline(ComputePipelineDescriptor {
                label: None,
                layout: vec![layout.clone()],
                push_constant_ranges: vec![],
                shader,
                shader_defs: vec![],
                entry_point: Cow::from("main"),
            });

        Self {
            layout,
            pipeline_id,
        }
    }
}

pub struct CuletPlugin;

impl Plugin for CuletPlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.get_sub_app_mut(RenderApp).unwrap();

        render_app.add_systems(
            ExtractSchedule,
            extract_mesh.in_set(RenderSet::ExtractCommands),
        );
        render_app.add_systems(Render, prepare_mesh.in_set(RenderSet::Prepare));
        render_app.add_systems(Render, prepare_output_buffer.in_set(RenderSet::Prepare));

        render_app
            .add_render_sub_graph(CuletGraph)
            .add_render_graph_node::<ViewNodeRunner<CuletNode>>(CuletGraph, CuletLabel)
            .add_render_graph_node::<ViewNodeRunner<UpscalingNode>>(CuletGraph, Node3d::Upscaling)
            .add_render_graph_node::<ViewNodeRunner<FxaaNode>>(CuletGraph, Node3d::Fxaa)
            .add_render_graph_edges(CuletGraph, (CuletLabel, Node3d::Fxaa, Node3d::Upscaling));
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.get_sub_app_mut(RenderApp).unwrap();

        render_app.init_resource::<CuletPipeline>();
    }
}
