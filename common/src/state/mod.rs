pub mod action;
pub mod call;
pub mod chats;
pub mod configuration;
pub mod data_transfer;
pub mod default_keybinds;
pub mod friends;
pub mod identity;
pub mod notifications;
pub mod pending_message;
pub mod route;
pub mod scope_ids;
pub mod settings;
pub mod storage;
pub mod ui;
pub mod utils;

use crate::language::{change_language, get_local_text_with_args};
use crate::notifications::NotificationAction;
use crate::warp_runner::WarpCmdTx;
// export specific structs which the UI expects. these structs used to be in src/state.rs, before state.rs was turned into the `state` folder
use crate::{language::get_local_text, warp_runner::ui_adapter};
pub use action::Action;
pub use chats::{Chat, Chats};
use dioxus_desktop::tao::window::WindowId;
pub use friends::Friends;
pub use identity::Identity;
pub use route::Route;
pub use settings::Settings;
pub use ui::{Theme, ToastNotification, UI};
use warp::blink::BlinkEventKind;
use warp::multipass::identity::Platform;
use warp::raygun::{ConversationType, Location};

use crate::STATIC_ARGS;

use crate::{
    testing::mock::generate_mock,
    warp_runner::{
        ui_adapter::{MessageEvent, MultiPassEvent, RayGunEvent},
        WarpEvent,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

use std::{
    collections::{BTreeMap, HashMap},
    fmt, fs,
    time::{Duration, Instant},
};
use uuid::Uuid;
use warp::{crypto::DID, multipass::identity::IdentityStatus, raygun};

use tracing::log;

use self::call::Call;
use self::pending_message::{FileLocation, FileProgression, PendingMessage};

use self::storage::Storage;
use self::ui::{Font, Layout};
use self::utils::get_available_themes;

pub const MAX_PINNED_MESSAGES: u8 = 100;
// todo: create an Identity cache and only store UUID in state.friends and state.chats
// store the following information in the cache: key: DID, value: { Identity, HashSet<UUID of conversations this identity is participating in> }
// the HashSet would be used to determine when to evict an identity. (they are not participating in any conversations and are not a friend)
#[derive(Default, Deserialize, Serialize)]
pub struct State {
    #[serde(skip)]
    id: DID,
    pub route: route::Route,
    chats: chats::Chats,
    friends: friends::Friends,
    #[serde(skip)]
    pub storage: storage::Storage,
    #[serde(skip)]
    pub scope_ids: scope_ids::ScopeIds,
    pub settings: settings::Settings,
    pub ui: ui::UI,
    pub configuration: configuration::Configuration,
    #[serde(skip)]
    identities: HashMap<DID, identity::Identity>,
    #[serde(skip)]
    pub initialized: bool,
    #[serde(skip)]
    warp_cmd_tx: Option<WarpCmdTx>,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("id", &self.did_key())
            .field("route", &self.route)
            .field("chats", &self.chats)
            .field("friends", &self.friends)
            .finish()
    }
}

// todo: why is there clone impl which returns a mutated value?
impl Clone for State {
    fn clone(&self) -> Self {
        State {
            id: self.did_key(),
            route: self.route.clone(),
            chats: self.chats.clone(),
            friends: self.friends.clone(),
            storage: self.storage.clone(),
            settings: Default::default(),
            scope_ids: Default::default(),
            ui: Default::default(),
            configuration: self.configuration.clone(),
            identities: HashMap::new(),
            initialized: self.initialized,
            warp_cmd_tx: None,
        }
    }
}

// This code defines a number of methods for the State struct, which are used to mutate the state in a controlled manner.
// For example, the set_active_chat method sets the active chat in the State struct, and the toggle_favorite method adds or removes a chat from the user's favorites.
//  These methods are used to update the relevant fields within the State struct in response to user actions or other events within the application.
impl State {
    /// Constructs a new `State` instance with default values.
    /// use state::load() instead
    #[deprecated]
    pub fn new() -> Self {
        State::default()
    }

    // gonna try adding this here to let shared libraries send warp commands.
    pub fn get_warp_ch(&self) -> WarpCmdTx {
        self.warp_cmd_tx
            .clone()
            .expect("dev needs to call set_warp_ch before get_warp_ch could ever be used")
    }

    pub fn set_warp_ch(&mut self, ch: WarpCmdTx) {
        self.warp_cmd_tx.replace(ch);
    }

    pub fn mutate(&mut self, action: Action) {
        // ignore noisy events
        if !matches!(action, Action::SetChatDraft(_, _)) {
            log::debug!("state::mutate: {}", action);
        }

        match action {
            Action::SetAccentColor(color) => {
                self.ui.accent_color = Some(color);
            }
            Action::ClearAccentColor => {
                self.ui.accent_color = None;
            }
            Action::SetDevSettings(enabled) => {
                self.set_show_dev_settings(enabled);
            }
            Action::SetExtensionEnabled(extension, enabled) => {
                if enabled {
                    self.ui.extensions.enable(extension);
                } else {
                    self.ui.extensions.disable(extension);
                }
            }
            Action::RegisterExtensions(extensions) => {
                for (name, ext) in extensions {
                    self.ui.extensions.insert(
                        name.clone(),
                        ext,
                        self.configuration.extensions.enable_automatically
                            || name.eq("emoji_selector"),
                    );
                }
            }
            // ===== Notifications =====
            Action::AddNotification(kind, count, forced) => self.ui.notifications.increment(
                &self.configuration,
                kind,
                count,
                forced || !self.ui.metadata.focused,
            ),
            Action::RemoveNotification(kind, count) => self.ui.notifications.decrement(kind, count),
            Action::ClearNotification(kind) => self.ui.notifications.clear_kind(kind),
            Action::ClearAllNotifications => self.ui.notifications.clear_all(),
            Action::AddToastNotification(notification) => {
                self.ui
                    .toast_notifications
                    .insert(Uuid::new_v4(), notification);
            }
            Action::DismissUpdate => {
                self.settings.update_dismissed = self.settings.update_available.take();
                self.ui
                    .notifications
                    .decrement(notifications::NotificationKind::Settings, 1);
            }
            // ===== Friends =====
            Action::SendRequest(identity) => self.new_outgoing_request(&identity),
            Action::RequestAccepted(identity) => self.complete_request(&identity),
            Action::CancelRequest(identity) => self.cancel_request(identity),
            //Action::IncomingRequest(identity) => self.new_incoming_request(&identity),
            Action::AcceptRequest(identity) => self.complete_request(identity),
            Action::DenyRequest(identity) => self.cancel_request(identity),
            Action::RemoveFriend(friend) => self.remove_friend(friend),
            Action::Block(identity) => self.block(identity),
            Action::Unblock(identity) => self.unblock(identity),
            // ===== UI =====
            // Favorites
            Action::Favorite(chat) => self.favorite(&chat),
            Action::ToggleFavorite(chat) => self.toggle_favorite(chat),
            Action::UnFavorite(chat_id) => self.unfavorite(chat_id),
            // Language
            Action::SetLanguage(language) => self.set_language(&language),
            // Overlay
            Action::AddOverlay(window) => self.ui.overlays.push(window),
            Action::SetOverlay(enabled) => self.toggle_overlay(enabled),
            // Sidebar
            Action::RemoveFromSidebar(chat_id) => self.remove_sidebar_chat(chat_id),
            Action::SidebarHidden(hidden) => self.ui.sidebar_hidden = hidden,
            // Navigation
            Action::Navigate(to) => self.set_active_route(to),
            // Generic UI
            Action::SetMeta(metadata) => self.ui.metadata = metadata,
            Action::ClearCallPopout(window) => self.ui.clear_call_popout(&window),
            Action::SetCallPopout(webview) => self.ui.set_call_popout(webview),
            // Development
            Action::SetDebugLogger(webview) => self.ui.set_debug_logger(webview),
            Action::ClearDebugLogger(window) => self.ui.clear_debug_logger(&window),
            Action::AddFilePreview(id, window_id) => self.ui.add_file_preview(id, window_id),
            Action::ForgetFilePreview(id) => {
                let _ = self.ui.file_previews.remove(&id);
            }
            Action::ClearAllPopoutWindows(window) => self.ui.clear_all_popout_windows(&window),
            Action::TrackEmojiUsage(emoji) => self.ui.track_emoji_usage(emoji),
            Action::SetEmojiDestination(destination) => self.ui.emoji_destination = destination,
            Action::SetEmojiPickerVisible(visible) => self.ui.emoji_picker_visible = visible,
            Action::SetTransformMarkdownText(flag) => self.ui.transform_markdown_text(flag),
            Action::SetTransformAsciiEmojis(flag) => self.ui.transform_ascii_emojis(flag),
            // ===== Settings =====
            Action::PauseGlobalKeybinds(b) => self.settings.pause_global_keybinds = b,
            Action::ResetKeybinds => {
                self.settings.keybinds = default_keybinds::get_default_keybinds()
            }
            // Themes
            Action::SetTheme(theme) => self.set_theme(theme),
            // Fonts
            Action::SetFont(font) => self.set_font(font),
            Action::SetFontScale(font_scale) => self.settings.set_font_scale(font_scale),

            // ===== Chats =====
            Action::ChatWith(chat, should_move_to_top) => {
                // warning: ensure that warp is used to get/create the chat which is passed in here
                //todo: check if (for the side which created the conversation) a warp event comes in and consider using that instead
                self.set_active_chat(chat, should_move_to_top);
            }
            Action::ClearActiveChat => {
                self.clear_active_chat();
            }
            Action::StartReplying(chat, message) => self.start_replying(chat, message),
            Action::CancelReply(chat_id) => self.cancel_reply(chat_id),
            Action::ClearUnreads(id) => self.clear_unreads(id),
            Action::ClearActiveUnreads => {
                if let Some(id) = self.chats.active {
                    self.clear_unreads(id);
                }
            }
            Action::ClearAllUnreads => self.chats.all.values_mut().for_each(|c| c.clear_unreads()),
            Action::SetChatDraft(chat_id, value) => self.set_chat_draft(&chat_id, value),
            Action::ClearChatDraft(chat_id) => self.clear_chat_draft(&chat_id),
            Action::SetChatAttachments(chat_id, value) => {
                self.set_chat_attachments(&chat_id, value)
            }
            Action::AppendChatAttachments(chat_id, value) => {
                if value.is_empty() {
                    return;
                }
                let new_files: Vec<Location> = value
                    .iter()
                    .map(|path| Location::Disk { path: path.clone() })
                    .collect();
                let mut current_files: Vec<_> = self
                    .get_active_chat()
                    .map(|f| f.files_attached_to_send)
                    .unwrap_or_default()
                    .drain(..)
                    .filter(|x| !new_files.contains(x))
                    .collect();
                current_files.extend(new_files);
                self.set_chat_attachments(&chat_id, current_files)
            }
            Action::ClearChatAttachments(chat_id) => self.clear_chat_attachments(&chat_id),
            Action::AddReaction(_, _, emoji) => {
                self.ui.emojis.increment_emoji(emoji);
            }
            Action::RemoveReaction(_, _, _) => todo!(),
            Action::MockSend(id, msg) => {
                let sender = self.did_key();
                let mut m = raygun::Message::default();
                m.set_conversation_id(id);
                m.set_sender(sender);
                m.set_lines(msg);
                let m = ui_adapter::Message::new(m, None, Uuid::new_v4().to_string());
                self.add_msg_to_chat(id, m);
            }
            // ===== Media =====
            Action::ToggleMute => self.toggle_mute(),
            Action::ToggleSilence => self.toggle_silence(),
            Action::SetId(identity) => self.set_own_identity(identity),
            Action::AnswerCall(id) => match self
                .ui
                .call_info
                .answer_call(id, Some(self.get_own_identity().did_key())) //Update call with own did immediately
            {
                Ok(call) => {
                    self.set_active_media(call.conversation_id);
                    self.send_chat_to_top_of_sidebar(call.conversation_id);
                }
                Err(e) => {
                    log::error!("failed to answer call: {e}");
                }
            },
            Action::RejectCall(id) => self.ui.call_info.reject_call(id),
            Action::OfferCall(call) => {
                let _ = self.ui.call_info.pending_call(
                    call.id,
                    call.conversation_id,
                    call.participants,
                );
                let _ = self.ui.call_info.answer_call(call.id, None);
                self.set_active_chat(&call.conversation_id, true);
                self.set_active_media(call.conversation_id);
            }
            Action::EndCall => {
                self.chats.active_media = None;
                self.ui.popout_media_player = false;
                self.ui.call_info.end_call();
            }
            // ===== Configuration =====
            Action::Config(action) => self.configuration.mutate(action),
        }

        let _ = self.save();
    }

    pub fn clear(&mut self) {
        self.chats = chats::Chats::default();
        self.friends = friends::Friends::default();
        self.settings = settings::Settings::default();
    }

    pub fn process_warp_event(&mut self, event: WarpEvent) {
        // Blink events are very frequent
        if !matches!(
            event,
            WarpEvent::Blink(BlinkEventKind::ParticipantSpeaking { .. })
                | WarpEvent::Blink(BlinkEventKind::SelfSpeaking)
                | WarpEvent::Message(MessageEvent::TypingIndicator { .. })
        ) {
            log::debug!("process_warp_event: {event}");
        } else if matches!(
            event,
            WarpEvent::Message(MessageEvent::TypingIndicator { .. })
        ) {
            log::trace!("process_warp_event: {event}");
        }

        match event {
            WarpEvent::MultiPass(evt) => self.process_multipass_event(evt),
            WarpEvent::RayGun(evt) => self.process_raygun_event(evt),
            WarpEvent::Message(evt) => self.process_message_event(evt),
            WarpEvent::Blink(evt) => self.process_blink_event(evt),
        };

        let _ = self.save();
    }

    fn process_multipass_event(&mut self, event: MultiPassEvent) {
        match event {
            MultiPassEvent::None => {}
            MultiPassEvent::FriendRequestReceived(identity) => {
                self.new_incoming_request(&identity);

                self.mutate(Action::AddNotification(
                    notifications::NotificationKind::FriendRequest,
                    1,
                    false,
                ));

                // TODO: Get state available in this scope.
                // Dispatch notifications only when we're not already focused on the application.
                let notifications_enabled = self.configuration.notifications.friends_notifications;

                if !self.ui.metadata.focused && notifications_enabled {
                    crate::notifications::push_notification(
                        get_local_text("friends.new-request"),
                        get_local_text_with_args(
                            "friends.new-request-name",
                            vec![("name", identity.username())],
                        ),
                        Some(crate::sounds::Sounds::Notification),
                        notify_rust::Timeout::Milliseconds(4),
                        NotificationAction::FriendListPending,
                    );
                }
            }
            MultiPassEvent::FriendRequestSent(identity) => {
                self.new_outgoing_request(&identity);
            }
            MultiPassEvent::FriendAdded(identity) => {
                self.complete_request(&identity);
            }
            MultiPassEvent::FriendRemoved(identity) => {
                self.friends.all.remove(&identity.did_key());
            }
            MultiPassEvent::FriendRequestCancelled(identity) => {
                self.cancel_request(&identity.did_key());
            }
            MultiPassEvent::FriendOnline(identity) => {
                if let Some(ident) = self.identities.get_mut(&identity.did_key()) {
                    ident.set_identity_status(identity.identity_status());
                }
            }
            MultiPassEvent::FriendOffline(identity) => {
                if let Some(ident) = self.identities.get_mut(&identity.did_key()) {
                    ident.set_identity_status(IdentityStatus::Offline);
                }
            }
            MultiPassEvent::Blocked(identity) => {
                self.block(&identity.did_key());
            }
            MultiPassEvent::Unblocked(identity) => {
                self.unblock(&identity.did_key());
            }
            MultiPassEvent::IdentityUpdate(identity) => {
                self.update_identity(identity.did_key(), identity);
            }
        }
    }

    fn process_raygun_event(&mut self, event: RayGunEvent) {
        match event {
            RayGunEvent::ConversationCreated(chat) => {
                if !self.chats.in_sidebar.contains(&chat.inner.id) {
                    self.chats.in_sidebar.insert(0, chat.inner.id);
                    self.identities.extend(
                        chat.identities
                            .iter()
                            .map(|ident| (ident.did_key(), ident.clone())),
                    );
                }
                self.chats.all.insert(chat.inner.id, chat.inner);
            }
            RayGunEvent::ConversationDeleted(id) => {
                self.chats.in_sidebar.retain(|x| *x != id);
                self.chats.all.remove(&id);
                if self.chats.active == Some(id) {
                    self.chats.active = None;
                }
            }
        }
    }

    fn process_message_event(&mut self, event: MessageEvent) {
        match event {
            MessageEvent::Received {
                conversation_id,
                mut message,
            } => {
                let own = self.get_own_identity().did_key();
                let ping = message.is_mention_self(&own);
                self.update_identity_status_hack(&message.inner.sender());
                let id = self.identities.get(&message.inner.sender()).cloned();
                // todo: don't load all the messages by default. if the user scrolled up, for example, this incoming message may not need to be fetched yet.
                self.add_msg_to_chat(conversation_id, message);

                //if self.chats.in_sidebar.contains(&conversation_id) {
                self.send_chat_to_top_of_sidebar(conversation_id);
                //}

                self.mutate(Action::AddNotification(
                    notifications::NotificationKind::Message,
                    1,
                    ping,
                ));

                // Dispatch notifications only when we're not already focused on the application.
                let message_notifications_enabled =
                    self.configuration.notifications.messages_notifications;
                let notifications_enabled = self.configuration.notifications.enabled;
                let should_play_sound = self.ui.current_layout != Layout::Compose
                    && self.configuration.audiovideo.message_sounds;
                let should_dispatch_notification =
                    should_play_sound && message_notifications_enabled && notifications_enabled;

                // This should be called if we have notifications enabled for new messages
                if should_dispatch_notification {
                    let sound = if self.configuration.audiovideo.message_sounds {
                        Some(crate::sounds::Sounds::Notification)
                    } else {
                        None
                    };
                    let text = match id {
                        Some(id) => get_local_text_with_args(
                            "messages.user-sent-message",
                            vec![("user", id.username())],
                        ),
                        None => get_local_text("messages.unknown-sent-message"),
                    };
                    crate::notifications::push_notification(
                        get_local_text("messages.new"),
                        text,
                        sound,
                        notify_rust::Timeout::Milliseconds(4),
                        NotificationAction::DisplayChat(conversation_id),
                    );
                }
            }
            MessageEvent::Sent {
                conversation_id,
                message,
            } => {
                // todo: don't load all the messages by default. if the user scrolled up, for example, this incoming message may not need to be fetched yet.
                let message_clone = message.clone();
                if let Some(chat) = self.chats.all.get_mut(&conversation_id) {
                    chat.messages.push_back(message);
                }
                self.send_chat_to_top_of_sidebar(conversation_id);
                self.decrement_outgoing_messages(conversation_id, message_clone.inner.id());
            }
            MessageEvent::Edited {
                conversation_id,
                mut message,
            } => {
                self.update_identity_status_hack(&message.inner.sender());
                let own = self.get_own_identity().did_key();
                if let Some(chat) = self.chats.all.get_mut(&conversation_id) {
                    let id = message.inner.id();
                    if let Some(msg) = chat.messages.iter_mut().find(|msg| msg.inner.id() == id) {
                        *msg = message.clone();
                    }

                    if let Some(msg) = chat.replying_to.as_mut() {
                        if msg.id() == message.inner.id() {
                            *msg = message.inner.clone()
                        }
                    }

                    if let Some(msg) = chat.pinned_messages.iter_mut().find(|m| m.id() == id) {
                        *msg = message.inner.clone();
                    }

                    if message.is_mention_self(&own) {
                        if let Some(msg) = chat.mentions.iter_mut().find(|m| m.inner.id() == id) {
                            *msg = message.clone();
                        }
                    } else {
                        chat.mentions.retain(|m| m.inner.id() != id);
                    }
                }
            }
            MessageEvent::Deleted {
                conversation_id,
                message_id,
                most_recent_message,
            } => {
                // can't have 2 mutable borrows
                let mut should_decrement_notifications = false;
                if let Some(chat) = self.chats.all.get_mut(&conversation_id) {
                    if chat.remove_unread(&message_id) {
                        should_decrement_notifications = true;
                    }
                    chat.messages.retain(|msg| msg.inner.id() != message_id);
                    chat.pinned_messages.retain(|msg| msg.id() != message_id);
                    chat.mentions.retain(|msg| msg.inner.id() != message_id);

                    if let Some(msg) = most_recent_message {
                        if chat.messages.is_empty() {
                            chat.messages.push_back(msg);
                        }
                    }

                    if chat
                        .replying_to
                        .as_ref()
                        .map(|msg| msg.id() == message_id)
                        .unwrap_or_default()
                    {
                        chat.replying_to.take();
                    }
                }

                if should_decrement_notifications {
                    self.mutate(Action::RemoveNotification(
                        notifications::NotificationKind::Message,
                        1,
                    ));
                }
            }
            MessageEvent::MessagePinned { message } => {
                self.pin_message(message);
            }
            MessageEvent::MessageUnpinned { message } => {
                self.unpin_message(message);
            }
            MessageEvent::MessageReactionAdded { message } => {
                self.update_reactions(message);
            }
            MessageEvent::MessageReactionRemoved { message } => {
                self.update_reactions(message);
            }
            MessageEvent::TypingIndicator {
                conversation_id,
                participant,
            } => {
                self.update_identity_status_hack(&participant);
                if !self.chats.in_sidebar.contains(&conversation_id) {
                    return;
                }
                match self.chats.all.get_mut(&conversation_id) {
                    Some(chat) => {
                        chat.typing_indicator.insert(participant, Instant::now());
                    }
                    None => {
                        log::warn!(
                            "attempted to update typing indicator for nonexistent conversation: {}",
                            conversation_id
                        );
                    }
                }
            }
            MessageEvent::RecipientAdded {
                conversation,
                identity,
            } => {
                self.identities.insert(identity.did_key(), identity);
                if let Some(chat) = self.chats.all.get_mut(&conversation.id()) {
                    chat.participants = HashSet::from_iter(conversation.recipients());
                }
            }
            MessageEvent::RecipientRemoved { conversation } => {
                if let Some(chat) = self.chats.all.get_mut(&conversation.id()) {
                    // Also remove the recipient from the calls if present
                    // Waiting for blink implementation to also kick the user from call?
                    /*for did in &chat.participants {
                        if !conversation.recipients().contains(did) {
                            let _ = self.ui.call_info.remove_participant(conversation.id(), did);
                        }
                    }*/
                    chat.participants = HashSet::from_iter(conversation.recipients());
                }
            }
            MessageEvent::ConversationNameUpdated { conversation } => {
                if let Some(chat) = self.chats.all.get_mut(&conversation.id()) {
                    chat.conversation_name = conversation.name();
                }
            }
            MessageEvent::ConversationSettingsUpdated {
                conversation,
                settings,
            } => {
                if let Some(chat) = self.chats.all.get_mut(&conversation.id()) {
                    chat.settings = settings;
                }
            }
            MessageEvent::AttachmentProgress { .. } => todo!(),
        }
    }

    fn process_blink_event(&mut self, event: BlinkEventKind) {
        match event {
            BlinkEventKind::IncomingCall {
                call_id,
                conversation_id,
                sender: _,
                participants,
            } => {
                let conversation_id = match conversation_id {
                    Some(r) => r,
                    None => {
                        log::error!("received incoming call with no conversation id");
                        return;
                    }
                };
                self.send_chat_to_top_of_sidebar(conversation_id);
                if let Err(e) =
                    self.ui
                        .call_info
                        .pending_call(call_id, conversation_id, participants)
                {
                    log::error!("failed to process IncomingCall event: {e}");
                }
            }
            BlinkEventKind::CallCancelled { call_id } => {
                self.ui.call_info.remove_pending_call(call_id);
            }
            BlinkEventKind::ParticipantJoined { call_id, peer_id } => {
                if let Err(e) = self.ui.call_info.participant_joined(call_id, peer_id) {
                    log::error!("failed to process ParticipantJoined event : {e}");
                }
            }
            BlinkEventKind::ParticipantLeft { call_id, peer_id } => {
                if let Err(e) = self.ui.call_info.participant_left(call_id, &peer_id) {
                    log::error!("failed to process ParticipantLeft event : {e}");
                }
            }
            BlinkEventKind::CallTerminated { call_id } => {
                if self
                    .ui
                    .call_info
                    .active_call()
                    .map(|x| x.call.id == call_id)
                    .unwrap_or(false)
                {
                    self.ui.call_info.end_call();
                }
            }
            BlinkEventKind::ParticipantSpeaking { peer_id } => {
                if let Err(e) = self.ui.call_info.participant_speaking(peer_id) {
                    log::error!("failed to process ParticipantSpeaking event : {e}");
                }
            }
            BlinkEventKind::SelfSpeaking => {
                // todo
            }
            BlinkEventKind::AudioDegradation { peer_id } => {
                // todo
                log::info!("audio degradation for peer {}", peer_id);
            }
            BlinkEventKind::AudioOutputDeviceNoLongerAvailable
            | BlinkEventKind::AudioInputDeviceNoLongerAvailable => {
                // todo: notify user
                log::info!("audio I/O device no longer available");
            }
            BlinkEventKind::ParticipantStateChanged { peer_id, state } => {
                if let Err(e) = self.ui.call_info.update_participant_state(peer_id, state) {
                    log::error!("failed to process ParticipantStateChanged event : {e}");
                }
            }
            BlinkEventKind::AudioStreamError => {
                log::error!("blink audio stream error");
                // todo
            }
        }
    }
}

impl State {
    pub fn mock(
        my_id: Identity,
        mut identities: HashMap<DID, Identity>,
        chats: chats::Chats,
        friends: friends::Friends,
        storage: Storage,
    ) -> Self {
        let id = my_id.did_key();
        identities.insert(my_id.did_key(), my_id);
        Self {
            id,
            route: Route { active: "/".into() },
            storage,
            chats,
            friends,
            identities,
            initialized: true,
            ..Default::default()
        }
    }
    /// Saves the current state to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = serde_json::to_string_pretty(self)?;
        let path = if STATIC_ARGS.use_mock {
            &STATIC_ARGS.mock_cache_path
        } else {
            &STATIC_ARGS.cache_path
        };
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn get_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Loads the state from a file on disk, if it exists.
    pub fn load() -> Self {
        if STATIC_ARGS.use_mock {
            return State::load_mock();
        };

        let mut success = true;

        let mut state = {
            match fs::read_to_string(&STATIC_ARGS.cache_path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!(
                            "state.json failed to deserialize: {e}. Initializing State with default values"
                        );
                        success = false;
                        State::default()
                    }
                },
                Err(_) => {
                    log::info!("state.json not found. Initializing State with default values");
                    success = false;
                    State::default()
                }
            }
        };
        // not sure how these defaulted to true, but this should serve as additional
        // protection in the future
        state.initialized = false;

        if !success {
            state.chats.readd_sidebars = true;
        }

        if state.settings.font_scale() == 0.0 {
            state.settings.set_font_scale(1.0);
        }

        // Guarantee any new keybinds added, will be added to the user's settings
        let default_keybinds = default_keybinds::get_default_keybinds();
        if state.settings.keybinds.len() < default_keybinds.len() {
            let new_keybinds = default_keybinds
                .iter()
                .filter(|default_keybind| {
                    !state
                        .settings
                        .keybinds
                        .iter()
                        .any(|keybind| &keybind == default_keybind)
                })
                .cloned()
                .collect::<Vec<_>>();

            state.settings.keybinds.extend(new_keybinds);
        }

        // Reload themes from disc
        let themes = get_available_themes();
        let theme = themes.iter().find(|t| {
            state
                .ui
                .theme
                .as_ref()
                .map(|theme| theme.eq(t))
                .unwrap_or_default()
        });
        if let Some(t) = theme {
            state.set_theme(Some(t.clone()));
        }
        let user_lang_saved = state.settings.language.clone();
        change_language(user_lang_saved);
        state
    }
    fn load_mock() -> Self {
        generate_mock()
        // the following doesn't work anymore now that Identities are centralized
        // let contents = match fs::read_to_string(&STATIC_ARGS.mock_cache_path) {
        //     Ok(r) => r,
        //     Err(_) => {
        //         return generate_mock();
        //     }
        // };
        // serde_json::from_str(&contents).unwrap_or_else(|_| generate_mock())
    }

    pub fn init_warp(
        &mut self,
        friends: Friends,
        chats: HashMap<Uuid, Chat>,
        mut identities: HashMap<DID, Identity>,
    ) {
        self.friends = friends;
        for (id, chat) in chats {
            if let Some(conv) = self.chats.all.get_mut(&id) {
                conv.messages = chat.messages;
                conv.conversation_type = chat.conversation_type;
                conv.has_more_messages = chat.has_more_messages;
                conv.conversation_name = chat.conversation_name;
                conv.creator = chat.creator;
                conv.pinned_messages = chat.pinned_messages;
            } else {
                self.chats.all.insert(id, chat);
            }
        }
        self.identities.extend(identities.drain());

        if self.chats.readd_sidebars {
            self.chats.readd_sidebars = false;
            self.chats
                .in_sidebar
                .append(&mut self.chats.all.keys().cloned().collect())
        }

        self.initialized = true;
    }
}

