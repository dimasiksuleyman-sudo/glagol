//! End-to-end tests: the server on 127.0.0.1 answered by Glagol's own
//! «organization server» client, which proves the two speak the same API.

use super::*;
use crate::stt::openai_compat::OpenAiCompatStt;
use crate::stt::SttError;

struct Fake {
    installed: bool,
    delay: Duration,
}

impl Recognizer for Fake {
    fn model(&self) -> Result<String, String> {
        if self.installed {
            Ok("gigaam-test".into())
        } else {
            Err("the Russian recognition model is not installed on the server".into())
        }
    }

    fn recognize(&self, pcm: &[f32]) -> Result<String, String> {
        std::thread::sleep(self.delay);
        Ok(format!("распознано {} отсчётов", pcm.len()))
    }
}

async fn start_server(fake: Fake) -> (Server, String) {
    let key = auth::generate_key();
    let server = Server::start("127.0.0.1:0".parse().unwrap(), key.clone(), Arc::new(fake))
        .await
        .unwrap();
    (server, key)
}

fn client(server: &Server, key: Option<String>) -> OpenAiCompatStt {
    OpenAiCompatStt::new(
        reqwest::Client::new(),
        format!("http://{}/v1", server.local_addr()),
        "gigaam-test",
        key,
    )
}

fn second_of_speech() -> Vec<u8> {
    crate::stt::wav::wrap_wav_s16le_mono(&vec![1000i16; 16_000], 16_000)
}

#[tokio::test]
async fn glagol_client_dictates_through_the_server() {
    let (server, key) = start_server(Fake {
        installed: true,
        delay: Duration::ZERO,
    })
    .await;
    let client = client(&server, Some(key));
    assert_eq!(client.list_models().await.unwrap(), vec!["gigaam-test"]);
    let transcript = client
        .transcribe(second_of_speech(), Some("ru"))
        .await
        .unwrap();
    assert_eq!(transcript.text, "распознано 16000 отсчётов");
    let status = server.status();
    assert_eq!(status.requests, 2);
    assert_eq!(status.failures, 0);
    assert_eq!(status.active_clients, 1);
    assert_eq!(status.pending, 0);
}

#[tokio::test]
async fn wrong_or_missing_key_is_refused() {
    let (server, _) = start_server(Fake {
        installed: true,
        delay: Duration::ZERO,
    })
    .await;
    let wrong = client(&server, Some(auth::generate_key()));
    assert!(matches!(wrong.list_models().await, Err(SttError::Auth)));
    let none = client(&server, None);
    assert!(matches!(
        none.transcribe(second_of_speech(), Some("ru")).await,
        Err(SttError::Auth)
    ));
    assert_eq!(
        server.status().active_clients,
        0,
        "refused clients are not counted"
    );
}

#[tokio::test]
async fn english_and_missing_model_are_explained() {
    let (server, key) = start_server(Fake {
        installed: true,
        delay: Duration::ZERO,
    })
    .await;
    let english = client(&server, Some(key))
        .transcribe(second_of_speech(), Some("en"))
        .await;
    assert!(
        matches!(&english, Err(SttError::Api(m)) if m.contains("400") && m.contains("Russian only")),
        "{english:?}"
    );

    let (server, key) = start_server(Fake {
        installed: false,
        delay: Duration::ZERO,
    })
    .await;
    let missing = client(&server, Some(key)).list_models().await;
    assert!(
        matches!(&missing, Err(SttError::Api(m)) if m.contains("503") && m.contains("not installed")),
        "{missing:?}"
    );
}

#[tokio::test]
async fn non_wav_audio_is_refused() {
    let (server, key) = start_server(Fake {
        installed: true,
        delay: Duration::ZERO,
    })
    .await;
    let result = client(&server, Some(key))
        .transcribe(b"ID3 not a wav at all".to_vec(), Some("ru"))
        .await;
    assert!(
        matches!(&result, Err(SttError::Api(m)) if m.contains("415")),
        "{result:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_queue_answers_429_and_glagol_clients_retry() {
    let (server, key) = start_server(Fake {
        installed: true,
        delay: Duration::from_millis(600),
    })
    .await;
    let client = client(&server, Some(key));
    let extra = 4;
    let calls: Vec<_> = (0..http::MAX_PENDING + extra)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move { client.transcribe(second_of_speech(), Some("ru")).await })
        })
        .collect();
    for call in calls {
        // The extra requests got «busy, retry in 2 s» and succeeded on retry.
        call.await
            .unwrap()
            .expect("every dictation is eventually recognized");
    }
    let status = server.status();
    assert_eq!(status.failures as usize, extra, "one 429 per extra request");
    assert_eq!(status.requests as usize, http::MAX_PENDING + 2 * extra);
    assert_eq!(status.pending, 0);
}

#[tokio::test]
async fn stopped_server_refuses_connections() {
    let (server, key) = start_server(Fake {
        installed: true,
        delay: Duration::ZERO,
    })
    .await;
    let client = client(&server, Some(key));
    server.stop();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(client.list_models().await.is_err());
}

#[test]
fn connection_codes_round_trip_for_private_addresses_only() {
    let key = auth::generate_key();
    let code = connection_code("192.168.1.10", 8765, &key);
    assert_eq!(
        parse_connection_code(&format!("  {code} ")).unwrap(),
        Connection {
            base_url: "http://192.168.1.10:8765/v1".into(),
            key: key.clone(),
        }
    );
    assert!(parse_connection_code(&connection_code("8.8.8.8", 8765, &key)).is_err());
    assert!(parse_connection_code("http://192.168.1.10:8765/v1").is_err());
    assert!(parse_connection_code("glagol://192.168.1.10:8765/?key=short").is_err());
    assert!(parse_connection_code(&format!("glagol://pc.local:8765/?key={key}")).is_err());
}
