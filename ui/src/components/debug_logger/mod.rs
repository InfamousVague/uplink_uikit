use dioxus::prelude::*;

use common::{icons::outline::Shape as Icon, state::State};
use kit::elements::{
    button::Button,
    label::Label,
    tooltip::{ArrowPosition, Tooltip},
    Appearance,
};

use dioxus_desktop::use_window;

use crate::logger;

const STYLE: &str = include_str!("./style.scss");
const SCRIPT: &str = include_str!("./script.js");

#[derive(Clone, PartialEq, Eq, Copy)]
pub enum LogLevel {
    Info,
    Error,
    Debug,
}

#[component]
#[allow(non_snake_case)]
pub fn DebugLogger(cx: Scope) -> Element {
    let window = use_window(cx);

    let logs_to_show = use_state(cx, logger::load_debug_log);

    use_future(cx, (), |_| {
        to_owned![logs_to_show];
        async move {
            let mut log_ch = logger::subscribe();
            while let Some(log) = log_ch.recv().await {
                logs_to_show.with_mut(|x| x.push(log.to_string()));
            }
        }
    });

    let eval = use_eval(cx);

    let active_tab: &UseState<String> = use_state(cx, || "Logs".into());
    let filter_level: &UseState<LogLevel> = use_state(cx, || LogLevel::Info);

    let state: &UseSharedState<State> = use_shared_state::<State>(cx)?;

    let state_json = state.read().get_json().unwrap_or_default();

    cx.render(rsx!(
        style { STYLE }
        div {
            onmounted: move |_| { _ = eval(SCRIPT) },
            id: "debug_logger",
            aria_label: "debug-logger",
            class: "debug-logger resize-vert-top",
            div {
                class: "header",
                aria_label: "debug-logger-header",
                div {
                    class: "logger-nav",
                    aria_label: "debug-logger-nav",
                    Button {
                        text: "Logs".into(),
                        icon: Icon::CommandLine,
                        appearance: if active_tab.get() == "Logs" { Appearance::Primary } else { Appearance::Secondary },
                        onpress: |_| {
                            active_tab.set("Logs".into());
                        }
                    },
                    (active_tab.get() == "Logs").then(|| cx.render(rsx!{
                        div {
                            class: "section",
                            Label {
                                text: "Filter:".into(),
                            },
                            Button {
                                icon: Icon::InformationCircle,
                                appearance: if filter_level.get() == &LogLevel::Info { Appearance::Info } else { Appearance::Secondary },
                                onpress: |_| {
                                    filter_level.set(LogLevel::Info);
                                },
                                tooltip: cx.render(rsx!(
                                    Tooltip {
                                        arrow_position: ArrowPosition::Top,
                                        text: "Info".into()
                                    }
                                )),
                            },
                            Button {
                                icon: Icon::ExclamationTriangle,
                                appearance: if filter_level.get() == &LogLevel::Error { Appearance::Danger } else { Appearance::Secondary },
                                onpress: |_| {
                                    filter_level.set(LogLevel::Error);
                                },
                                tooltip: cx.render(rsx!(
                                    Tooltip {
                                        arrow_position: ArrowPosition::Top,
                                        text: "Error".into()
                                    }
                                )),
                            },
                            Button {
                                icon: Icon::BugAnt,
                                appearance: if filter_level.get() == &LogLevel::Debug { Appearance::Secondary } else { Appearance::Secondary },
                                onpress: |_| {
                                    filter_level.set(LogLevel::Debug);
                                },
                                tooltip: cx.render(rsx!(
                                    Tooltip {
                                        arrow_position: ArrowPosition::Top,
                                        text: "Debug".into()
                                    }
                                )),
                            },
                        }
                    })),
                    Button {
                        text: "State".into(),
                        icon: Icon::Square3Stack3d,
                        appearance: if active_tab.get() == "State" { Appearance::Primary } else { Appearance::Secondary },
                        onpress: |_| {
                            active_tab.set("State".into());
                        }
                    },
                    Button {
                        text: "Web Inspector".into(),
                        icon: Icon::ArrowTopRightOnSquare,
                        appearance: Appearance::Secondary,
                        onpress: |_| {
                            window.webview.open_devtools();
                        }
                    },
                }
            },
            match active_tab.get().as_str() {
                "Logs" => rsx!(div {
                    aria_label: "debug-logger-body",
                    class: "body",
                    div {
                        logs_to_show.iter().map(|log| {
                            let mut fields = log.split('|');
                            let log_datetime = fields.next().unwrap_or_default();
                            let log_level = fields.next().unwrap_or_default();
                            let log_message = fields.next().unwrap_or_default();
                            let log_level_string = log_level.trim().to_lowercase();
                            rsx!(
                                p {
                                    class: "item",
                                    aria_label: "debug-logger-item",
                                    span {
                                        aria_label: "debug-logger-item-timestamp",
                                        class: "log-text muted",
                                        "{log_datetime}"
                                    },
                                    span {
                                        aria_label: "debug-logger-item-level",
                                        class: "log-text bold {log_level_string}",
                                        "{log_level}"
                                    },
                                    span {
                                        class: "log-text muted",
                                        "»"
                                    }
                                    span {
                                        aria_label: "debug-logger-item-text",
                                        id: "log_text",
                                        class: "log-text",
                                        " {log_message}"
                                    }
                                }
                            )
                        })
                    }
                }),
                "State" => rsx!(div {
                    aria_label: "debug-logger-body",
                    class: "body",
                    pre {
                        class: "language-js",

                        code {
                            "{state_json}"
                        }
                    }
                    script {
                        r#"
                        (() => {{
                            Prism.highlightAll();
                        }})();
                        "#
                    }
                }),
                _ => rsx!(div { "Unknown tab" }),
            }
        },
    ))
}