// for id
impl State {
    pub fn did_key(&self) -> DID {
        self.id.clone()
    }
}

// for route
impl State {
    /// Sets the active route in the `State` struct.
    ///
    /// # Arguments
    ///
    /// * `to` - The route to set as the active route.
    fn set_active_route(&mut self, to: String) {
        self.route.active = to;
    }
}

// for chats
impl State {
    pub fn chats(&self) -> &chats::Chats {
        &self.chats
    }
    pub fn chats_favorites(&self) -> Vec<Chat> {
        self.chats
            .favorites
            .iter()
            .filter_map(|did| self.chats.all.get(did))
            .cloned()
            .collect()
    }
    pub fn chats_sidebar(&self) -> Vec<Chat> {
        self.chats
            .in_sidebar
            .iter()
            .filter_map(|did| self.chats.all.get(did))
            .cloned()
            .collect()
    }
    pub fn chat_participants(&self, chat: &Chat) -> Vec<Identity> {
        chat.participants
            .iter()
            .filter_map(|did| self.identities.get(did))
            .cloned()
            .collect()
    }

    // hide IF favorites.len() = 0 AND not is_minimal_view OR is_sidebar_hidden
    pub fn show_slimbar(&self) -> bool {
        let has_favs = !self.chats_favorites().is_empty();
        let is_minimal_view = self.ui.is_minimal_view();
        let sidebar_hidden = self.ui.sidebar_hidden;
        let experimental_features = self.configuration.developer.experimental_features;

        has_favs || is_minimal_view || sidebar_hidden || experimental_features
    }
    fn add_msg_to_chat(&mut self, conversation_id: Uuid, message: ui_adapter::Message) {
        let msg_id = message.inner.id();
        let is_active_scrolled = self.chats.active_chat_is_scrolled();
        if let Some(chat) = self.chats.all.get_mut(&conversation_id) {
            chat.typing_indicator.remove(&message.inner.sender());
            chat.messages.push_back(message.clone());
            chat.mentions.push_back(message);
            // only care about the most recent message, for the sidebar
            if chat.messages.len() > 1 {
                chat.messages.pop_front();
            }

            if self.ui.current_layout != ui::Layout::Compose
                || self.chats.active != Some(conversation_id)
                || is_active_scrolled
            {
                chat.add_unread(msg_id);
            }
        }
    }

