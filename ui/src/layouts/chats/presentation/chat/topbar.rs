use dioxus::prelude::*;
use futures::{channel::oneshot, StreamExt};
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        indicator::Status,
        user_image::UserImage,
        user_image_group::UserImageGroup,
    },
    elements::{
        button::Button,
        input::{Input, Options},
        Appearance,
    },
};

use common::{
    icons::outline::Shape as Icon,
    language::get_local_text_with_args,
    state::Action,
    warp_runner::{RayGunCmd, WarpCmd},
};
use common::{state::State, WARP_CMD_CH};

use common::language::get_local_text;

use uuid::Uuid;
use warp::raygun::{ConversationSettings, ConversationType};

use tracing::log;

use crate::{
    layouts::chats::data::{get_input_options, ChatData, ChatProps},
    utils::build_participants,
};

enum EditGroupCmd {
    UpdateGroupName((Uuid, String)),
}

pub fn get_topbar_children(props: ChatProps) -> Element {
    let mut state = use_context::<Signal<State>>();

    let chat_data = use_context::<Signal<ChatData>>();
    let mut show_group_users_signal = props.show_group_users.clone();
    let mut show_rename_group_signal = props.show_rename_group.clone();
    let mut show_manage_members_signal = props.show_manage_members.clone();
    let mut show_group_settings_signal = props.show_group_settings.clone();

    let ch = use_coroutine(|mut rx: UnboundedReceiver<EditGroupCmd>| async move {
        let warp_cmd_tx = WARP_CMD_CH.tx.clone();
        while let Some(cmd) = rx.next().await {
            match cmd {
                EditGroupCmd::UpdateGroupName((conv_id, new_conversation_name)) => {
                    let (tx, rx) = oneshot::channel();
                    if let Err(e) =
                        warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::UpdateConversationName {
                            conv_id,
                            new_conversation_name,
                            rsp: tx,
                        }))
                    {
                        log::error!("failed to send warp command: {}", e);
                        continue;
                    }
                    let res = rx.await.expect("command canceled");
                    if let Err(e) = res {
                        log::error!("failed to update group conversation name: {}", e);
                    }
                }
            }
        }
    });

    // todo: get rid of this data variable
    let data = chat_data.read();
    if !data.active_chat.is_initialized {
        return rsx!(
            UserImageGroup {
                loading: true,
                aria_label: "user-image-group".to_string(),
                participants: vec![]
            },
            div {
                class: "user-info",
                aria_label: "user-info",
                div {
                    class: "skeletal-bars",
                    div {
                        class: "skeletal skeletal-bar",
                    },
                    div {
                        class: "skeletal skeletal-bar",
                    },
                }
            }
        );
    }

    let conversation_title = data
        .active_chat
        .conversation_name()
        .unwrap_or(data.active_chat.other_participants_names());
    let show_group_list = props
        .show_group_users
        .read()
        .map(|group_chat_id| group_chat_id == chat_data.read().active_chat.id())
        .unwrap_or(false);

    let direct_message = data.active_chat.conversation_type() == ConversationType::Direct;
    let (show_manage_members, show_rename) = match data.active_chat.conversation_settings() {
        ConversationSettings::Group(group_settings) => (
            props.is_owner || group_settings.members_can_add_participants(),
            props.is_owner || group_settings.members_can_change_name(),
        ),
        ConversationSettings::Direct(_) => (false, true),
    };

    let active_participant = data.active_chat.my_id();
    let mut all_participants = data.active_chat.other_participants();
    all_participants.push(active_participant);
    let members_count = get_local_text_with_args(
        "uplink.members-count",
        vec![("num", all_participants.len())],
    );

    let conv_id = data.active_chat.id();
    let subtext = data.active_chat.subtext();

    let show_group_settings = match chat_data.read().active_chat.conversation_settings() {
        ConversationSettings::Group(_) => props.is_owner,
        ConversationSettings::Direct(_) => false,
    };

    rsx!(
        if direct_message {{rsx! (
            UserImage {
                loading: false,
                platform: data.active_chat.platform(),
                status: Status::from(data.active_chat.active_participant().identity_status()),
                image: data.active_chat.first_image(),
            }
        )}} else {{rsx! (
            UserImageGroup {
                loading: false,
                aria_label: "user-image-group".to_string(),
                participants: build_participants(&all_participants),
            }
        )}}
        ContextMenu {
            id: "chat_topbar_context".to_string(),
            fit_parent: true,
            key: "{props.channel.id}-channel",
            devmode: state.read().configuration.developer.developer_mode,
            items: rsx!(
                if direct_message {{rsx!(
                    ContextItem {
                        icon: Icon::XMark,
                        aria_label: "close-chat-context-option".to_string(),
                        text: "Close Chat".to_string(),
                        onpress: move |_| {
                            state.write().mutate(Action::RemoveFromSidebar(conv_id));
                        }
                    }
                )}} else {{rsx!(
                    if show_rename {{rsx!(
                        ContextItem {
                            icon: Icon::PencilSquare,
                            aria_label: "rename-group-context-option".to_string(),
                            text: "Rename".to_string(),
                            onpress: move |_| {
                                show_rename_group_signal.set(true);
                            }
                        }
                    )}}
                    if show_manage_members {{rsx!(
                        ContextItem {
                            icon: Icon::Users,
                            aria_label: "manage-members-context-option".to_string(),
                            text: "Manage Members".to_string(),
                            onpress: move |_| {
                                show_manage_members_signal.set(Some(chat_data.read().active_chat.id()));
                            }
                        }
                    )}}
                    if show_group_settings {{rsx!(
                        ContextItem {
                            danger: true,
                            icon: Icon::Cog,
                            aria_label: "group-settings-context-option".to_string(),
                            text: "Settings".to_string(),
                            onpress: move |_| {
                                show_group_settings_signal.set(true);
                            }
                        },
                    )}}
                    // TODO: `Delete` item
                )}}
            ),
            div {
                class: "user-info",
                aria_label: "user-info",
                onclick: move |_| {
                    if show_group_list && !direct_message {
                        show_group_users_signal.set(None);
                    } else if !direct_message {
                        show_group_users_signal.set(Some(chat_data.read().active_chat.id()));
                        show_rename_group_signal.set(false);
                    }
                },
                if props.show_rename_group.read().clone() {{rsx! (
                    div {
                        id: "edit-group-name",
                        class: "edit-group-name",
                        Input {
                            placeholder:  get_local_text("messages.group-name"),
                            default_text: conversation_title.clone(),
                            aria_label: "groupname-input".to_string(),
                            options: Options {
                                with_clear_btn: true,
                                ..get_input_options()
                            },
                            onreturn: move |(v, is_valid, _): (String, bool, _)| {
                                if !is_valid {
                                    return;
                                }
                                if v != conversation_title.clone() {
                                    ch.send(EditGroupCmd::UpdateGroupName((conv_id, v)));
                                }
                                show_rename_group_signal.set(false);
                            },
                        },
                        Button {
                            icon: Icon::XMark,
                            appearance: Appearance::Secondary,
                            onpress: move |_| show_rename_group_signal.set(false),
                            aria_label: "close-rename-group".to_string(),
                        }
                    })}
                } else {{rsx!(
                    p {
                        aria_label: "user-info-username",
                        class: "username",
                        "{conversation_title}"
                    },
                    p {
                        aria_label: "user-info-status",
                        class: "status",
                        if direct_message {
                            {rsx! (span {
                                "{subtext}"
                            })}
                        } else {
                            {rsx! (
                                span {"{members_count}"}
                            )}
                        }
                    }
                )}}
            }
        }
    )
}
