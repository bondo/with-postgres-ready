use std::{future::Future, panic::UnwindSafe};

use crate::Runner;

/// Run a test with a postgres container.
/// The test will be passed a postgres connection url.
pub fn with_postgres_ready<T, Fut>(f: T)
where
    T: FnOnce(String) -> Fut + UnwindSafe,
    Fut: Future<Output = ()> + Send + 'static,
{
    Runner::new().run(f);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test]
    fn it_works() {
        with_postgres_ready(|url| async move {
            let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
                .await
                .unwrap();

            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("connection error: {}", e);
                }
            });

            let rows = client
                .query("SELECT 1 + 2", &[])
                .await
                .expect("Error running query");

            assert_eq!(rows.len(), 1);

            let sum: i32 = rows[0].get(0);
            assert_eq!(sum, 3);
        });
    }
}