    pub fn active_chat_has_draft(&self) -> bool {
        self.get_active_chat()
            .as_ref()
            .and_then(|d| d.draft.as_ref())
            .map(|d| !d.is_empty())
            .unwrap_or(false)
    }

    pub fn active_chat_send_in_progress(&self) -> Option<Vec<PendingMessage>> {
        self.get_active_chat()
            .map(|chat| chat.pending_outgoing_messages)
    }

    /// Cancels a reply within a given chat on `State` struct.
    ///
    /// # Arguments
    ///
    /// * `chat` - The chat to stop replying to.
    fn cancel_reply(&mut self, chat_id: Uuid) {
        if let Some(c) = self.chats.all.get_mut(&chat_id) {
            c.replying_to = None;
        }
    }
    pub fn can_use_active_chat(&self) -> bool {
        self.get_active_chat()
            .map(|c| {
                if c.conversation_type == ConversationType::Direct {
                    return c
                        .participants
                        .iter()
                        .all(|e| e.eq(&self.did_key()) || self.has_friend_with_did(e));
                }
                // If more than 2 participants -> group chat
                // Dont need to be friends with all in a group
                true
            })
            .unwrap_or_default()
    }
    /// Clears the active chat in the `State` struct.
    fn clear_active_chat(&mut self) {
        self.chats.active = None;
    }
    pub fn clear_typing_indicator(&mut self, instant: Instant) -> bool {
        let mut needs_update = false;
        for conv_id in self.chats.in_sidebar.iter() {
            let chat = match self.chats.all.get_mut(conv_id) {
                Some(c) => c,
                None => {
                    log::warn!("conv {} found in sidebar but not in HashMap", conv_id);
                    continue;
                }
            };
            let old_len = chat.typing_indicator.len();
            chat.typing_indicator
                .retain(|_id, time| instant - *time < Duration::from_secs(5));
            let new_len = chat.typing_indicator.len();

            if old_len != new_len {
                needs_update = true;
            }
        }

        needs_update
    }

