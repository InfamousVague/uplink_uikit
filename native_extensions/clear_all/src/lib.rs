use common::icons::outline::Shape as Icon;
use dioxus::prelude::*;
use extensions::{export_extension, Details, Extension, Location, Meta, Type};
use kit::elements::button::Button;
use once_cell::sync::Lazy;

// These two lines are all you need to use your Extension implementation as a shared library
static EXTENSION: Lazy<ClearAll> = Lazy::new(|| ClearAll {});
export_extension!(EXTENSION);

pub struct ClearAll;

impl Extension for ClearAll {
    fn details(&self) -> Details {
        Details {
            location: Location::Sidebar,
            ext_type: Type::SimpleAction,
            meta: Meta {
                name: "clear_all",
                pretty_name: "Clear All Notifications",
                description: "Clears all notifications with a single click.",
                author: "Satellite <devs@satellite.im>",
            },
        }
    }

    fn stylesheet(&self) -> String {
        ".clear-all_container, .clear-all_container .btn-wrap, .clear-all_container .btn { width: 100% }".into()
    }

    fn render<'a>(&self, runtime: std::rc::Rc<Runtime>) -> Element {
        use_hook(move || Rc::new(RuntimeGuard::new(runtime.clone())));
        let styles = self.stylesheet();

        rsx! (
            style { "{styles}" },
            // Render standard (required) button to fire action.
            div {
                class: "clear-all_container",
                Button {
                    icon: Icon::BellSlash,
                    text: "Clear Notis".into(),
                    onpress: move |_| {
                        // TODO: Clear all notifications in state.
                    }
                }
            }
        ))
    }
}
