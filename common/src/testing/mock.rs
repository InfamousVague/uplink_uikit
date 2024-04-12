use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{BufWriter, Write},
};

use base64::{engine::general_purpose, Engine};
use chrono::{Duration, Utc};
use image::{ImageBuffer, Rgb, RgbImage};
use lipsum::lipsum;
use names::Generator;
use rand::{seq::SliceRandom, Rng};
use substring::Substring;
use titlecase::titlecase;
use uuid::Uuid;
use warp::{
    constellation::{directory::Directory, file::File},
    crypto::DID,
    multipass::identity::{IdentityStatus, Platform},
    raygun::{ConversationSettings, Message},
};

use crate::state::{storage::Storage, Chat, Chats, Friends, Identity, State, ToastNotification};

use crate::warp_runner::ui_adapter;

const FRIEND_COUNT: usize = 20;

pub fn generate_mock() -> State {
    let me = &generate_random_identities(1)[0];
    let identities = generate_random_identities(FRIEND_COUNT);
    let blocked_identities = generate_random_identities(3);
    let incoming_requests = generate_random_identities(2);
    let outgoing_requests = generate_random_identities(1);

    let mut all_chats: HashMap<Uuid, Chat> = HashMap::new();

    for ident in identities.iter() {
        let chat = generate_random_chat(me.clone(), &vec![ident.clone()]);
        all_chats.insert(chat.id, chat);
    }

    // let group_chat = generate_random_chat(me.clone(), &identities);
    // let group_chat_sidebar = group_chat.clone();

    // all_chats.insert(group_chat.id, group_chat);

    let in_sidebar = VecDeque::new();
    // in_sidebar.push(group_chat_sidebar.id);
    let mut toast_notifications = HashMap::new();
    toast_notifications.insert(
        Uuid::new_v4(),
        ToastNotification::init("title1".into(), "content1".into(), None, 5),
    );
    toast_notifications.insert(
        Uuid::new_v4(),
        ToastNotification::init("title2".into(), "content2".into(), None, 10),
    );
    // comment this out to test toast notifications
    toast_notifications.clear();

    let storage = generate_fake_storage();
    let mut id_map: HashMap<DID, Identity> = HashMap::new();
    for ident in identities.iter().cloned() {
        id_map.insert(ident.did_key(), ident);
    }
    for ident in blocked_identities.iter().cloned() {
        id_map.insert(ident.did_key(), ident);
    }
    for ident in incoming_requests.iter().cloned() {
        id_map.insert(ident.did_key(), ident);
    }
    for ident in outgoing_requests.iter().cloned() {
        id_map.insert(ident.did_key(), ident);
    }

    let chats = Chats {
        all: all_chats.clone(),
        active: None,
        active_media: None,
        in_sidebar,
        favorites: vec![],
        readd_sidebars: false,
    };
    let friends = Friends {
        all: HashSet::from_iter(identities.iter().map(|x| x.did_key())),
        blocked: HashSet::from_iter(blocked_identities.iter().map(|x| x.did_key())),
        incoming_requests: HashSet::from_iter(incoming_requests.iter().map(|x| x.did_key())),
        outgoing_requests: HashSet::from_iter(outgoing_requests.iter().map(|x| x.did_key())),
    };

    State::mock(me.clone(), id_map, chats, friends, storage)
}

fn generate_fake_chat(participants: Vec<Identity>, conversation: Uuid) -> Chat {
    let default_id = Identity::default();
    let mut messages = VecDeque::<ui_adapter::Message>::new();

    let mut rng = rand::thread_rng();

    let message_count = rng.gen_range(0..20);
    for _ in 0..message_count {
        let sender = participants.choose(&mut rng).unwrap_or(&default_id);
        let word_count = rng.gen_range(3..20);
        let mut default_message = Message::default();
        default_message.set_conversation_id(conversation);
        default_message.set_sender(sender.did_key());
        default_message.set_lines(vec![lipsum(word_count)]);

        messages.push_back(ui_adapter::Message::new(
            default_message,
            None,
            Uuid::new_v4().to_string(),
        ));
    }

    let pinned_messages: Vec<_> = messages
        .iter()
        .filter(|m| m.inner.pinned())
        .cloned()
        .map(|m| m.inner)
        .rev()
        .collect();

    Chat::new(
        conversation,
        HashSet::from_iter(participants.iter().map(|x| x.did_key())),
        match participants.len() {
            0..=2 => ConversationSettings::Direct(Default::default()),
            _ => ConversationSettings::Group(Default::default()),
        },
        None,
        None,
        messages,
        pinned_messages,
    )
}

