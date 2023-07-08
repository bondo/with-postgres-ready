# with_postgres_ready

with_postgres_ready makes it easy to write tests that relies on a postgres database being ready to accept connections.
It does this by starting a Docker container with postgres, polling the database until it is ready, and then executing the block.

# Examples

To get a connection url with the default configuration:

```rust
use with_postgres_ready::*;

#[test_log::test]
fn it_can_use_defaults() {
    with_postgres_ready(|url| async move {
        // Connect to the database using the url.
    });
}
```

To get more control, use the `Runner` builder:

```rust
use with_postgres_ready::*;

#[test_log::test]
fn it_can_use_custom_connection_timeout() {
    Runner::new().connection_timeout(Duration::from_secs(5)).run(|url| async move {
        // Connect to the database using the url.
    });
}
```
