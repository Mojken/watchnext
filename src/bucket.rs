
struct Bucket {
    cap: u8,
    refill_tick: tokio::time::Duration,
    tokens: Mutex<u8>,
    last_refill: Mutex<tokio::time::Instant>,
}

impl Bucket {
    pub fn new(max: u8, refill_tick: tokio::time::Duration) -> Self {
        Self {
            cap: max,
            refill_tick: refill_tick,
            tokens: Mutex::new(max),
            last_refill: Mutex::new(tokio::time::Instant::now() - refill_tick),
        }
    }

    pub async fn acquire(&self) {
        trace!(target: "bucket", "Calling aquire on bucket");
        let mut tokens = self.tokens.lock().await;
        let mut last_refill = self.last_refill.lock().await;
        trace!(target: "bucket", "Acquired locks");

        let now = tokio::time::Instant::now();
        let d_tokens: u8 = (now.duration_since(*last_refill).div_duration_f32(self.refill_tick)).floor() as u8;
        *tokens = std::cmp::min(self.cap, *tokens + d_tokens);

        if *tokens > 0 {
            *last_refill = *last_refill + self.refill_tick * d_tokens.into();
            *tokens -= 1;
            trace!(target: "bucket", "Tokens were available, returning. New tokens: {}", *tokens);
        } else {
            *last_refill = *last_refill + self.refill_tick * (d_tokens + 1).into();
            info!(target: "bucket", "Tokens were not available, sleeping.");
            tokio::time::sleep_until(*last_refill).await;
            trace!(target: "bucket", "Sleeping done, returning");
        }
    }
}