// Generate a random chat with the specified DID key as one of the participants
fn generate_random_chat(me: Identity, identities: &[Identity]) -> Chat {
    // Choose a random set of participants for the chat, including "me"
    let mut participants = identities.to_vec();
    participants.push(me);

    // Generate a random conversation UUID
    let conversation = Uuid::new_v4();

    // Generate a fake chat with the selected participants and conversation UUID
    let mut chat = generate_fake_chat(participants, conversation);

    // Generate a random number of messages between the participants
    let mut rng = rand::thread_rng();
    let num_messages = rng.gen_range(0..20);
    for _ in 0..num_messages {
        // Generate a random message and add it to the chat
        let message = generate_fake_message(chat.id, identities);
        chat.messages.push_back(message);
    }

    chat
}

fn fake_id() -> Identity {
    let mut id = Identity::default();
    let mut generator = Generator::default();
    let mut username = generator.next().unwrap().replace('-', " ");
    username = titlecase(&username);

    let mut rng = rand::thread_rng();
    let status_len = rng.gen_range(4..10);
    let status_msg = lipsum(status_len);

    id.set_username(&username);
    id.set_status_message(Some(status_msg));
    id
}

fn generate_random_identities(count: usize) -> Vec<Identity> {
    let mut identities: Vec<Identity> = Vec::new();
    let mut rng = rand::thread_rng();
    for _ in 0..count {
        let mut identity = fake_id();

        let mut img: RgbImage = ImageBuffer::from_raw(64, 64, vec![0; 64 * 64 * 3]).unwrap();
        for (_, _, pixel) in img.enumerate_pixels_mut() {
            // Set the pixel to a random color
            let random_color = Rgb([
                rand::thread_rng().gen_range(0..255),
                rand::thread_rng().gen_range(0..255),
                rand::thread_rng().gen_range(0..255),
            ]);
            *pixel = random_color;
        }
        let mut buffer = Vec::new();

        {
            let mut writer = BufWriter::new(&mut buffer);
            writer.write_all(&img.into_raw()).unwrap();
        }

        let base64_url = general_purpose::STANDARD.encode(&buffer);
        let image_url = format!("data:image/png;base64,{base64_url}");

        let status = match rng.gen_range(0..3) {
            0 => IdentityStatus::Online,
            1 => IdentityStatus::Away,
            2 => IdentityStatus::Busy,
            _ => IdentityStatus::Offline,
        };

        identity.set_identity_status(status);

        let platform = match rng.gen() {
            true => Platform::Desktop,
            false => Platform::Mobile,
        };

        identity.set_platform(platform);
        identity.set_profile_picture(&image_url);
        identity.set_profile_banner(&image_url);

        identities.push(identity);
    }

    identities
}

fn generate_fake_message(conversation_id: Uuid, identities: &[Identity]) -> ui_adapter::Message {
    let lorem_ipsum = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";
    let reactions = ["❤️", "😂", "😍", "💯", "👍", "😮", "😢", "😡", "🤔", "😎"];

    let mut rng = rand::thread_rng();

    // Pick a random start and end index for the lorem ipsum text.
    let start_index = rng.gen_range(0..lorem_ipsum.len() - 1);
    let end_index = rng.gen_range(start_index + 1..lorem_ipsum.len());

    // Use the start and end indices to extract a random substring of the lorem ipsum text.
    let binding = lorem_ipsum.to_string();
    let text = binding.substring(start_index, end_index);

    // Generate a random number of reactions with a 30% probability.
    let _num_reactions = if rng.gen_bool(0.3) {
        rng.gen_range(0..reactions.len())
    } else {
        0
    };

    // Choose a random identity as the sender.
    let sender = &identities[rng.gen_range(0..identities.len())];

    // Generate a random timestamp within the past 30 days.
    let timestamp = Utc::now() - Duration::try_days(rng.gen_range(0..30)).unwrap_or_default();

    let mut default_message = Message::default();
    default_message.set_conversation_id(conversation_id);
    default_message.set_date(timestamp);
    default_message.set_sender(sender.did_key());
    default_message.set_replied(None);
    default_message.set_lines(vec![text.into()]);

    ui_adapter::Message::new(default_message, None, Uuid::new_v4().to_string())
}

fn generate_fake_storage() -> Storage {
    let directories = vec![
        Directory::new("Fake Folder 1"),
        Directory::new("Fake Folder 2"),
        Directory::new("Fake Folder 3"),
    ];

    let files = vec![
        File::new("Fake 1.png"),
        File::new("Fake 2.txt"),
        File::new("Fake 3.jpeg"),
    ];

    Storage {
        initialized: true,
        current_dir: Directory::new("root"),
        directories_opened: vec![Directory::new("root")],
        directories,
        files,
        max_size: 1024,
        current_size: 500,
        files_in_queue_to_upload: Vec::new(),
    }
}