    /// Clears the given chats draft message
    fn clear_chat_draft(&mut self, chat_id: &Uuid) {
        if let Some(c) = self.chats.all.get_mut(chat_id) {
            c.draft = None;
        }
    }

    fn clear_chat_attachments(&mut self, chat_id: &Uuid) {
        if let Some(c) = self.chats.all.get_mut(chat_id) {
            c.files_attached_to_send.clear();
        }
    }

    /// Clear unreads  within a given chat on `State` struct.
    ///
    /// # Arguments
    ///
    /// * `chat_id` - The chat to clear unreads on.
    ///
    fn clear_unreads(&mut self, chat_id: Uuid) {
        if let Some(chat) = self.chats.all.get_mut(&chat_id) {
            chat.clear_unreads();
        }
    }
    /// Adds the given chat to the user's favorites.
    fn favorite(&mut self, chat: &Uuid) {
        if !self.chats.favorites.contains(chat) {
            self.chats.favorites.push(*chat);
        }
    }
    pub fn finished_loading_chat(&mut self, chat_id: Uuid) {
        if let Some(chat) = self.chats.all.get_mut(&chat_id) {
            chat.has_more_messages = false;
        }
    }
    /// Get the active chat on `State` struct.
    pub fn get_active_chat(&self) -> Option<Chat> {
        self.chats
            .active
            .and_then(|uuid| self.chats.all.get(&uuid).cloned())
    }
    pub fn get_active_media_chat(&self) -> Option<&Chat> {
        self.chats
            .active_media
            .and_then(|uuid| self.chats.all.get(&uuid))
    }
    pub fn get_chat_by_id(&self, id: Uuid) -> Option<Chat> {
        self.chats.all.get(&id).cloned()
    }
    pub fn get_chat_with_friend(&self, friend: DID) -> Option<Chat> {
        self.chats
            .all
            .values()
            .find(|chat| {
                chat.conversation_type == ConversationType::Direct
                    && chat.participants.contains(&friend)
            })
            .cloned()
    }
    // assumes the messages are sorted by most recent to oldest
    pub fn update_chat_messages(
        &mut self,
        conversation_id: Uuid,
        messages: Vec<ui_adapter::Message>,
    ) {
        if let Some(chat) = self.chats.all.get_mut(&conversation_id) {
            chat.messages = messages.into();
        }
    }

