use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue,
};
use std::borrow::Cow;
use std::collections::HashMap;
use wgpu;
use wgpu::util::DeviceExt;
use wgpu::{BufferAddress, Extent3d, ImageCopyTexture, StoreOp};

struct ShaderMaker {
    composer: Composer,
}

impl ShaderMaker {
    pub fn new() -> Self {
        let composer = Composer::default();

        Self { composer }
    }

    /// Add a shader as a composable module so that it can be imported by other shaders.
    pub fn add_composable(
        &mut self,
        source: &str,
        module_name: &str,
        shader_defs: &[&str],
    ) {
        let module_exists = self.composer.contains_module(module_name);

        if !module_exists {
            let mut shader_defs_map: HashMap<String, ShaderDefValue> = HashMap::new();
            for def in shader_defs.iter() {
                shader_defs_map.insert((*def).into(), Default::default());
            }

            match self
                .composer
                .add_composable_module(ComposableModuleDescriptor {
                    source,
                    shader_defs: shader_defs_map,
                    as_name: Some(module_name.into()),
                    ..Default::default()
                }) {
                Ok(module) => {
                    println!(
                        "Added composable module {} [{:?}]",
                        module.name, module.shader_defs
                    )
                }
                Err(e) => {
                    println!("? -> {e:#?}")
                }
            }
        };
    }

    /// Make a naga module using the shader.
    pub fn make_shader(
        &mut self,
        source: &str,
        shader_defs: &[&str],
    ) -> Option<wgpu::ShaderSource> {
        let mut shader_defs_map: HashMap<String, ShaderDefValue> = HashMap::new();
        for def in shader_defs.iter() {
            shader_defs_map.insert((*def).into(), Default::default());
        }

        match self.composer.make_naga_module(NagaModuleDescriptor {
            source,
            shader_defs: shader_defs_map.into(),
            ..Default::default()
        }) {
            Ok(module) => Some(wgpu::ShaderSource::Naga(Cow::Owned(module))),
            Err(e) => {
                println!("{}", e.emit_to_string(&self.composer));
                None
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex2d {
    pub(crate) position: [f32; 2],
}

pub trait VertexBuffer {
    /// Vertex buffer layout provided to a pipeline.
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a>;
}

impl VertexBuffer for Vertex2d {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex2d>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                // Position.
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

fn main() {
    // Context for all other wgpu objects.
    let instance = wgpu::Instance::default();

    // Handle to a physical graphics and/or compute device.
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
        .unwrap();

    // Use the adapter to create a device and a queue.
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
            label: None,
        },
        None,
    ))
        .unwrap();

    let mut shader_maker = ShaderMaker::new();

    let shader_source = shader_maker.make_shader(include_str!("test.wgsl"), &["BLUE"]);

    let shader_desc = wgpu::ShaderModuleDescriptor {
        label: None,
        source: shader_source.unwrap(),
    };

    let shader = device.create_shader_module(shader_desc);

    let texture_size = (128u32, 128u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let u32_size = std::mem::size_of::<u32>() as u32;

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[Vertex2d::desc()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        // If the pipeline will be used with a multiview resource pass, this
        // indicates how many array layers the attachments will have.
        multiview: None,
    });

    let vertices = [
        Vertex2d {
            position: [-1.0, 1.0],
        },
        Vertex2d {
            position: [-1.0, -1.0],
        },
        Vertex2d {
            position: [1.0, -1.0],
        },
        Vertex2d {
            position: [1.0, 1.0],
        },
    ];

    let indices = [0, 1, 2, 2, 3, 0];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("vertex buffer")),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("index buffer")),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: Extent3d {
            width: texture_size.0,
            height: texture_size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(&format!("staging buffer")),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        size: (align_up(
            u32_size * texture_size.0,
            wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
        ) * texture_size.1) as BufferAddress,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("main render encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main render pass"),
            color_attachments: &[
                // This is what @location(0) in the fragment shader targets.
                Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view, // Change this to change where to draw.
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&pipeline);

        // Set vertex buffer for VertexInput.
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));

        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        render_pass.draw_indexed(0..6, 0, 0..1);
    }

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            aspect: wgpu::TextureAspect::All,
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(align_up(
                    u32_size * texture_size.0,
                    wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
                )),
                rows_per_image: Some(texture_size.1),
            },
        },
        wgpu::Extent3d {
            width: texture_size.0 as u32,
            height: texture_size.1 as u32,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);

    let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    device.poll(wgpu::Maintain::Wait);
    pollster::block_on(rx.receive()).unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();

    use image::{ImageBuffer, Rgba};
    let buffer =
        ImageBuffer::<Rgba<u8>, _>::from_raw(texture_size.0, texture_size.1, data).unwrap();
    buffer.save("image.png").unwrap();

    // Have to drop the BufferView before unmapping.
    std::mem::drop(buffer);

    staging_buffer.unmap();
}

pub const fn align_up(num: u32, align: u32) -> u32 {
    ((num) + ((align) - 1)) & !((align) - 1)
}
