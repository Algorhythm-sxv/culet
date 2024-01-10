use std::sync::{mpsc::channel, Arc};

use glam::vec3;
use wgpu::{util::DeviceExt, Device, Queue};

use crate::{
    camera::Camera,
    mesh::{GpuTriangle, Mesh},
    render::GpuRenderInfo,
};

pub const TEXTURE_SIZE: u32 = 1024;

#[derive(Debug)]
pub struct WgpuHandle {
    device: Arc<Device>,
    queue: Arc<Queue>,
    vertex_buffer: wgpu::Buffer,
    texture: wgpu::Texture,
    output_buffer: wgpu::Buffer,
    texture_bind_group: wgpu::BindGroup,
    triangle_bind_group: wgpu::BindGroup,
    camera_bind_group: wgpu::BindGroup,
    render_info_bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl WgpuHandle {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        // create a texture for the GPU to render to internally
        let texture_desc = wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: TEXTURE_SIZE,
                height: TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_formats: &[],
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::STORAGE_BINDING,
            label: None,
        };
        let texture = device.create_texture(&texture_desc);
        let texture_view = texture.create_view(&Default::default());

        // create a buffer to shuffle the rendered texture back to the CPU
        let output_buffer_size = (4 * TEXTURE_SIZE * TEXTURE_SIZE) as wgpu::BufferAddress;
        let output_buffer_desc = wgpu::BufferDescriptor {
            size: output_buffer_size,
            label: Some("GPU output buffer"),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        };
        let output_buffer = device.create_buffer(&output_buffer_desc);

        // create a buffer to store the triangle and normal information for the GPU
        let init_tris = [GpuTriangle::new(
            vec3(0.0, 0.0, -1.5),
            vec3(1.0, 0.0, -1.5),
            vec3(0.0, 1.0, -1.5),
        )];
        let triangle_buffer_desc = wgpu::util::BufferInitDescriptor {
            label: Some("Triangle buffer"),
            contents: bytemuck::cast_slice(&init_tris),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        };
        let triangle_buffer = device.create_buffer_init(&triangle_buffer_desc);

        // create a buffer to store the camera information for the GPU
        let camera = [Camera::default().aspect_ratio(1.0)];
        let camera_buffer_desc = wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::cast_slice(&camera),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        };
        let camera_buffer = device.create_buffer_init(&camera_buffer_desc);

        // create a container struct for render info
        let render_info = [GpuRenderInfo::default()];
        let render_info_buffer_desc = wgpu::util::BufferInitDescriptor {
            label: Some("RenderInfo buffer"),
            contents: bytemuck::cast_slice(&render_info),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        };
        let render_info_buffer = device.create_buffer_init(&render_info_buffer_desc);

        let shaders = device.create_shader_module(wgpu::include_wgsl!("shaders/shader.wgsl"));

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        let triangle_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let render_info_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture bind group"),
            layout: &texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            }],
        });

        let triangle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Triangle array bind group"),
            layout: &triangle_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(triangle_buffer.as_entire_buffer_binding()),
            }],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(camera_buffer.as_entire_buffer_binding()),
            }],
        });

        let render_info_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render info bind group"),
            layout: &render_info_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    render_info_buffer.as_entire_buffer_binding(),
                ),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &triangle_bind_group_layout,
                &camera_bind_group_layout,
                &render_info_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &shaders,
            entry_point: "main",
        });

        Self {
            device,
            queue,
            texture,
            vertex_buffer: triangle_buffer,
            output_buffer,
            texture_bind_group,
            triangle_bind_group,
            camera_bind_group,
            render_info_bind_group,
            pipeline,
        }
    }

    pub fn render(&self, output_buffer: &mut [u8]) {
        let device = &self.device;

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let compute_pass_descriptor = wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
            timestamp_writes: None,
        };

        {
            let mut compute_pass = encoder.begin_compute_pass(&compute_pass_descriptor);
            compute_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.triangle_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.camera_bind_group, &[]);
            compute_pass.set_bind_group(3, &self.render_info_bind_group, &[]);
            compute_pass.set_pipeline(&self.pipeline);

            // workgroup size (64, 1, 1), divide up the X axis but not the others
            compute_pass.dispatch_workgroups(TEXTURE_SIZE / 64, TEXTURE_SIZE, 1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(TEXTURE_SIZE * 4),
                    rows_per_image: Some(TEXTURE_SIZE),
                },
            },
            wgpu::Extent3d {
                width: TEXTURE_SIZE,
                height: TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        let (sender, receiver) = channel();
        let buffer_slice = self.output_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, move |r| sender.send(r).unwrap());
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().unwrap();
        {
            let view = buffer_slice.get_mapped_range();
            output_buffer.copy_from_slice(&view[..]);
        }

        self.output_buffer.unmap();
    }

    pub fn set_camera(&mut self, new_camera: &Camera) {
        let camera = [*new_camera];
        // create a buffer to store the camera information for the GPU
        let camera_buffer_desc = wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::cast_slice(&camera),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        };
        let camera_buffer = self.device.create_buffer_init(&camera_buffer_desc);

        let camera_bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        self.camera_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(camera_buffer.as_entire_buffer_binding()),
            }],
        });
    }

    pub fn set_mesh(&mut self, mesh: &Mesh) {
        let tris: Vec<GpuTriangle> = mesh.triangle_slice().iter().map(|&t| t.into()).collect();

        let triangle_buffer_desc = wgpu::util::BufferInitDescriptor {
            label: Some("Triangle buffer"),
            contents: bytemuck::cast_slice(&tris),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        };
        let triangle_buffer = self.device.create_buffer_init(&triangle_buffer_desc);
        let triangle_bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
        self.triangle_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Triangle array bind group"),
            layout: &triangle_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(triangle_buffer.as_entire_buffer_binding()),
            }],
        });
    }

    pub fn set_render_info(&mut self, info: GpuRenderInfo) {
        let render_info = [info];
        let render_info_buffer_desc = wgpu::util::BufferInitDescriptor {
            label: Some("RenderInfo buffer"),
            contents: bytemuck::cast_slice(&render_info),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        };
        let render_info_buffer = self.device.create_buffer_init(&render_info_buffer_desc);

        let render_info_bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
        self.render_info_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render info bind group"),
            layout: &render_info_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    render_info_buffer.as_entire_buffer_binding(),
                ),
            }],
        });
    }
}