    /// Check if given chat is favorite on `State` struct.
    ///
    /// # Arguments
    ///
    /// * `chat` - The chat to check.
    pub fn is_favorite(&self, chat: &Chat) -> bool {
        self.chats.favorites.contains(&chat.id)
    }

    pub fn reached_max_pinned(&self, chat: &Uuid) -> bool {
        let conv = match self.chats.all.get(chat) {
            Some(c) => c,
            None => {
                log::warn!("attempted to get nonexistent conversation");
                return true;
            }
        };
        conv.pinned_messages.len() >= MAX_PINNED_MESSAGES.into()
    }

    fn pin_message(&mut self, message: warp::raygun::Message) {
        let message_id = message.id();
        let conv = match self.chats.all.get_mut(&message.conversation_id()) {
            Some(c) => c,
            None => {
                log::warn!("attempted to update message in nonexistent conversation");
                return;
            }
        };

        conv.pinned_messages.push(message);
        conv.pinned_messages
            .sort_by_key(|r| std::cmp::Reverse(r.date()));

        if let Some(msg) = conv
            .messages
            .iter_mut()
            .find(|m| m.inner.id() == message_id)
        {
            msg.inner.set_pinned(true);
        }
    }

    fn unpin_message(&mut self, message: warp::raygun::Message) {
        let message_id = message.id();
        let conv = match self.chats.all.get_mut(&message.conversation_id()) {
            Some(c) => c,
            None => {
                log::warn!("attempted to update message in nonexistent conversation");
                return;
            }
        };

        conv.pinned_messages.retain(|x| x.id() != message_id);

        if let Some(msg) = conv
            .messages
            .iter_mut()
            .find(|m| m.inner.id() == message_id)
        {
            msg.inner.set_pinned(false);
        }
    }

