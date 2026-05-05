use std::time::Duration;

pub async fn retry<F, T, E>(mut operation: F, max_retries: usize) -> std::result::Result<T, E>
where
    F: AsyncFnMut() -> std::result::Result<T, E>,
{
    let mut attempt = 0;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) if attempt < max_retries => {
                attempt += 1;
                tokio::time::sleep(backoff(attempt)).await;
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }
}

fn backoff(attempt: usize) -> Duration {
    Duration::from_millis(100 * 2_u64.pow(attempt.saturating_sub(1) as u32))
}

#[cfg(test)]
mod tests {
    use super::backoff;

    #[test]
    fn backoff_grows() {
        assert!(backoff(2) > backoff(1));
    }
}
