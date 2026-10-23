# fedkr-federation

An MIT OR Apache-2.0, non-published Rust crate providing the bounded
ActivityPub account-resolution and public-post verification foundation used
by Fediverse.kr. Its default feature set is read-only. It has no Dioxus,
Diesel, member database, or environment-variable dependency; callers inject a
`FederationTransport` or construct the supplied HTTP transport with an
application-owned signer.

## Optional publishing capability

Enable `publish` to prepare an immutable public ActivityStreams `Create` with
a `Note`, then explicitly make one signed HTTP POST through
`delivery::HttpActivitySender` (or an implementation of `ActivitySender`).
The caller supplies the stable activity/object IDs, actor and followers URLs,
text, and published timestamp. Stable IDs let a host recognize its own
retries; this crate does not deduplicate sends or promise delivery exactly
once. `HttpActivitySender` performs one bounded attempt to one supplied inbox;
it does not discover recipients, fan out to followers, persist an outbox, or
schedule retries. Its `Accepted` result means the remote HTTP endpoint
returned a 2xx response, not that the post was stored, delivered onward, or
displayed to users.

This crate does not host actors, serve WebFinger/actor/outbox/object resources,
or process inbound inbox activities. Those server-side capabilities remain
unimplemented. The application's legacy `/actor/inbox` compatibility route
is a bounded no-op response and is not an inbound ActivityPub implementation.

`social_post` and `status_bot` are synthetic dry-runs: they prepare an
activity and print JSON to stdout; neither contacts an instance nor uses
real account data. Example:

```sh
cargo run -p fedkr-federation --features publish --example social_post
```

Test the unchanged read-only default and the opt-in publishing surface with:

```sh
cargo test --locked --offline -p fedkr-federation
cargo test --locked --offline -p fedkr-federation --features publish
```

## Signing advisory

The HTTP-signature implementation uses RSA (`rsa` 0.9.10). The project
dependency policy records RustSec advisory RUSTSEC-2023-0071 for this version,
with no patched stable release identified there; it also notes that this
finding alone does not establish an application exploit or quantify exposure.
The finding is not suppressed. See the [dependency policy](../../docs/dependency-policy.md)
for the project's review and verification requirements.
