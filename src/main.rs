mod plot;
mod source;

use crate::plot::WaterfallPlot;
use crate::source::DataSupplier;
use clap::Parser;

/// Simple program to plot a waterfall from standard input
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    frequency: u32,

    #[arg(short, long)]
    samplerate: u32,

    #[arg(short, long)]
    bandwidth: u32,

    /// Enable vertical synchronization (avoids tearing)
    #[arg(short, long)]
    vsync: bool,

    /// Number of FFT blocks to average
    #[arg(short, long, default_value_t = 1)]
    averaging: u32,
}

fn main() {
    // Command line argument parsing
    let args = Args::parse();

    // Window creation
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new()
        .with_title("Waterfall")
        .with_inner_size(glutin::dpi::LogicalSize::new(1024.0, 1024.0));
    let windowed_context = glutin::ContextBuilder::new()
        .with_gl(glutin::GlRequest::GlThenGles {
            opengl_version: (3, 0),
            opengles_version: (3, 0),
        })
        .with_vsync(args.vsync)
        .build_windowed(wb, &event_loop)
        .unwrap();
    let window = unsafe { windowed_context.make_current().unwrap() };
    let gl =
        unsafe { glow::Context::from_loader_function(|s| window.get_proc_address(s) as *const _) };

    //
    let mut waterfallplot = unsafe { WaterfallPlot::new(gl) };
    let mut samples_supplier = DataSupplier::new(args.averaging);
    let mut last_touch: f64 = 0.0;
    let mut frequency = args.frequency;

    samples_supplier.set_frequency(frequency);
    samples_supplier.set_samplerate(args.samplerate);
    samples_supplier.set_bandwidth(args.bandwidth);
    samples_supplier.activate();

    unsafe {
        {
            use glutin::event::{Event, TouchPhase, WindowEvent};
            use glutin::event_loop::ControlFlow;

            event_loop.run(move |event, _, control_flow| {
                *control_flow = ControlFlow::Wait;
                match event {
                    Event::LoopDestroyed => {
                        return;
                    }
                    Event::MainEventsCleared => {
                        window.window().request_redraw();
                    }
                    Event::RedrawRequested(_) => {
                        waterfallplot.update_plot(samples_supplier.get_block());
                        window.swap_buffers().unwrap();
                    }
                    Event::WindowEvent { ref event, .. } => match event {
                        WindowEvent::Resized(physical_size) => {
                            waterfallplot
                                .set_window_size(physical_size.width, physical_size.height);
                            window.resize(*physical_size);
                        }
                        WindowEvent::CloseRequested => {
                            waterfallplot.drop();
                            *control_flow = ControlFlow::Exit
                        }
                        WindowEvent::Touch(touch) => match touch.phase {
                            TouchPhase::Started => {
                                last_touch = touch.location.y;
                            }
                            TouchPhase::Moved | TouchPhase::Ended => {
                                waterfallplot.scroll((last_touch - touch.location.y) as i32);
                                last_touch = touch.location.y;
                            }
                            _ => (),
                        },
                        WindowEvent::KeyboardInput {
                            input:
                                glutin::event::KeyboardInput {
                                    virtual_keycode: Some(key),
                                    state: glutin::event::ElementState::Pressed,
                                    ..
                                },
                            ..
                        } => match key {
                            glutin::event::VirtualKeyCode::A => {
                                waterfallplot.incr_offset(10.0);
                            }
                            glutin::event::VirtualKeyCode::Z => {
                                waterfallplot.incr_offset(-10.0);
                            }
                            glutin::event::VirtualKeyCode::S => {
                                waterfallplot.incr_max(10.0);
                            }
                            glutin::event::VirtualKeyCode::X => {
                                waterfallplot.incr_max(-10.0);
                            }
                            glutin::event::VirtualKeyCode::D => {
                                waterfallplot.incr_min(10.0);
                            }
                            glutin::event::VirtualKeyCode::C => {
                                waterfallplot.incr_min(-10.0);
                            }
                            glutin::event::VirtualKeyCode::PageUp => {
                                frequency += 10e6 as u32;
                                samples_supplier.set_frequency(frequency);
                            }
                            glutin::event::VirtualKeyCode::PageDown => {
                                frequency -= 10e6 as u32;
                                samples_supplier.set_frequency(frequency);
                            }
                            glutin::event::VirtualKeyCode::Right => {
                                frequency += 1e6 as u32;
                                samples_supplier.set_frequency(frequency);
                            }
                            glutin::event::VirtualKeyCode::Left => {
                                frequency -= 1e6 as u32;
                                samples_supplier.set_frequency(frequency);
                            }
                            glutin::event::VirtualKeyCode::Up => {
                                waterfallplot.scroll(-100);
                            }
                            glutin::event::VirtualKeyCode::Down => {
                                waterfallplot.scroll(100);
                            }
                            _ => (),
                        },
                        _ => (),
                    },
                    _ => (),
                }
            });
        }
    }
}