    // this is used for adding/removing reactions.
    // if pinned messages ever need to display a reaction, additional code may be needed here.
    pub fn update_reactions(&mut self, message: warp::raygun::Message) {
        let conv = match self.chats.all.get_mut(&message.conversation_id()) {
            Some(c) => c,
            None => {
                log::warn!("attempted to update message in nonexistent conversation");
                return;
            }
        };
        let message_id = message.id();
        if let Some(msg) = conv
            .messages
            .iter_mut()
            .find(|m| m.inner.id() == message_id)
        {
            *msg.inner.reactions_mut() = message.reactions();
        } else {
            log::warn!("attempted to update a message which wasn't found");
        }
    }

    /// Remove a chat from the sidebar on `State` struct.
    ///
    /// # Arguments
    ///
    /// * `chat_id` - The chat to remove.
    fn remove_sidebar_chat(&mut self, chat_id: Uuid) {
        self.chats.in_sidebar.retain(|id| *id != chat_id);

        if let Some(id) = self.chats.active {
            if id == chat_id {
                self.clear_active_chat();
            }
        }
    }
    /// Sets the active chat in the `State` struct.
    ///
    /// # Arguments
    ///
    /// * `chat` - The chat to set as the active chat.
    fn set_active_chat(&mut self, chat: &Uuid, should_move_to_top: bool) {
        self.chats.active = Some(*chat);
        if should_move_to_top {
            self.send_chat_to_top_of_sidebar(*chat);
        } else if !self.chats.in_sidebar.contains(chat) {
            self.chats.in_sidebar.push_front(*chat);
        }
        // don't clear unreads here. need additional information, which is present in the Chatbar.
    }

    fn send_chat_to_top_of_sidebar(&mut self, chat_id: Uuid) {
        self.chats.in_sidebar.retain(|id| id != &chat_id);
        self.chats.in_sidebar.push_front(chat_id);
    }

    pub fn set_chat_scrolled(&mut self, chat_id: Uuid, val: bool) {
        if let Some(chat) = self.chats.all.get_mut(&chat_id) {
            chat.is_scrolled = val;
        }
    }

    // indicates that a conversation has a pending outgoing message
    // sends it to the active chat
    pub fn increment_outgoing_messages(&mut self, message_id: Uuid, msg: Vec<String>) {
        if let Some(id) = self.chats.active {
            self.increment_outgoing_messages_for(id, message_id, msg);
        }
    }

    pub fn increment_outgoing_messages_for(
        &mut self,
        chat_id: Uuid,
        message_id: Uuid,
        msg: Vec<String>,
    ) {
        let did = self.get_own_identity().did_key();
        if let Some(chat) = self.chats.all.get_mut(&chat_id) {
            if !chat.append_pending_msg(chat_id, message_id, did, msg) {
                log::debug!("attempted to add an already existing pending message");
            }
        }
    }

    pub fn update_outgoing_messages(
        &mut self,
        conv_id: Uuid,
        message_id: Uuid,
        location: Location,
        progress: FileProgression,
    ) -> bool {
        let mut update = false;
        if let FileProgression::ProgressFailed {
            name,
            last_size: _,
            error,
        } = &progress
        {
            let err = get_upload_error_text(error);
            self.mutate(Action::AddToastNotification(ToastNotification::init(
                name.clone(),
                err,
                None,
                2,
            )));
            update = true;
        }
        if let Some(chat) = self.chats.all.get_mut(&conv_id) {
            chat.update_pending_msg(message_id, location, progress);
        }
        update
    }

    pub fn remove_outgoing_attachment(
        &mut self,
        conv_id: Uuid,
        message_id: Uuid,
        location: FileLocation,
    ) {
        if let Some(chat) = self.chats.all.get_mut(&conv_id) {
            chat.remove_pending_msg_attachment(message_id, location);
        }
    }

    pub fn decrement_outgoing_messages(&mut self, conv_id: Uuid, message_id: Uuid) {
        if let Some(chat) = self.chats.all.get_mut(&conv_id) {
            chat.remove_pending_msg(message_id);
        }
    }

    fn set_chat_attachments(&mut self, chat_id: &Uuid, value: Vec<Location>) {
        if let Some(c) = self.chats.all.get_mut(chat_id) {
            c.files_attached_to_send = value;
        }
    }

    /// Sets the draft on a given chat to some contents.
    fn set_chat_draft(&mut self, chat_id: &Uuid, value: String) {
        if let Some(c) = self.chats.all.get_mut(chat_id) {
            c.draft = Some(value);
        }
    }
    /// Begins replying to a message in the specified chat in the `State` struct.
    fn start_replying(&mut self, chat: &Uuid, message: &ui_adapter::Message) {
        if let Some(c) = self.chats.all.get_mut(chat) {
            c.replying_to = Some(message.inner.clone());
        }
    }
    /// Toggles the specified chat as a favorite in the `State` struct. If the chat
    /// is already a favorite, it is removed from the favorites list. Otherwise, it
    /// is added to the list.
    fn toggle_favorite(&mut self, chat: &Uuid) {
        let faves = &mut self.chats.favorites;
        if let Some(index) = faves.iter().position(|uid| uid == chat) {
            faves.remove(index);
        } else {
            faves.push(*chat);
        }
    }
    /// Removes the given chat from the user's favorites.
    fn unfavorite(&mut self, chat_id: Uuid) {
        self.chats.favorites.retain(|uid| *uid != chat_id);
    }
}

// for friends
impl State {
    pub fn friends(&self) -> &friends::Friends {
        &self.friends
    }

    fn block(&mut self, identity: &DID) {
        // If the identity is not already blocked, add it to the blocked list
        self.friends.blocked.insert(identity.clone());

        // Remove the identity from the outgoing requests list if they are present
        self.friends.outgoing_requests.remove(identity);
        self.friends.incoming_requests.remove(identity);

        // still want the username to appear in the blocked list
        //self.identities.remove(&identity.did_key());

        // Remove the identity from the friends list if they are present
        self.remove_friend(identity);
    }
    fn complete_request(&mut self, identity: &Identity) {
        self.friends.outgoing_requests.remove(&identity.did_key());
        self.friends.incoming_requests.remove(&identity.did_key());
        self.friends.all.insert(identity.did_key());
        // should already be in self.identities
        self.identities.insert(identity.did_key(), identity.clone());
    }
    fn cancel_request(&mut self, identity: &DID) {
        self.friends.outgoing_requests.remove(identity);
        self.friends.incoming_requests.remove(identity);
    }
    fn new_incoming_request(&mut self, identity: &Identity) {
        self.friends.incoming_requests.insert(identity.did_key());
        self.identities.insert(identity.did_key(), identity.clone());
    }

    fn new_outgoing_request(&mut self, identity: &Identity) {
        self.friends.outgoing_requests.insert(identity.did_key());
        self.identities.insert(identity.did_key(), identity.clone());
    }
    pub fn get_friends_by_first_letter(
        friends: HashMap<DID, Identity>,
    ) -> BTreeMap<char, Vec<Identity>> {
        let mut friends_by_first_letter: BTreeMap<char, Vec<Identity>> = BTreeMap::new();

        // Iterate over the friends and add each one to the appropriate Vec in the
        // friends_by_first_letter HashMap
        for (_, friend) in friends {
            let first_letter = friend
                .username()
                .chars()
                .next()
                .expect("all friends should have a username")
                .to_ascii_lowercase();

            friends_by_first_letter
                .entry(first_letter)
                .or_default()
                .push(friend.clone());
        }

        for (_, list) in friends_by_first_letter.iter_mut() {
            list.sort_by(|a, b| {
                a.username()
                    .cmp(&b.username())
                    .then(a.did_key().to_string().cmp(&b.did_key().to_string()))
            })
        }

        friends_by_first_letter
    }
    pub fn has_friend_with_did(&self, did: &DID) -> bool {
        self.friends.all.contains(did)
    }
    fn remove_friend(&mut self, did: &DID) {
        // Remove the friend from the all field of the friends struct
        self.friends.all.remove(did);

        let all_chats = self.chats.all.clone();

        // Check if there is a direct chat with the friend being removed
        let direct_chat = all_chats.values().find(|chat| {
            chat.conversation_type == ConversationType::Direct
                && chat
                    .participants
                    .iter()
                    .any(|participant| participant == did)
        });

        // if no direct chat was found then return
        let direct_chat = match direct_chat {
            Some(c) => c,
            None => return,
        };

        // If the friend's direct chat is currently the active chat, clear the active chat
        if let Some(id) = self.chats.active {
            if id == direct_chat.id {
                self.clear_active_chat();
            }
        }

        // Remove chat from favorites if it exists
        self.unfavorite(direct_chat.id);
    }
    fn unblock(&mut self, identity: &DID) {
        self.friends.blocked.remove(identity);
    }
    pub fn is_blocked(&self, did: &DID) -> bool {
        self.friends.blocked.contains(did)
    }
}

