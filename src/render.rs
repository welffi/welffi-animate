use crate::rig::{Rig, JOINT_COUNT};
use eframe::egui;
use eframe::egui_wgpu::{wgpu, RenderState};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

const SAMPLES: u32 = 4;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADER: &str = include_str!("shader.wgsl");

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineVertex {
    pos: [f32; 3],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    tint: [f32; 4],
}

pub struct PartMesh {
    pub vertices: Vec<Vertex>,
    pub min: Vec3,
    pub max: Vec3,
}

pub fn load_parts(bytes: &[u8], rig: &Rig) -> [PartMesh; JOINT_COUNT] {
    let mut found: Vec<(String, PartMesh)> = Vec::new();
    let mut at = 4;
    let read_u32 = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let count = read_u32(0);
    for _ in 0..count {
        let name_len = bytes[at] as usize;
        let name = String::from_utf8_lossy(&bytes[at + 1..at + 1 + name_len]).to_string();
        at += 1 + name_len;
        let vertex_count = read_u32(at);
        at += 4;
        let size = std::mem::size_of::<Vertex>() * vertex_count;
        let vertices: Vec<Vertex> = bytemuck::pod_collect_to_vec(&bytes[at..at + size]);
        at += size;
        let (min, max) = vertices.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), v| {
            let p = Vec3::from(v.pos);
            (lo.min(p), hi.max(p))
        });
        found.push((name, PartMesh { vertices, min, max }));
    }
    rig.joints.each_ref().map(|joint| {
        let i = found.iter().position(|(name, _)| name == joint.part).expect("rig mesh is missing a part");
        found.swap_remove(i).1
    })
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    resolve: wgpu::TextureView,
    depth: wgpu::TextureView,
}

pub struct Scene3d {
    mesh_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    parts: Vec<(wgpu::Buffer, u32)>,
    part_uniforms: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    grid: (wgpu::Buffer, u32),
    grid_uniform: (wgpu::Buffer, wgpu::BindGroup),
    skin: wgpu::BindGroup,
    targets: Option<Targets>,
    texture_id: Option<egui::TextureId>,
}

