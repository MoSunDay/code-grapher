//! WebSocket /ws/events: broadcast server events to browsers.

use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| client_loop(state, socket))
}

async fn client_loop(state: AppState, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.events.subscribe();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(text) => {
                    if sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    let _ = sender
                        .send(Message::Text(format!("{{\"type\":\"lagged\",\"n\":{n}}}").into()))
                        .await;
                }
                Err(_) => break,
            },
            _ = &mut recv_task => break,
        }
    }
}
