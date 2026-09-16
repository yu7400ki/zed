#![cfg_attr(target_family = "wasm", no_main)]

#[path = "example_support/fonts.rs"]
mod example_support;

use gpui::{
    App, Bounds, Context, MouseButton, MouseUpEvent, Pixels, SharedString, Window, WindowBounds,
    WindowOptions, div, point, prelude::*, px, rgb, size,
};
use gpui_platform::application;

struct PlacementExample {
    bounds: Bounds<Pixels>,
    maximized: bool,
    reports: usize,
    _observer: gpui::Subscription,
}

impl PlacementExample {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let observer = cx.observe_window_bounds(window, |example, window, cx| {
            example.bounds = window.bounds();
            example.maximized = window.is_maximized();
            example.reports += 1;
            cx.notify();
        });

        Self {
            bounds: window.bounds(),
            maximized: window.is_maximized(),
            reports: 0,
            _observer: observer,
        }
    }
}

fn button(
    label: &str,
    on_click: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(label.to_owned()))
        .px_4()
        .py_2()
        .bg(rgb(0x3f3f46))
        .rounded_md()
        .child(label.to_owned())
        .on_mouse_up(MouseButton::Left, on_click)
}

impl Render for PlacementExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_8()
            .size_full()
            .items_center()
            .justify_center()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xffffff))
            .child(format!(
                "origin: {}, {} size: {}, {}",
                self.bounds.origin.x,
                self.bounds.origin.y,
                self.bounds.size.width,
                self.bounds.size.height
            ))
            .child(format!(
                "maximized: {} bounds reports: {}",
                self.maximized, self.reports
            ))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(button("Move right", |_, window, _| {
                        let origin = window.bounds().origin + point(px(40.), px(0.));
                        window.set_window_position(origin);
                    }))
                    .child(button("Maximize", |_, window, _| window.zoom_window()))
                    .child(button("Unzoom", |_, window, _| window.unzoom_window())),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        if !example_support::load_fonts(cx) {
            return;
        }

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(520.), px(320.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| PlacementExample::new(window, cx)),
        )
        .unwrap();
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}
