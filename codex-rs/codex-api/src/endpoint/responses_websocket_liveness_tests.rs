use super::*;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;

async fn socket_pair() -> (WsStream, WebSocketStream<TcpStream>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let connector = WebSocketConnector::new(&factory).unwrap();
    let request = format!("ws://{address}").into_client_request().unwrap();
    let (client, server) = tokio::join!(
        connector.connect_loopback_direct(request, WebSocketConfig::default()),
        async {
            let (socket, _) = listener.accept().await.unwrap();
            tokio_tungstenite::accept_async(socket).await.unwrap()
        }
    );
    (WsStream::new(client.unwrap().0), server)
}

async fn acknowledge(
    client: &mut WsStream,
    server: &mut WebSocketStream<TcpStream>,
    liveness: &mut ResponseLiveness,
) {
    let created = json!({"type": "response.created", "response": {"id": "resp-1"}});
    server
        .send(Message::Text(created.to_string().into()))
        .await
        .unwrap();
    let message = liveness
        .next_message(&mut client.rx_message, &mut client.transport_activity)
        .await
        .unwrap();
    liveness.record_event(
        &serde_json::from_str::<ResponsesStreamEvent>(message.to_text().unwrap()).unwrap(),
    );
}

#[tokio::test]
async fn control_frames_keep_delayed_response_on_original_socket() {
    let (mut client, mut server) = socket_pair().await;
    let mut liveness = ResponseLiveness::new(Duration::from_millis(150));
    acknowledge(&mut client, &mut server, &mut liveness).await;
    let sender = tokio::spawn(async move {
        for _ in 0..6 {
            tokio::time::sleep(Duration::from_millis(40)).await;
            server.send(Message::Ping(vec![1].into())).await.unwrap();
            assert!(matches!(server.next().await, Some(Ok(Message::Pong(_)))));
            server.send(Message::Pong(vec![2].into())).await.unwrap();
        }
        server
            .send(Message::Text("completed".into()))
            .await
            .unwrap();
        server
    });
    let message = liveness
        .next_message(&mut client.rx_message, &mut client.transport_activity)
        .await
        .unwrap();
    assert_eq!(message, Message::Text("completed".into()));
    let _server = sender.await.unwrap();
}

#[tokio::test]
async fn control_frames_keep_delayed_first_event_on_original_socket() {
    let (mut client, mut server) = socket_pair().await;
    let liveness = ResponseLiveness::new(Duration::from_millis(150));
    let expected = Message::Text(
        json!({"type": "response.created", "response": {"id": "delayed"}})
            .to_string()
            .into(),
    );
    let created = expected.clone();
    let sender = tokio::spawn(async move {
        for _ in 0..6 {
            tokio::time::sleep(Duration::from_millis(40)).await;
            server.send(Message::Ping(vec![1].into())).await.unwrap();
            assert_eq!(
                server.next().await.unwrap().unwrap(),
                Message::Pong(vec![1].into()),
            );
        }
        server.send(created).await.unwrap();
        server
    });
    assert_eq!(
        liveness
            .next_message(&mut client.rx_message, &mut client.transport_activity)
            .await
            .unwrap(),
        expected,
    );
    let _server = sender.await.unwrap();
}

#[tokio::test]
async fn healthy_transport_has_a_bounded_response_progress_deadline() {
    let (mut client, mut server) = socket_pair().await;
    let mut liveness = ResponseLiveness::new(Duration::from_millis(50));
    acknowledge(&mut client, &mut server, &mut liveness).await;
    let sender = tokio::spawn(async move {
        loop {
            if server.send(Message::Pong(vec![1].into())).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    let error = liveness
        .next_message(&mut client.rx_message, &mut client.transport_activity)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "stream error: response progress timeout waiting for websocket"
    );
    sender.abort();
}

#[tokio::test]
async fn silent_transport_expires_before_acknowledged_response_deadline() {
    let (mut client, mut server) = socket_pair().await;
    let mut liveness = ResponseLiveness::new(Duration::from_millis(30));
    acknowledge(&mut client, &mut server, &mut liveness).await;
    let error = liveness
        .next_message(&mut client.rx_message, &mut client.transport_activity)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "stream error: idle timeout waiting for websocket"
    );
}

#[tokio::test]
async fn application_heartbeats_preserve_transport_but_cannot_hide_a_stalled_response() {
    let (mut client, mut server) = socket_pair().await;
    let mut liveness = ResponseLiveness::new(Duration::from_millis(50));
    acknowledge(&mut client, &mut server, &mut liveness).await;
    let sender = tokio::spawn(async move {
        loop {
            if server
                .send(Message::Text(
                    json!({"type": "response.heartbeat"}).to_string().into(),
                ))
                .await
                .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    let mut heartbeats = 0;
    let error = loop {
        match liveness
            .next_message(&mut client.rx_message, &mut client.transport_activity)
            .await
        {
            Ok(message) => {
                let event =
                    serde_json::from_str::<ResponsesStreamEvent>(message.to_text().unwrap())
                        .unwrap();
                liveness.record_event(&event);
                heartbeats += 1;
            }
            Err(error) => break error,
        }
    };
    assert!(heartbeats > 1);
    assert_eq!(
        error.to_string(),
        "stream error: response progress timeout waiting for websocket"
    );
    sender.abort();
}

#[tokio::test]
async fn cancelling_a_pending_reader_leaves_transport_pump_responsive() {
    let (mut client, mut server) = socket_pair().await;
    let mut liveness = ResponseLiveness::new(Duration::from_secs(1));
    acknowledge(&mut client, &mut server, &mut liveness).await;
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            liveness.next_message(&mut client.rx_message, &mut client.transport_activity),
        )
        .await
        .is_err()
    );
    server.send(Message::Ping(vec![7].into())).await.unwrap();
    assert_eq!(
        server.next().await.unwrap().unwrap(),
        Message::Pong(vec![7].into())
    );
    server
        .send(Message::Text("after cancellation".into()))
        .await
        .unwrap();
    assert_eq!(
        liveness
            .next_message(&mut client.rx_message, &mut client.transport_activity)
            .await
            .unwrap(),
        Message::Text("after cancellation".into()),
    );
}
