use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub struct Presence {
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Value>>,
}
impl Presence {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            result: Arc::new(Mutex::new(
                json!({"state":"idle","activityConfirmed":false,"errorCode":null}),
            )),
        }
    }
    pub fn status(&self) -> Value {
        self.result.lock().unwrap().clone()
    }
    pub fn stop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        self.result = Arc::new(Mutex::new(
            json!({"state":"idle","activityConfirmed":false,"errorCode":null}),
        ));
    }
    pub fn start(&mut self, session: Value) {
        self.stop();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let result = self.result.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move{while !cancel.load(Ordering::SeqCst){*result.lock().unwrap()=json!({"state":"connecting","activityConfirmed":false,"errorCode":null});let success=connect(&session,&cancel,&result).await;if cancel.load(Ordering::SeqCst){return;}if !success{*result.lock().unwrap()=json!({"state":"unavailable","activityConfirmed":false,"errorCode":null});}for _ in 0..40{if cancel.load(Ordering::SeqCst){return;}tokio::time::sleep(std::time::Duration::from_millis(250)).await;}}});
        });
    }
}
pub fn frame(op: u32, payload: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(payload).unwrap();
    let mut frame = Vec::with_capacity(body.len() + 8);
    frame.extend(op.to_le_bytes());
    frame.extend((body.len() as u32).to_le_bytes());
    frame.extend(body);
    frame
}
#[cfg(windows)]
fn pop_frame(buffer: &mut Vec<u8>) -> Result<Option<(u32, Value)>, String> {
    if buffer.len() < 8 {
        return Ok(None);
    }
    let length = u32::from_le_bytes(buffer[4..8].try_into().unwrap()) as usize;
    if length > 1024 * 1024 {
        return Err("Oversized IPC frame".into());
    }
    if buffer.len() < length + 8 {
        return Ok(None);
    }
    let op = u32::from_le_bytes(buffer[..4].try_into().unwrap());
    let payload = serde_json::from_slice(&buffer[8..8 + length]).map_err(|e| e.to_string())?;
    buffer.drain(..8 + length);
    Ok(Some((op, payload)))
}
#[cfg(windows)]
async fn connect(session: &Value, cancel: &AtomicBool, result: &Mutex<Value>) -> bool {
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
    async fn read(
        pipe: &mut NamedPipeClient,
        buffer: &mut Vec<u8>,
    ) -> Result<(u32, Value), String> {
        loop {
            if let Some(frame) = pop_frame(buffer)? {
                return Ok(frame);
            }
            let mut chunk = [0; 8192];
            let count = pipe.read(&mut chunk).await.map_err(|e| e.to_string())?;
            if count == 0 {
                return Err("IPC disconnected".into());
            }
            buffer.extend_from_slice(&chunk[..count]);
        }
    }

    for index in 0..10 {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let Ok(mut pipe) = ClientOptions::new().open(format!("\\\\?\\pipe\\discord-ipc-{index}"))
        else {
            continue;
        };
        let mut buffer = vec![];
        let handshake = frame(0, &json!({"v":1,"client_id":session["appId"]}));
        if pipe.write_all(&handshake).await.is_err() {
            continue;
        }
        let ready = tokio::time::timeout(std::time::Duration::from_millis(2500), async {
            loop {
                let (op, payload) = read(&mut pipe, &mut buffer).await?;
                if op == 3 {
                    pipe.write_all(&frame(4, &payload))
                        .await
                        .map_err(|e| e.to_string())?;
                } else if op == 2 {
                    return Err("Closed".into());
                } else if op == 1 && payload["evt"] == "READY" {
                    return Ok::<_, String>(());
                }
            }
        })
        .await;
        if !matches!(ready, Ok(Ok(()))) {
            continue;
        }
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let nonce = format!("{}-{}", std::process::id(), rand::random::<u64>());
        let command = json!({"cmd":"SET_ACTIVITY","nonce":nonce,"args":{"pid":std::process::id(),"activity":{"type":0,"details":format!("Playing {}",session["gameName"].as_str().unwrap_or("Game")),"timestamps":{"start":session["startTime"].as_i64().unwrap_or(0)/1000}}}});
        if pipe.write_all(&frame(1, &command)).await.is_err() {
            continue;
        }
        let mut confirmed = false;
        let ack_deadline = std::time::Instant::now() + std::time::Duration::from_millis(2500);
        loop {
            if cancel.load(Ordering::SeqCst) {
                return true;
            }
            let incoming = tokio::time::timeout(
                std::time::Duration::from_millis(250),
                read(&mut pipe, &mut buffer),
            )
            .await;
            match incoming {
                Ok(Ok((3, payload))) => {
                    if pipe.write_all(&frame(4, &payload)).await.is_err() {
                        return false;
                    }
                }
                Ok(Ok((1, payload))) => {
                    if payload["nonce"] == nonce {
                        if payload["evt"] == "ERROR" {
                            *result.lock().unwrap() = json!({"state":"rejected","activityConfirmed":false,"errorCode":payload.pointer("/data/code")});
                            return true;
                        }
                        if payload["cmd"] == "SET_ACTIVITY" {
                            confirmed = true;
                            *result.lock().unwrap() =
                                json!({"state":"ready","activityConfirmed":true,"errorCode":null});
                        }
                    }
                }
                Ok(Ok((2, _))) | Ok(Err(_)) => return false,
                _ => {}
            }
            if !confirmed && std::time::Instant::now() > ack_deadline {
                return false;
            }
        }
    }
    false
}
#[cfg(not(windows))]
async fn connect(_: &Value, _: &AtomicBool, _: &Mutex<Value>) -> bool {
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn fragmented_combined_and_oversized_frames() {
        let one = frame(1, &json!({"evt":"READY"}));
        let two = frame(3, &json!({"ping":1}));
        let mut buffer = one[..5].to_vec();
        assert!(pop_frame(&mut buffer).unwrap().is_none());
        buffer.extend_from_slice(&one[5..]);
        buffer.extend_from_slice(&two);
        assert_eq!(pop_frame(&mut buffer).unwrap().unwrap().1["evt"], "READY");
        assert_eq!(pop_frame(&mut buffer).unwrap().unwrap().0, 3);
        assert!(buffer.is_empty());
        buffer.extend(1u32.to_le_bytes());
        buffer.extend((1024 * 1024 + 1u32).to_le_bytes());
        assert!(pop_frame(&mut buffer).is_err());
    }
    #[test]
    fn little_endian_framing() {
        let value = json!({"v":1});
        let bytes = frame(3, &value);
        assert_eq!(&bytes[..4], &3u32.to_le_bytes());
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
            bytes.len() - 8
        );
        assert_eq!(serde_json::from_slice::<Value>(&bytes[8..]).unwrap(), value);
    }
}
