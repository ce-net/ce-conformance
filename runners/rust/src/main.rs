//! ce-conformance Rust runner — drives the ce-rs SDK through the shared Tier-A scenario contract
//! (see ../../SCENARIOS.md) against the live node at $CE_NODE_URL, using ONLY the SDK's public
//! API. Emits one machine-readable line per scenario:
//!
//!     CONF <scenario_id> PASS
//!     CONF <scenario_id> FAIL: <detail>
//!
//! and exits non-zero if any scenario fails. Every language runner implements the same scenarios
//! with the same ids and output contract, so the driver (../../run.sh) builds one cross-language
//! matrix.

use ce_rs::CeClient;
use futures_util::StreamExt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Outcome = (bool, String);

fn ok() -> Outcome {
    (true, String::new())
}
fn no(detail: impl Into<String>) -> Outcome {
    (false, detail.into())
}

fn nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    )
}

/// Subscribe to a fresh topic, publish, and return the first payload received on that topic.
async fn await_publish(ce: &CeClient, topic: &str, payload: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    ce.subscribe(topic).await?;
    let mut stream = Box::pin(ce.messages_stream().await?);
    // Publish after a short delay so the inbound stream is established first.
    let ce2 = ce.clone();
    let topic2 = topic.to_string();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        let _ = ce2.publish(&topic2, &payload).await;
    });
    let want_topic = topic.to_string();
    let fut = async move {
        while let Some(item) = stream.next().await {
            if let Ok(m) = item {
                if m.topic == want_topic {
                    return Ok::<Vec<u8>, anyhow::Error>(hex::decode(&m.payload_hex).unwrap_or_default());
                }
            }
        }
        anyhow::bail!("stream ended before message")
    };
    match tokio::time::timeout(Duration::from_secs(8), fut).await {
        Ok(r) => r,
        Err(_) => anyhow::bail!("timeout on {topic}"),
    }
}

async fn s_status(ce: &CeClient) -> Outcome {
    match ce.status().await {
        Ok(s) if !s.node_id.is_empty() => ok(),
        Ok(_) => no("empty node_id"),
        Err(e) => no(e.to_string()),
    }
}

async fn s_pubsub_text(ce: &CeClient) -> Outcome {
    let topic = format!("conf/pubsub-text/{}", nonce());
    match await_publish(ce, &topic, b"hello-conformance".to_vec()).await {
        Ok(got) if got == b"hello-conformance" => ok(),
        Ok(got) => no(format!("got {:?}", String::from_utf8_lossy(&got))),
        Err(e) => no(e.to_string()),
    }
}

async fn s_binary(ce: &CeClient) -> Outcome {
    let topic = format!("conf/pubsub-bin/{}", nonce());
    let want = vec![0u8, 255, 16, 127, 0, 171];
    match await_publish(ce, &topic, want.clone()).await {
        Ok(got) if got == want => ok(),
        Ok(got) => no(format!("got {got:?}")),
        Err(e) => no(e.to_string()),
    }
}

async fn s_request_reply(ce: &CeClient, self_id: &str) -> Outcome {
    let topic = format!("conf/reqrep/{}", nonce());
    if let Err(e) = ce.subscribe(&topic).await {
        return no(e.to_string());
    }
    // A minimal responder: read the inbound stream and echo any request on the topic.
    let ce2 = ce.clone();
    let topic2 = topic.clone();
    let handle = tokio::spawn(async move {
        if let Ok(stream) = ce2.messages_stream().await {
            let mut stream = Box::pin(stream);
            while let Some(item) = stream.next().await {
                if let Ok(m) = item {
                    if m.topic == topic2 {
                        if let Some(token) = m.reply_token {
                            let payload = hex::decode(&m.payload_hex).unwrap_or_default();
                            let mut out = b"echo:".to_vec();
                            out.extend_from_slice(&payload);
                            let _ = ce2.reply(token, &out).await;
                        }
                    }
                }
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(600)).await;
    let res = ce.request(self_id, &topic, b"ping", 8000).await;
    handle.abort();
    match res {
        Ok(v) if v == b"echo:ping" => ok(),
        Ok(v) => no(format!("got {:?}", String::from_utf8_lossy(&v))),
        Err(e) => no(e.to_string()),
    }
}

async fn s_request_unknown(ce: &CeClient) -> Outcome {
    let bogus = "0".repeat(64);
    let start = Instant::now();
    let r = ce
        .request(&bogus, &format!("conf/nonexistent/{}", nonce()), b"x", 3000)
        .await;
    let elapsed = start.elapsed();
    match r {
        Ok(_) => no("expected error, got success"),
        Err(_) if elapsed < Duration::from_secs(15) => ok(),
        Err(_) => no(format!("did not bound: {elapsed:?}")),
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let base = std::env::var("CE_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:8844".into());
    let ce = CeClient::new(base);
    let self_id = match ce.status().await {
        Ok(s) => s.node_id,
        Err(e) => {
            println!("CONF setup FAIL: {e}");
            std::process::exit(2);
        }
    };

    let results: Vec<(&str, Outcome)> = vec![
        ("status", s_status(&ce).await),
        ("pubsub_text", s_pubsub_text(&ce).await),
        ("binary_payload", s_binary(&ce).await),
        ("request_reply", s_request_reply(&ce, &self_id).await),
        ("request_unknown_errors", s_request_unknown(&ce).await),
    ];

    let mut all_pass = true;
    for (id, (passed, detail)) in &results {
        if *passed {
            println!("CONF {id} PASS");
        } else {
            all_pass = false;
            println!("CONF {id} FAIL: {detail}");
        }
    }
    std::process::exit(if all_pass { 0 } else { 1 });
}
