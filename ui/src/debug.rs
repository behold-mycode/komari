use backend::{capture_image, infer_minimap, infer_rune, record_images, test_spin_rune};
use dioxus::prelude::*;

use crate::button::{Button, ButtonKind};

#[component]
pub fn Debug() -> Element {
    let mut is_recording = use_signal(|| false);

    rsx! {
        div { class: "flex flex-col h-full overflow-y-auto scrollbar pr-4 pb-3",
            div { class: "grid grid-cols-2 gap-3",
                Button {
                    text: "Capture color image",
                    kind: ButtonKind::Secondary,
                    on_click: move |_| async {
                        capture_image(false).await;
                    },
                }
                Button {
                    text: "Capture grayscale image",
                    kind: ButtonKind::Secondary,
                    on_click: move |_| async {
                        capture_image(true).await;
                    },
                }
                Button {
                    text: "Infer rune",
                    kind: ButtonKind::Secondary,
                    on_click: move |_| async {
                        infer_rune().await;
                    },
                }
                Button {
                    text: "Infer minimap",
                    kind: ButtonKind::Secondary,
                    on_click: move |_| async {
                        infer_minimap().await;
                    },
                }
                Button {
                    text: "Spin rune sandbox test",
                    kind: ButtonKind::Secondary,
                    on_click: move |_| async {
                        test_spin_rune().await;
                    },
                }
                Button {
                    text: if is_recording() { "Stop recording" } else { "Start recording" },
                    kind: ButtonKind::Secondary,
                    on_click: move |_| async move {
                        let recording = *is_recording.peek();
                        is_recording.toggle();
                        record_images(!recording).await;
                    },
                }
            }
        }
    }
}