impl Scene3d {
    pub fn new(rs: &RenderState, meshes: &[PartMesh; JOINT_COUNT], skin_png: &[u8]) -> Scene3d {
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let skin_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skin"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let mesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&skin_layout)],
            immediate_size: 0,
        });
        let line_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("line"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });

        let mesh_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let line_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let mesh_pipeline = pipeline(
            device,
            &mesh_layout,
            &shader,
            ("mesh_vs", "mesh_fs"),
            wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &mesh_attributes },
            wgpu::PrimitiveTopology::TriangleList,
        );
        let line_pipeline = pipeline(
            device,
            &line_layout,
            &shader,
            ("line_vs", "line_fs"),
            wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<LineVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &line_attributes },
            wgpu::PrimitiveTopology::LineList,
        );

        let parts = meshes
            .iter()
            .map(|m| {
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("part"),
                    contents: bytemuck::cast_slice(&m.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                (buffer, m.vertices.len() as u32)
            })
            .collect();
        let make_uniform = || {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uniform"),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("uniform"),
                layout: &uniform_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            });
            (buffer, group)
        };
        let part_uniforms = (0..JOINT_COUNT).map(|_| make_uniform()).collect();
        let grid_uniform = make_uniform();

        let grid_lines = grid_vertices();
        let grid = (
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("grid"),
                contents: bytemuck::cast_slice(&grid_lines),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            grid_lines.len() as u32,
        );

        let image = image::load_from_memory(skin_png).expect("rig texture").to_rgba8();
        let texture = device.create_texture_with_data(
            &rs.queue,
            &wgpu::TextureDescriptor {
                label: Some("skin"),
                size: wgpu::Extent3d { width: image.width(), height: image.height(), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            image.as_raw(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let skin = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skin"),
            layout: &skin_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&texture.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        Scene3d { mesh_pipeline, line_pipeline, parts, part_uniforms, grid, grid_uniform, skin, targets: None, texture_id: None }
    }

    pub fn render(&mut self, rs: &RenderState, size: [u32; 2], view_proj: Mat4, worlds: &[Mat4; JOINT_COUNT], tints: &[[f32; 4]; JOINT_COUNT]) -> egui::TextureId {
        let size = [size[0].max(1), size[1].max(1)];
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            let targets = make_targets(&rs.device, size);
            let mut renderer = rs.renderer.write();
            match self.texture_id {
                Some(id) => renderer.update_egui_texture_from_wgpu_texture(&rs.device, &targets.resolve, wgpu::FilterMode::Linear, id),
                None => self.texture_id = Some(renderer.register_native_texture(&rs.device, &targets.resolve, wgpu::FilterMode::Linear)),
            }
            self.targets = Some(targets);
        }
        let targets = self.targets.as_ref().unwrap();

        let write = |buffer: &wgpu::Buffer, model: Mat4, tint: [f32; 4]| {
            let u = Uniforms { mvp: (view_proj * model).to_cols_array_2d(), model: model.to_cols_array_2d(), tint };
            rs.queue.write_buffer(buffer, 0, bytemuck::bytes_of(&u));
        };
        write(&self.grid_uniform.0, Mat4::IDENTITY, [0.0; 4]);
        for i in 0..JOINT_COUNT {
            write(&self.part_uniforms[i].0, worlds[i], tints[i]);
        }

        let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("scene") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.msaa,
                    depth_slice: None,
                    resolve_target: Some(&targets.resolve),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.22, g: 0.22, b: 0.235, a: 1.0 }), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.mesh_pipeline);
            pass.set_bind_group(1, &self.skin, &[]);
            for ((buffer, count), (_, group)) in self.parts.iter().zip(&self.part_uniforms) {
                pass.set_bind_group(0, group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
            pass.set_pipeline(&self.line_pipeline);
            pass.set_bind_group(0, &self.grid_uniform.1, &[]);
            pass.set_vertex_buffer(0, self.grid.0.slice(..));
            pass.draw(0..self.grid.1, 0..1);
        }
        rs.queue.submit([encoder.finish()]);
        self.texture_id.unwrap()
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    entries: (&str, &str),
    buffer: wgpu::VertexBufferLayout,
    topology: wgpu::PrimitiveTopology,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entries.0),
        layout: Some(layout),
        vertex: wgpu::VertexState { module: shader, entry_point: Some(entries.0), compilation_options: Default::default(), buffers: &[Some(buffer)] },
        primitive: wgpu::PrimitiveState { topology, cull_mode: None, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(topology == wgpu::PrimitiveTopology::TriangleList),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entries.1),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn make_targets(device: &wgpu::Device, size: [u32; 2]) -> Targets {
    let texture = |format, samples, usage| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("scene target"),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&Default::default())
    };
    Targets {
        size,
        msaa: texture(COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT),
        resolve: texture(COLOR_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING),
        depth: texture(DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT),
    }
}

fn grid_vertices() -> Vec<LineVertex> {
    let extent = 20;
    let mut lines = Vec::new();
    let mut add = |a: [f32; 3], b: [f32; 3], color: [f32; 4]| {
        lines.push(LineVertex { pos: a, color });
        lines.push(LineVertex { pos: b, color });
    };
    let gray = [0.29, 0.29, 0.3, 1.0];
    let e = extent as f32;
    for i in -extent..=extent {
        let v = i as f32;
        let x_color = if i == 0 { [0.62, 0.22, 0.25, 1.0] } else { gray };
        let z_color = if i == 0 { [0.24, 0.4, 0.68, 1.0] } else { gray };
        add([-e, 0.0, v], [e, 0.0, v], x_color);
        add([v, 0.0, -e], [v, 0.0, e], z_color);
    }
    lines
}
