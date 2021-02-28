use std::borrow::Cow;
use wgpu;
use winit::{
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

include!(concat!(env!("OUT_DIR"), "/embedded_shaders.rs"));

async fn run(event_loop: EventLoop<()>, window: Window) {
    let window_size = window.inner_size();
    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find a WebGPU adapter");

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    let vertex_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::SpirV(Cow::Borrowed(&shaders::TRIANGLE_VERT)),
        flags: wgpu::ShaderFlags::empty(),
    });
    let fragment_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::SpirV(Cow::Borrowed(&shaders::TRIANGLE_FRAG)),
        flags: wgpu::ShaderFlags::empty(),
    });

    let vertex_data: [f32; 24] = [
        1.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, -1.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
        1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0,
    ];
    let data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (vertex_data.len() * 4) as u64,
        usage: wgpu::BufferUsage::VERTEX,
        mapped_at_creation: true,
    });
    {
        let mut view = data_buffer.slice(..).get_mapped_range_mut();
        let float_view = unsafe {
            std::slice::from_raw_parts_mut(view.as_mut_ptr() as *mut f32, vertex_data.len())
        };
        float_view.copy_from_slice(&vertex_data)
    }
    data_buffer.unmap();

    let index_data: [u16; 3] = [0, 1, 2];
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (index_data.len() * 4) as u64,
        usage: wgpu::BufferUsage::INDEX,
        mapped_at_creation: true,
    });
    {
        let mut view = index_buffer.slice(..).get_mapped_range_mut();
        let u16_view = unsafe {
            std::slice::from_raw_parts_mut(view.as_mut_ptr() as *mut u16, index_data.len())
        };
        u16_view.copy_from_slice(&index_data)
    }
    index_buffer.unmap();

    let vertex_attrib_descs = [
        wgpu::VertexAttribute {
            offset: 0,
            format: wgpu::VertexFormat::Float4,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            offset: 4 * 4,
            format: wgpu::VertexFormat::Float4,
            shader_location: 1,
        },
    ];

    let vertex_buffer_layouts = [wgpu::VertexBufferLayout {
        array_stride: 2 * 4 * 4,
        step_mode: wgpu::InputStepMode::Vertex,
        attributes: &vertex_attrib_descs,
    }];

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let swap_chain_format = wgpu::TextureFormat::Bgra8Unorm;
    let swap_chain = device.create_swap_chain(
        &surface,
        &wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format: swap_chain_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
        },
    );

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vertex_module,
            entry_point: "main",
            buffers: &vertex_buffer_layouts,
        },
        primitive: wgpu::PrimitiveState {
            // Note: it's not possible to set a "none" strip index format,
            // which raises an error in Chrome Canary b/c when using non-strip
            // topologies, the index format must be none. However, wgpu-rs
            // instead defaults this to uint16, leading to an invalid state.
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: Some(wgpu::IndexFormat::Uint16),
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            polygon_mode: wgpu::PolygonMode::Fill,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &fragment_module,
            entry_point: "main",
            targets: &[wgpu::ColorTargetState {
                format: swap_chain_format,
                alpha_blend: wgpu::BlendState::REPLACE,
                color_blend: wgpu::BlendState::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
        }),
    });

    let clear_color = wgpu::Color {
        r: 0.3,
        g: 0.3,
        b: 0.3,
        a: 1.0,
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput { input, .. }
                    if input.virtual_keycode == Some(VirtualKeyCode::Escape) =>
                {
                    *control_flow = ControlFlow::Exit
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                let frame = swap_chain
                    .get_current_frame()
                    .expect("Failed to get swapchain frame")
                    .output;
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &frame.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(clear_color),
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: None,
                    });

                    render_pass.set_pipeline(&render_pipeline);
                    render_pass.set_vertex_buffer(0, data_buffer.slice(..));
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        // Note: also bug in wgpu-rs set_index_buffer or web sys not passing
                        // the right index type
                        render_pass
                            .set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                        render_pass.draw_indexed(0..3, 0, 0..1);
                    }
                    // This is actually kind of wrong to do, but it kind of works out anyways
                    #[cfg(target_arch = "wasm32")]
                    {
                        render_pass.draw(0..3, 0..1);
                    }
                }
                queue.submit(Some(encoder.finish()));
            }
            _ => (),
        }
    });
}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    {
        futures::executor::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("Failed to initialize logger");
        use winit::platform::web::WindowExtWebSys;
        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("Failed to append canvas to body");
        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}
