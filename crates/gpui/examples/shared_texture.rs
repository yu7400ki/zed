#[cfg(target_os = "windows")]
#[path = "example_support/fonts.rs"]
mod example_support;

#[cfg(target_os = "windows")]
mod example {
    use anyhow::{Context as _, Result};
    use gpui::{
        App, Bounds, Context, DevicePixels, Render, SharedTextureId, Window, WindowBounds,
        WindowOptions, canvas, div, point, prelude::*, px, rgb, rgba, size,
    };
    use gpui_platform::application;
    use windows::{
        Win32::{
            Foundation::HMODULE,
            Graphics::{
                Direct3D::D3D_DRIVER_TYPE_HARDWARE,
                Direct3D11::*,
                Dxgi::{
                    Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
                    DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIResource1,
                },
            },
        },
        core::Interface,
    };

    use crate::example_support;

    const TEXTURE_SIZE: i32 = 256;

    /// Draws the picture the other device shares: a checkerboard tinted by
    /// position, fading from transparent at the top to opaque at the bottom.
    /// The colors are premultiplied by the alpha, which is what a shared
    /// texture is expected to hold.
    fn picture() -> Vec<u8> {
        let extent = TEXTURE_SIZE as f32;
        let mut pixels = Vec::with_capacity((TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize);
        for y in 0..TEXTURE_SIZE {
            for x in 0..TEXTURE_SIZE {
                let alpha = y as f32 / (extent - 1.0);
                let checker = if (x / 32 + y / 32) % 2 == 0 {
                    1.0
                } else {
                    0.55
                };
                let red = checker * (x as f32 / (extent - 1.0));
                let green = checker * (1.0 - y as f32 / (extent - 1.0));
                let blue = checker * 0.85;
                pixels.extend_from_slice(&[
                    (red * alpha * 255.0) as u8,
                    (green * alpha * 255.0) as u8,
                    (blue * alpha * 255.0) as u8,
                    (alpha * 255.0) as u8,
                ]);
            }
        }
        pixels
    }

    /// A Direct3D device of its own, standing in for the process that would
    /// normally paint the texture this example shares.
    struct Producer {
        _device: ID3D11Device,
        _texture: ID3D11Texture2D,
    }

    impl Producer {
        /// Paints [`picture`] into a texture that can be opened on another
        /// device, and returns the producer alongside a shared NT handle for
        /// it. The handle belongs to the caller.
        fn new() -> Result<(Self, windows::Win32::Foundation::HANDLE)> {
            let mut device = None;
            let mut device_context = None;
            unsafe {
                D3D11CreateDevice(
                    // The default adapter, which is the one gpui picks as
                    // well. A producer that lands on a different adapter of a
                    // multi-GPU machine has its texture refused by the window.
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut device_context),
                )
            }
            .context("Creating the producing Direct3D device")?;
            let device = device.context("Direct3D device missing")?;
            let device_context = device_context.context("Direct3D device context missing")?;

            let desc = D3D11_TEXTURE2D_DESC {
                Width: TEXTURE_SIZE as u32,
                Height: TEXTURE_SIZE as u32,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: (D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0 | D3D11_RESOURCE_MISC_SHARED.0)
                    as u32,
            };
            let mut texture = None;
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
                .context("Creating the shared texture")?;
            let texture = texture.context("Shared texture missing")?;

            let pixels = picture();
            unsafe {
                device_context.UpdateSubresource(
                    &texture,
                    0,
                    None,
                    pixels.as_ptr() as *const _,
                    (TEXTURE_SIZE * 4) as u32,
                    0,
                );
                // Without this the other device may open the texture before the
                // picture has landed in it.
                device_context.Flush();
            }

            let resource: IDXGIResource1 = texture.cast().context("Casting to IDXGIResource1")?;
            let handle = unsafe {
                resource.CreateSharedHandle(
                    None,
                    DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0,
                    None,
                )
            }
            .context("Creating a shared handle for the texture")?;

            Ok((
                Self {
                    _device: device,
                    _texture: texture,
                },
                handle,
            ))
        }
    }

    struct SharedTextureExample {
        _producer: Producer,
        texture: SharedTextureId,
    }

    impl SharedTextureExample {
        fn new(window: &Window) -> Result<Self> {
            let (producer, handle) = Producer::new()?;
            let texture = window
                .register_shared_texture(
                    handle,
                    size(DevicePixels(TEXTURE_SIZE), DevicePixels(TEXTURE_SIZE)),
                )
                .context("Registering the shared texture with the window")?;
            Ok(Self {
                _producer: producer,
                texture,
            })
        }

        /// A square that paints `source` of the shared texture, sized so that
        /// a full-texture source lands one texel per device pixel.
        fn surface(
            &self,
            source: Bounds<DevicePixels>,
            scale_factor: f32,
        ) -> impl IntoElement + use<> {
            let texture = self.texture;
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    if !window.paint_surface(bounds, texture, source) {
                        // A device loss the texture could not be opened again
                        // across. A producer would share a new handle here.
                        eprintln!("the shared texture is gone");
                    }
                },
            )
            .size(px(TEXTURE_SIZE as f32 / scale_factor))
        }
    }

    impl Render for SharedTextureExample {
        fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let scale_factor = window.scale_factor();
            let whole = Bounds {
                origin: point(DevicePixels(0), DevicePixels(0)),
                size: size(DevicePixels(TEXTURE_SIZE), DevicePixels(TEXTURE_SIZE)),
            };
            let quadrant = Bounds {
                origin: point(
                    DevicePixels(TEXTURE_SIZE / 2),
                    DevicePixels(TEXTURE_SIZE / 2),
                ),
                size: size(
                    DevicePixels(TEXTURE_SIZE / 2),
                    DevicePixels(TEXTURE_SIZE / 2),
                ),
            };

            div()
                .flex()
                .flex_col()
                .gap_4()
                .p_4()
                .size_full()
                .bg(rgb(0x1d4e89))
                .text_color(rgb(0xffffff))
                .child("A texture from another Direct3D device, drawn in the scene")
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            div()
                                .relative()
                                .child(self.surface(whole, scale_factor))
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(48.))
                                        .top(px(96.))
                                        .p_2()
                                        .bg(rgba(0xff8800aa))
                                        .rounded_md()
                                        .child("above the surface"),
                                ),
                        )
                        .child(
                            div()
                                .overflow_hidden()
                                .size(px(TEXTURE_SIZE as f32 / scale_factor / 2.))
                                .child(self.surface(quadrant, scale_factor)),
                        ),
                )
                .child(
                    "Left: the whole texture, one texel per device pixel. \
                     Right: its bottom right quarter, stretched, clipped to half the square.",
                )
        }
    }

    pub fn run() {
        application().run(|cx: &mut App| {
            if !example_support::load_fonts(cx) {
                return;
            }
            let bounds = Bounds::centered(None, size(px(720.), px(480.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|_| {
                        SharedTextureExample::new(window).expect("opening the shared texture")
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
    }
}

#[cfg(target_os = "windows")]
fn main() {
    example::run();
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("Shared textures are a Windows feature; this example does nothing here.");
}
