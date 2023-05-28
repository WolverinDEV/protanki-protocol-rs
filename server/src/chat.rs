use std::{collections::{BTreeMap, VecDeque}, sync::{Arc, RwLock}, task::{self, Poll}};

use anyhow::Context;
use fost_protocol::{codec::{ChatMessage, ChatCC, ChatModeratorLevel, UserStatus}, packets::{s2c, PacketDowncast, c2s}};
use tokio::sync::mpsc::{self, error::TrySendError};

use crate::{client::{ClientId, ClientComponent, Client, AuthenticationState}, server::ServerEvent};

#[derive(Debug, Clone)]
enum ServerChatEvent {
    Message(ChatMessage),
    MessageHistory(Vec<ChatMessage>)
}

pub struct ServerChat {
    subscriber: BTreeMap<ClientId, mpsc::UnboundedSender<ServerChatEvent>>,

    message_history_length: usize,
    message_history: VecDeque<ChatMessage>,
}

impl ServerChat {
    pub fn new() -> Self {
        let mut result = Self {
            subscriber: BTreeMap::new(),

            message_history_length: 100,
            message_history: VecDeque::with_capacity(100),
        };

        result.register_system_message("Hello world".to_string(), false);
        result.register_system_message("This server is currently under development and might not be fully functional!".to_string(), true);

        result
    }

    pub fn register_system_message(&mut self, text: String, warning: bool) {
        let chat_message = ChatMessage {
            source_user_status: None,
            target_user_status: None,
            system: true,
            warning,
            text,
        };

        self.dispatch_server_event(&ServerChatEvent::Message(chat_message.clone()));
        self.message_history.push_back(chat_message);
        while self.message_history.len() > self.message_history_length {
            let _ = self.message_history.pop_front();
        }
    }

    pub fn register_message(&mut self, user_id: &str, target_user_id: Option<String>, message: &str) {
        let chat_message = ChatMessage {
            source_user_status: Some(UserStatus {
                chat_moderator_level: ChatModeratorLevel::None,
                ip: "".to_string(),
                rank_index: 2,
                uid: user_id.to_string()
            }),
            target_user_status: None,
            system: false,
            warning: false,
            text: message.to_string(),
        };

        self.dispatch_server_event(&ServerChatEvent::Message(chat_message.clone()));
        self.message_history.push_back(chat_message);
        while self.message_history.len() > self.message_history_length {
            let _ = self.message_history.pop_front();
        }
    }

    pub fn send_message_history(&self, client_id: u32) {
        let subscriber = match self.subscriber.get(&client_id) {
            Some(subscriber) => subscriber,
            None => return,
        };

        let _ = subscriber.send(
            ServerChatEvent::MessageHistory(
                self.message_history.iter()
                    .cloned()
                    .collect()
            )
        );
    }

    fn dispatch_server_event(&mut self, event: &ServerChatEvent) {
        self.subscriber.drain_filter(|_, subscriber| {
            subscriber.send(event.clone()).is_err()
        });
    }
}

pub struct ServerChatComponent {
    server_chat: Arc<RwLock<ServerChat>>,
    subscriber: Option<mpsc::UnboundedReceiver<ServerChatEvent>>,
    waker: Option<task::Waker>,

    chat_initialized: bool,
    chat_messages_shown: bool,
}

impl ServerChatComponent {
    pub fn new(server_chat: Arc<RwLock<ServerChat>>) -> Self {
        Self {
            server_chat,
            subscriber: None,
            waker: None,

            chat_initialized: false,
            chat_messages_shown: false
        }
    }

    pub fn subscribe(&mut self, client: &mut Client) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::unbounded_channel();

        {
            let mut server_chat = self.server_chat.write()
                .ok()
                .context("failed to accquire server chat")?;

            server_chat.subscriber.insert(client.client_id(), tx);
            server_chat.send_message_history(client.client_id());
        }

        {
            self.subscriber = Some(rx);
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }

        Ok(())
    }

    pub fn unsubscribe(&mut self) {
        self.subscriber = None;
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    fn handle_chat_event(&mut self, client: &mut Client, event: ServerChatEvent) -> anyhow::Result<()> {
        match event {
            ServerChatEvent::Message(message) => {
                client.send_packet(&s2c::GlobalChatAddMessages{
                    messages: vec![ message ]
                });
                self.chat_messages_shown = true;
            },
            ServerChatEvent::MessageHistory(messages) => {
                if self.chat_messages_shown {
                    /* TODO: Clear chat history */
                }

                client.send_packet(&s2c::GlobalChatAddMessages{ messages });
            }
        }

        Ok(())
    }
}

impl ClientComponent for ServerChatComponent {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn fost_protocol::packets::Packet) -> anyhow::Result<()> {
        let user_id = if let AuthenticationState::Authenticated { user_id } = client.authentication_state() {
            user_id.clone()
        } else {
            anyhow::bail!("client is not authenticated")
        };

        if let Some(packet) = packet.downcast_ref::<c2s::GlobalChatSendMessage>() {
            let mut server_chat = self.server_chat.write()
                .ok()
                .context("failed to accquire server chat")?;

            let target = if packet.target.len() > 0 {
                Some(packet.target.clone())
            } else {
                None
            };
            server_chat.register_message(&user_id, target, &packet.text);
        }

        Ok(())    
    }

    fn poll(&mut self, client: &mut Client, cx: &mut task::Context) -> anyhow::Result<()> {
        self.waker = Some(cx.waker().clone());

        if !self.chat_initialized {
            if let AuthenticationState::Authenticated { user_id } = client.authentication_state() {
                self.chat_initialized = true;
            
                client.send_packet(&s2c::GlobalChatInitParameters{
                    init_params: ChatCC {
                        admin: false,
                        antiflood_enabled: true,
                        buffer_size: 128,
                        chat_enabled: true,
                        chat_moderator_level: ChatModeratorLevel::None,
                        links_white_list: None,
                        min_char: 1,
                        min_word: 1,
                        self_name: user_id.clone(),
                        show_links: true,
                        typing_speed_antiflood_enabled: true,
                    }
                });
    
                client.send_packet(&s2c::GlobalChatAntifloodParameters{
                    enter_cost: 880,
                    symbol_cost: 176
                });
    
                /* subscribe to the server chat by default */
                self.subscribe(client);
            }
        }

        loop {
            let rx = match &mut self.subscriber {
                Some(rx) => rx,
                None => return Ok(())
            };

            let message = match rx.poll_recv(cx) {
                Poll::Ready(Some(event)) => event,
                Poll::Ready(None) => {
                    self.subscriber = None;
                    break;
                },
                Poll::Pending => break,
            };

            self.handle_chat_event(client, message)?;
        }

        Ok(())
    }
}
