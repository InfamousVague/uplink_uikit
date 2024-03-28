mod controls;
pub mod coroutines;
mod edit_group;
mod group_settings;
mod group_users;
mod pinned_messages;
mod topbar;

use dioxus::prelude::*;

use kit::{
    components::message_group::MessageGroupSkeletal,
    layout::{modal::Modal, topbar::Topbar},
};

use crate::{
    components::media::calling::CallControl,
    layouts::chats::{
        data::{self, ChatData, MessagesToEdit, MessagesToSend, ScrollBtn},
        presentation::{
            chat::{edit_group::EditGroup, group_settings::GroupSettings, group_users::GroupUsers},
            chatbar::get_chatbar,
            messages::get_messages,
        },
        scripts::{DISABLE_RELOAD, SHOW_CONTEXT, USER_TAG_SCRIPT},
    },
};

use common::state::{ui, Action, Identity, State};

use common::language::get_local_text;

use tracing::log;
use uuid::Uuid;
use warp::crypto::DID;

#[allow(non_snake_case)]
pub fn Compose(cx: Scope) -> Element {
    log::trace!("rendering compose");
    use_shared_state_provider(cx, ChatData::default);
    use_shared_state_provider(cx, ScrollBtn::new);
    use_shared_state_provider(cx, MessagesToSend::default);
    use_shared_state_provider(cx, MessagesToEdit::default);
    let state = use_shared_state::<State>(cx)?;
    let chat_data = use_shared_state::<ChatData>(cx)?;

    let init = coroutines::init_chat_data(cx, state, chat_data);
    coroutines::handle_warp_events(cx, state, chat_data);

    state.write_silent().ui.current_layout = ui::Layout::Compose;

    let show_manage_members: &UseState<Option<Uuid>> = use_state(cx, || None);
    let show_group_settings: &UseState<bool> = use_state(cx, || false);
    let show_rename_group: &UseState<bool> = use_state(cx, || false);
    let show_group_users: &UseState<Option<Uuid>> = use_state(cx, || None);

    let quick_profile_uuid = &*cx.use_hook(|| Uuid::new_v4().to_string());
    let quickprofile_data: &UseRef<Option<(f64, f64, Identity, bool)>> = use_ref(cx, || None);
    let update_script = use_state(cx, String::new);
    let identity_profile = use_state(cx, DID::default);

    let eval_provider = use_eval(cx);
    let script = DISABLE_RELOAD;
    let _ = eval_provider(script);
    // Handle user tag click
    // We handle it here since user tags are not dioxus components
    use_effect(cx, chat_data, |_| {
        to_owned![state, eval_provider, quickprofile_data];
        async move {
            if let Ok(eval) = eval_provider(USER_TAG_SCRIPT) {
                loop {
                    if let Ok(s) = eval.recv().await {
                        match serde_json::from_str::<(f64, f64, DID)>(
                            s.as_str().unwrap_or_default(),
                        ) {
                            Ok((x, y, did)) => {
                                if let Some(id) = state.read().get_identity(&did) {
                                    quickprofile_data.set(Some((x, y, id, false)));
                                }
                            }
                            Err(e) => {
                                log::error!("failed to deserialize message: {}: {}", s, e);
                            }
                        }
                    }
                }
            }
        }
    });
    use_effect(cx, quickprofile_data, |data| {
        to_owned![quick_profile_uuid, update_script, identity_profile];
        async move {
            if let Some((x, y, id, right)) = data.read().as_ref() {
                let script = SHOW_CONTEXT
                    .replace("UUID", &quick_profile_uuid)
                    .replace("$PAGE_X", &x.to_string())
                    .replace("$PAGE_Y", &y.to_string())
                    .replace("$SELF", &right.to_string());
                update_script.set(script);
                identity_profile.set(id.did_key());
            }
        }
    });

    // if the emoji picker is visible, autofocusing on the chatbar will close the emoji picker.
    let should_ignore_focus = state.read().ui.ignore_focus || state.read().ui.emoji_picker_visible;
    let creator = chat_data.read().active_chat.creator();

    let chat_id = chat_data.read().active_chat.id();
    let user_did: DID = state.read().did_key();
    let is_owner = creator.map(|id| id == user_did).unwrap_or_default();

    if init.value().is_some() {
        if let Some(chat) = state.read().get_active_chat() {
            let metadata = data::Metadata::new(&state.read(), &chat);
            if chat_data.read().active_chat.metadata_changed(&metadata) {
                // If the metadata has changed, we should cancel out all actions to modify it.
                if *show_rename_group.get() {
                    show_rename_group.set(false);
                }
                // Now we can continue
                if !*show_group_settings.get() && show_manage_members.get().is_none() {
                    chat_data.write().active_chat.set_metadata(metadata);
                }
            }
        }
    }

    cx.render(rsx!(
        div {
            id: "compose",
            Topbar {
                with_back_button: state.read().ui.is_minimal_view() && state.read().ui.sidebar_hidden,
                onback: move |_| {
                    let current = state.read().ui.sidebar_hidden;
                    state.write().mutate(Action::SidebarHidden(!current));
                },
                controls: cx.render(rsx!(controls::get_controls{
                    show_manage_members: show_manage_members.clone(),
                    show_rename_group: show_rename_group.clone(),
                    show_group_settings: show_group_settings.clone(),
                    show_group_users: show_group_users.clone(),
                    ignore_focus: should_ignore_focus,
                    is_owner: is_owner,
                })),
                topbar::get_topbar_children {
                    show_manage_members: show_manage_members.clone(),
                    show_rename_group: show_rename_group.clone(),
                    show_group_settings: show_group_settings.clone(),
                    show_group_users: show_group_users.clone(),
                    ignore_focus: should_ignore_focus,
                    is_owner: is_owner,
                }
            },
            // may need this later when video calling is possible.
            // data.as_ref().and_then(|data| data.active_media.then(|| rsx!(
            //     MediaPlayer {
            //         settings_text: get_local_text("settings.settings"),
            //         enable_camera_text: get_local_text("media-player.enable-camera"),
            //         fullscreen_text: get_local_text("media-player.fullscreen"),
            //         popout_player_text: get_local_text("media-player.popout-player"),
            //         screenshare_text: get_local_text("media-player.screenshare"),
            //         end_text: get_local_text("uplink.end"),
            //     },
            // ))),
        show_manage_members
            .map_or(false, |group_chat_id| (group_chat_id == chat_id)).then(|| rsx!(
                Modal {
                    open: show_manage_members.is_some(),
                    transparent: true,
                    with_title: get_local_text("friends.manage-group-members"),
                    onclose: move |_| {
                        show_manage_members.set(None);
                    },
                    right: "var(--gap)",
                    EditGroup {}
                }
            )),
        show_group_settings.then(|| rsx!(
                Modal {
                    open: *show_group_settings.get(),
                    transparent: true,
                    with_title: get_local_text("settings"),
                    onclose: move |_| {
                        show_group_settings.set(false);
                    },
                    right: "var(--gap)",
                    GroupSettings {}
                }
            )),
        show_group_users
            .map_or(false, |group_chat_id| (group_chat_id == chat_id)).then(|| rsx!(
                Modal {
                    open: show_group_users.is_some(),
                    right: "calc(100% - (var(--width-sidebar) * 2 ) - var(--padding-more))",
                    transparent: true,
                    with_title: get_local_text("friends.view-group"),
                    onclose: move |_| {
                        show_group_users.set(None);
                    },
                    GroupUsers {
                        active_chat: state.read().get_active_chat(),
                        quickprofile_data: quickprofile_data.clone(),
                    }
                }
        )),
        CallControl {
            in_chat: true
        },
        if init.value().is_none() {
           rsx!(
                div {
                    id: "messages",
                    MessageGroupSkeletal {},
                    MessageGroupSkeletal { alt: true },
                    MessageGroupSkeletal {},
                }
            )
        } else {
            rsx!(get_messages{quickprofile_data: quickprofile_data.clone()})
        },
        get_chatbar {
            show_manage_members: show_manage_members.clone(),
            show_rename_group: show_rename_group.clone(), // TODO: wire this to a context item when right clicking the topbar.
            show_group_settings: show_group_settings.clone(),
            show_group_users: show_group_users.clone(),
            ignore_focus: should_ignore_focus,
            is_owner: is_owner,
        },
        super::quick_profile::QuickProfileContext{
            id: quick_profile_uuid,
            update_script: update_script,
            did_key: identity_profile,
        }
    }
    ))
}
