use std::{future::Future, time::Duration};

/// Call an async task function, and write a message to stdout
/// with an automatic spinner to show that we're not blocked.
/// Note that generally the called function should not output
/// anything to stdout as this will interfere with the spinner.
pub(crate) async fn async_task_with_spinner<F, T>(msg: &str, f: F) -> T
where
    F: Future<Output = T>,
{
    let pb = indicatif::ProgressBar::new_spinner();
    let style = indicatif::ProgressStyle::default_bar();
    pb.set_style(style.template("{spinner} {msg}").unwrap());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(150));
    let r = f.await;
    pb.finish_and_clear();
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spinner() {
        async_task_with_spinner("Testing...", tokio::time::sleep(Duration::from_secs(5))).await
    }
}