// for storage
impl State {}

// for settings
impl State {
    fn set_show_dev_settings(&mut self, value: bool) {
        self.ui.show_dev_settings = value;
    }

    /// Sets the user's language.
    fn set_language(&mut self, string: &str) {
        self.settings.language = string.to_string();
    }

    pub fn update_available(&mut self, version: String) {
        if self.settings.update_available != Some(version.clone()) {
            self.settings.update_available = Some(version);
            self.ui.notifications.increment(
                &self.configuration,
                notifications::NotificationKind::Settings,
                1,
                !self.ui.metadata.focused,
            )
        }
    }
}

// for ui
impl State {
    // returns true if toasts were removed
    pub fn decrement_toasts(&mut self) -> bool {
        let mut remaining: HashMap<Uuid, ToastNotification> = HashMap::new();
        for (id, toast) in self.ui.toast_notifications.iter_mut() {
            toast.decrement_time();
            if toast.remaining_time() > 0 {
                remaining.insert(*id, toast.clone());
            }
        }

        if remaining.len() != self.ui.toast_notifications.len() {
            self.ui.toast_notifications = remaining;
            true
        } else {
            false
        }
    }
    pub fn has_toasts(&self) -> bool {
        !self.ui.toast_notifications.is_empty()
    }
    fn toggle_mute(&mut self) {
        self.ui.toggle_muted();
    }

    fn toggle_silence(&mut self) {
        self.ui.toggle_silenced();
    }

    pub fn remove_toast(&mut self, id: &Uuid) {
        let _ = self.ui.toast_notifications.remove(id);
    }
    pub fn remove_window(&mut self, id: WindowId) {
        self.ui.remove_overlay(id);
    }
    pub fn reset_toast_timer(&mut self, id: &Uuid) {
        if let Some(toast) = self.ui.toast_notifications.get_mut(id) {
            toast.reset_time();
        }
    }
    /// Sets the active media to the specified conversation id
    fn set_active_media(&mut self, id: Uuid) {
        self.chats.active_media = Some(id);
    }
    pub fn set_theme(&mut self, theme: Option<Theme>) {
        self.ui.theme = theme;
    }
    pub fn set_font(&mut self, font: Option<Font>) {
        self.ui.font = font;
    }
    /// Updates the display of the overlay
    fn toggle_overlay(&mut self, enabled: bool) {
        self.ui.enable_overlay = enabled;
        if !enabled {
            self.ui.clear_overlays();
        }
    }
}

// for configuration
impl State {}

// for identities
impl State {
    pub fn blocked_fr_identities(&self) -> Vec<Identity> {
        self.friends
            .blocked
            .iter()
            .filter_map(|did| self.identities.get(did))
            .cloned()
            .collect()
    }
    pub fn friend_identities(&self) -> Vec<Identity> {
        self.friends
            .all
            .iter()
            .filter_map(|did| self.identities.get(did))
            .cloned()
            .collect()
    }
    pub fn get_identities_from_call(&self, call: &Call) -> Vec<Identity> {
        call.participants_joined
            .keys()
            .filter_map(|id| self.identities.get(id))
            .cloned()
            .collect()
    }
    pub fn get_identities(&self, ids: &[DID]) -> Vec<Identity> {
        ids.iter()
            .filter_map(|id| self.identities.get(id))
            .cloned()
            .collect()
    }
    pub fn get_identity(&self, did: &DID) -> Option<Identity> {
        self.identities.get(did).cloned()
    }
    pub fn get_own_identity(&self) -> Identity {
        self.identities
            .get(&self.did_key())
            .cloned()
            .unwrap_or_default()
    }
    pub fn incoming_fr_identities(&self) -> Vec<Identity> {
        self.friends
            .incoming_requests
            .iter()
            .filter_map(|did| self.identities.get(did))
            .cloned()
            .collect()
    }
    /// Getters
    /// Getters are the only public facing methods besides dispatch.
    /// Getters help retrieve data from state in common ways preventing reused code.
    pub fn is_me(&self, identity: &Identity) -> bool {
        identity.did_key().to_string() == self.did_key().to_string()
    }
    pub fn outgoing_fr_identities(&self) -> Vec<Identity> {
        self.friends
            .outgoing_requests
            .iter()
            .filter_map(|did| self.identities.get(did))
            .cloned()
            .collect()
    }
    pub fn set_own_identity(&mut self, identity: Identity) {
        self.id = identity.did_key();
        self.ui.cached_username = Some(identity.username());
        self.identities.insert(identity.did_key(), identity);
    }
    pub fn search_identities(
        &self,
        name_prefix: &str,
    ) -> (Vec<identity_search_result::Entry>, Vec<Identity>) {
        let entries = self
            .identities
            .values()
            .filter(|id| {
                let un = id.username();
                if un.len() < name_prefix.len() {
                    false
                } else {
                    un[..name_prefix.len()].eq_ignore_ascii_case(name_prefix)
                }
            })
            .filter(|id| id.did_key() != self.did_key())
            .map(|id| identity_search_result::Entry::from_identity(id.username(), id.did_key()))
            .collect();

        let identities = self
            .identities
            .values()
            .filter(|id| {
                let un = id.username();
                if un.len() < name_prefix.len() {
                    false
                } else {
                    un[..name_prefix.len()].eq_ignore_ascii_case(name_prefix)
                }
            })
            .filter(|f| f.did_key() != self.did_key())
            .cloned()
            .collect();

        (entries, identities)
    }
    // lets the user search for a group chat by chat name or, if a chat is not named, by the names of its participants
    pub fn search_group_chats(
        &self,
        name_prefix: &str,
    ) -> (Vec<identity_search_result::Entry>, Vec<Chat>) {
        let get_display_name = |chat: &Chat| -> String {
            let names: Vec<_> = chat
                .participants
                .iter()
                .filter_map(|id| self.identities.get(id))
                .map(|x| x.username())
                .collect();

            names.join(",")
        };

        let compare_str = |v: &str| {
            if v.len() < name_prefix.len() {
                false
            } else {
                v.to_lowercase().starts_with(&name_prefix.to_lowercase())
            }
        };

        let chats_entries = self
            .chats
            .all
            .iter()
            .filter(|(_, v)| v.conversation_type == ConversationType::Group)
            .filter(|(_k, v)| {
                let names: Vec<_> = v
                    .participants
                    .iter()
                    .filter_map(|id| self.identities.get(id))
                    .map(|x| x.username())
                    .collect();

                let user_name_match = names.iter().any(|n| compare_str(n));
                let group_name_match = match v.conversation_name.as_ref() {
                    Some(n) => compare_str(n),
                    None => false,
                };

                user_name_match || group_name_match
            })
            .map(|(k, v)| {
                if let Some(name) = v.conversation_name.as_ref() {
                    identity_search_result::Entry::from_chat(name.clone(), *k)
                } else {
                    let name = get_display_name(v);
                    identity_search_result::Entry::from_chat(name, *k)
                }
            })
            .collect();

        let chats: Vec<Chat> = self
            .chats
            .all
            .iter()
            .filter(|(_, v)| v.conversation_type == ConversationType::Group)
            .filter(|(_k, v)| {
                let names: Vec<_> = v
                    .participants
                    .iter()
                    .filter_map(|id| self.identities.get(id))
                    .map(|x| x.username())
                    .collect();

                let user_name_match = names.iter().any(|n| compare_str(n));
                let group_name_match = match v.conversation_name.as_ref() {
                    Some(n) => compare_str(n),
                    None => false,
                };

                user_name_match || group_name_match
            })
            .map(|(_, v)| v.clone())
            .collect();
        (chats_entries, chats)
    }
    pub fn update_identity(&mut self, id: DID, ident: identity::Identity) {
        if let Some(friend) = self.identities.get_mut(&id) {
            *friend = ident;
        }
    }

