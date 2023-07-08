use std::{
    future::Future,
    panic::{self, UnwindSafe},
    time::Duration,
};

use dockertest::{waitfor::RunningWait, Composition, DockerTest, Image};
use tokio::{runtime::Handle, task, time::sleep};

const POSTGRES_PASSWORD: &str = "postgres";

pub struct Runner {
    container_tag: &'static str,
    container_timeout: Duration,
    connection_timeout: Duration,
    connection_test_interval: Duration,
}

impl Default for Runner {
    fn default() -> Self {
        Self {
            container_tag: "15.3-alpine3.18",
            container_timeout: Duration::from_secs(10),
            connection_timeout: Duration::from_secs(2),
            connection_test_interval: Duration::from_millis(100),
        }
    }
}

/// A test helper that runs a postgres container and waits for it to be ready.
impl Runner {
    /// Create a new instance of the test helper.
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the postgres image tag to use.
    /// See <https://hub.docker.com/_/postgres> for available tags.
    ///
    /// Defaults to `15.3-alpine3.18`.
    pub fn container_tag(mut self, container_tag: &'static str) -> Self {
        self.container_tag = container_tag;
        self
    }

    /// Set the container timeout for the test.
    /// The test will fail if the container is not ready within this time.
    ///
    /// Defaults to 10 seconds.
    pub fn container_timeout(mut self, container_timeout: Duration) -> Self {
        self.container_timeout = container_timeout;
        self
    }

    /// Set the connection timeout for the test.
    /// The test will fail if the connection is not established within this time.
    ///
    /// Defaults to 2 seconds.
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set the interval between connection attempts.
    ///
    /// Defaults to 100 milliseconds.
    pub fn connection_test_interval(mut self, connection_test_interval: Duration) -> Self {
        self.connection_test_interval = connection_test_interval;
        self
    }

    /// Run the test.
    /// The test will be passed a postgres connection url.
    /// The test will fail if the connection is not established within the connection timeout.
    /// The test will fail if the test function panics.
    pub fn run<T, Fut>(self, f: T)
    where
        T: FnOnce(String) -> Fut + UnwindSafe,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut test = DockerTest::new().with_default_source(dockertest::Source::DockerHub);

        let image = Image::with_repository("postgres").tag(self.container_tag);
        let mut composition = Composition::with_image(image)
            .with_env(
                [(
                    "POSTGRES_PASSWORD".to_string(),
                    POSTGRES_PASSWORD.to_string(),
                )]
                .into(),
            )
            .with_wait_for(Box::new(RunningWait {
                check_interval: 1,
                max_checks: self.container_timeout.as_secs(),
            }));
        composition.publish_all_ports();
        test.add_composition(composition);

        test.run(|ops| {
            let url = {
                let handle = ops.handle("postgres");
                let (ip, port) = handle
                    .host_port(5432)
                    .expect("Should have port 5432 mapped");
                format!("postgresql://postgres:{POSTGRES_PASSWORD}@{ip}:{port}/postgres")
            };

            let has_timed_out = block_on(async {
                tokio::select! {
                    _ = self.wait_for_connection(&url) => false,
                    _ = sleep(self.connection_timeout) => true,
                }
            });

            let res = if has_timed_out {
                Ok(())
            } else {
                panic::catch_unwind(|| block_on(f(url)))
            };

            async move {
                if has_timed_out {
                    panic!(
                        "Timed out waiting for postgres to be ready after {} seconds",
                        self.connection_timeout.as_secs_f32()
                    );
                }
                if let Err(err) = res {
                    panic::resume_unwind(err);
                }
            }
        });
    }

    async fn wait_for_connection(&self, url: &str) {
        loop {
            if tokio_postgres::connect(url, tokio_postgres::NoTls)
                .await
                .is_ok()
            {
                break;
            }
            sleep(self.connection_test_interval).await;
        }
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    task::block_in_place(|| Handle::current().block_on(future))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test]
    fn it_can_use_bullseye_tag() {
        Runner::new()
            .container_tag("12.15-bullseye")
            .run(|url| async move {
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