    pub fn update_identity_with(&mut self, id: DID, mut ident: impl FnMut(&mut Identity)) {
        if let Some(friend) = self.identities.get_mut(&id) {
            ident(friend);
        }
    }
    // identities are updated once a minute for friends. but if someone sends you a message, they should be seen as online.
    // this function checks if the friend is offline and if so, sets them to online. This may be incorrect, but should
    // be corrected when the identity list is periodically updated
    pub fn update_identity_status_hack(&mut self, id: &DID) {
        if let Some(ident) = self.identities.get_mut(id) {
            if ident.identity_status() == IdentityStatus::Offline {
                ident.set_identity_status(IdentityStatus::Online);
            }
        };
    }

    pub fn profile_picture(&self) -> String {
        self.identities
            .get(&self.did_key())
            .map(|x| x.profile_picture())
            .unwrap_or_default()
    }

    pub fn profile_banner(&self) -> String {
        self.identities
            .get(&self.did_key())
            .map(|x| x.profile_banner())
            .unwrap_or_default()
    }
    pub fn join_usernames(identities: &[Identity]) -> String {
        identities
            .iter()
            .map(|x| x.username())
            .collect::<Vec<String>>()
            .join(", ")
    }
    pub fn mock_own_platform(&mut self, platform: Platform) {
        if let Some(ident) = self.identities.get_mut(&self.did_key()) {
            ident.set_platform(platform);
        }
    }
    pub fn remove_self(&self, identities: &[Identity]) -> Vec<Identity> {
        identities
            .iter()
            .filter(|x| x.did_key() != self.did_key())
            .cloned()
            .collect()
    }
    pub fn status_message(&self) -> Option<String> {
        self.identities
            .get(&self.did_key())
            .and_then(|x| x.status_message())
    }
    pub fn username(&self) -> String {
        self.identities
            .get(&self.did_key())
            .map(|x| x.username())
            .unwrap_or_default()
    }
}

// putting this in a separate module for naming purposes
pub mod identity_search_result {
    use uuid::Uuid;
    use warp::crypto::DID;

    #[derive(Debug, Clone)]
    pub struct Entry {
        pub display_name: String,
        pub id: Identifier,
    }

    #[allow(clippy::large_enum_variant)]
    #[derive(Debug, Clone)]
    pub enum Identifier {
        Did(DID),
        Uuid(Uuid),
    }

    impl Entry {
        pub fn from_identity(name: String, did: DID) -> Self {
            Self {
                display_name: name,
                id: Identifier::Did(did),
            }
        }

        pub fn from_chat(name: String, id: Uuid) -> Self {
            Self {
                display_name: name,
                id: Identifier::Uuid(id),
            }
        }
    }
}

// Define a struct to represent a group of messages from the same sender.
#[derive(Clone)]
pub struct MessageGroup<'a> {
    pub sender: DID,
    pub remote: bool,
    pub messages: Vec<GroupedMessage<'a>>,
}

impl<'a> MessageGroup<'a> {
    pub fn new(sender: DID, my_did: &DID) -> Self {
        Self {
            remote: sender != *my_did,
            sender,
            messages: vec![],
        }
    }
}

// Define a struct to represent a message that has been placed into a group.
#[derive(Clone)]
pub struct GroupedMessage<'a> {
    pub message: &'a ui_adapter::Message,
    pub attachment_progress: Option<&'a HashMap<FileLocation, FileProgression>>,
    pub is_pending: bool,
    pub is_first: bool,
    pub is_last: bool,
    // if the user scrolls over this message, more messages should be loaded
    pub should_fetch_more: bool,
}

impl<'a> GroupedMessage<'a> {
    pub fn clear_last(&mut self) {
        self.is_last = false;
    }
}

pub fn create_message_groups<'a>(
    my_did: DID,
    when_to_fetch_more: usize,
    // true if the chat has more messages to fetch
    has_more: bool,
    input: &'a VecDeque<ui_adapter::Message>,
) -> Vec<MessageGroup<'a>> {
    let mut messages: Vec<MessageGroup<'a>> = vec![];
    let mut need_to_fetch_more = when_to_fetch_more;

    let mut need_more = || {
        let r = need_to_fetch_more > 0;
        need_to_fetch_more = need_to_fetch_more.saturating_sub(1);
        r && has_more
    };

    for msg in input.iter() {
        if let Some(group) = messages.iter_mut().last() {
            if group.sender == msg.inner.sender() {
                let g = GroupedMessage {
                    message: msg,
                    attachment_progress: None,
                    is_pending: false,
                    is_first: false,
                    is_last: true,
                    should_fetch_more: need_more(),
                };
                // I really hope last() is O(1) time
                if let Some(g) = group.messages.iter_mut().last() {
                    g.clear_last();
                }

                group.messages.push(g);
                continue;
            }
        }

        // new group
        let mut grp = MessageGroup::new(msg.inner.sender(), &my_did);
        let g = GroupedMessage {
            message: msg,
            attachment_progress: None,
            is_pending: false,
            is_first: true,
            is_last: true,
            should_fetch_more: need_more(),
        };
        grp.messages.push(g);
        messages.push(grp);
    }

    messages
}

pub fn pending_group_messages<'a>(
    pending: &'a [PendingMessage],
    own_did: DID,
) -> Option<MessageGroup<'a>> {
    if pending.is_empty() {
        return None;
    };
    let mut messages: Vec<GroupedMessage<'a>> = vec![];
    let size = pending.len();
    for (i, msg) in pending.iter().enumerate() {
        if i == size - 1 {
            let g = GroupedMessage {
                message: &msg.message,
                attachment_progress: Some(&msg.attachments_progress),
                is_pending: true,
                is_first: false,
                is_last: true,
                should_fetch_more: false,
            };
            messages.push(g);
            continue;
        }
        let g = GroupedMessage {
            message: &msg.message,
            attachment_progress: Some(&msg.attachments_progress),
            is_pending: true,
            is_first: true,
            is_last: true,
            should_fetch_more: false,
        };
        messages.push(g);
    }
    Some(MessageGroup {
        sender: own_did,
        remote: false,
        messages,
    })
}

pub fn get_upload_error_text(err: &warp::error::Error) -> String {
    match err {
        warp::error::Error::InvalidLength {
            context: _,
            current: _,
            minimum: _,
            maximum: _,
        } => get_local_text_with_args(
            "messages.attachments-fail-msg",
            vec![(
                "reason",
                get_local_text("messages.attachments-fail-no-storage"),
            )],
        ),
        _ => get_local_text("messages.attachments-fail"),
    }
}
