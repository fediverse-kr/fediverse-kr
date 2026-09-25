# Dependency and contribution checks

`scripts/check-contribution.py` is the local verification entrypoint. GitHub CI/Actions is not configured or run: the maintainer verifies, builds, and deploys directly from the maintainer environment. The checker intentionally checks only the supported build graphs, rather than `--all-features` (which would pull unsupported desktop and mobile dependencies):

- `wasm32-unknown-unknown` with `web`
- `aarch64-unknown-linux-gnu` with `server`
- `x86_64-unknown-linux-gnu` with `server`

## Linux setup

Install stable Rust via [rustup](https://rustup.rs/) and Python 3. On Ubuntu/Debian, install the public native prerequisites (no private registry, cluster, credentials or compiler cache is required):

```sh
sudo apt-get update
sudo apt-get install --yes build-essential pkg-config libpq-dev python3
rustup target add wasm32-unknown-unknown
cargo install cargo-deny --version 0.20.2 --locked
cargo fetch --locked
```

`cargo fetch` primes a fresh clone for the optional `--offline` commands in the README. `quick` fetches current RustSec metadata and therefore needs public network access. Then run:

```sh
python3 scripts/test-contribution-checks.py
python3 scripts/check-contribution.py quick
```

On an ARM64 Linux workstation, first run `export FEDKR_TEST_TARGET=aarch64-unknown-linux-gnu`. The default is `x86_64-unknown-linux-gnu`. Build checks and executable tests use that native server target; the WASM check and all three dependency-policy graphs run on either architecture. Merely installing a Rust cross target does not supply a cross C compiler or libpq sysroot. Executable checks on the other architecture require a suitable local/native environment; dependency-policy resolution alone does not prove that its binary was built or tested.

The quick gate runs locked WASM and native server checks, the non-ignored native Linux server test suite, a fresh RustSec database fetch, and an offline locked `cargo deny check all` for each of the three graphs. It checks all policy graphs even when one fails, then returns nonzero if any failed. Build/test failures stop the quick gate immediately. The entrypoint parses Cargo's test summaries and fails if no Rust test actually ran. Repository-wide `cargo fmt --all -- --check` is a separate formatting gate; the shared entrypoint keeps build and policy results separate from formatting results.

## Isolated PostgreSQL gate

Database tests require a disposable PostgreSQL database named exactly `fedkr_test`, reachable at the existing fixture contract's literal loopback address and port `127.0.0.1:16439`. The checker rejects remote hosts, IPv6 loopback, another database name, another port, and Unix-socket query overrides. Never point this variable at development, staging, or production data. With Docker installed, create a disposable instance without a persistent volume:

```sh
docker run --detach --rm --name fedkr-contribution-test \
  --publish 127.0.0.1:16439:5432 \
  --env POSTGRES_DB=fedkr_test --env POSTGRES_USER=fedkr_test \
  --env POSTGRES_PASSWORD=fedkr_test \
  postgres:16@sha256:a3b7f434b2dc57ce85a67e171163eb8ab1a1ebcb39d27484661f26b1dfbe30d6
docker exec fedkr-contribution-test pg_isready -U fedkr_test -d fedkr_test
```

Wait for `pg_isready` to report accepting connections before testing. If port 16439 is occupied, do not reuse or stop an unrelated database: use a separate disposable VM/container network where that port is private, or report the blocked gate.

```sh
export FEDKR_TEST_DATABASE_URL='postgresql://fedkr_test:fedkr_test@127.0.0.1:16439/fedkr_test'
python3 scripts/check-contribution.py database
```

This mode runs the server tests with `--include-ignored --test-threads=1`. The credential above is synthetic and only for this disposable instance. After the test, remove only that instance with `docker stop fedkr-contribution-test`. Run the database mode separately even when the quick gate reports an advisory failure, so dependency findings do not hide database results. No production data or credentials are used.

## `cargo deny` policy

`deny.toml` applies these rules:

- Fetch and evaluate RustSec advisories, reject yanked packages, and reject unmaintained or unsound advisories. The advisory database may be at most 14 days old. There are no blanket advisory ignores.
- Allow only the permissive SPDX licenses already accepted by the third-party notice policy, plus `Apache-2.0 WITH LLVM-exception` used by the supported graph. GPL, AGPL, and MPL are not blanket exceptions. This dependency allowlist does not license the application itself.
- Warn on duplicate transitive versions instead of forcing risky wholesale upgrades. Reject wildcard requirements in workspace manifests and ban OpenSSL/native-tls additions in favor of the existing rustls stack.
- Deny unknown registries and Git repositories. crates.io and `https://github.com/DioxusLabs/components` are the only allowed sources. Both `Cargo.toml` and `Cargo.lock` pin the latter to revision `bf007c15d0cf4d04d3181cc46cf12325aa773955`. The manifest pin makes the previously locked revision explicit; it changes the lock source IDs for `dioxus-primitives` and `dioxus-attributes`, not their source commit or package versions.

The current lock update is deliberately narrow: `h2` 0.4.16 fixes RUSTSEC-2026-0258, `rustls` 0.23.45 (and `rustls-webpki` 0.103.15) fixes RUSTSEC-2026-0285, and `chacha20` 0.10.2 replaces the yanked 0.10.1 release.

Two upstream issues remain intentionally visible and make the dependency gate fail closed:

- RUSTSEC-2023-0071 is reported for `rsa` 0.9.10 and has no patched stable release. The application uses RSA private-key signing in federation, but this check does not establish an application exploit or quantify exposure. Do not suppress the advisory without a reviewed reachability/mitigation assessment; a signing implementation change requires separate review.
- RUSTSEC-2026-0253 affects transitive `lru` 0.16.4. The fix is in 0.18.2, outside `dioxus-server` 0.7.10's compatible requirement. Resolving it requires an upstream Dioxus update or a reviewed backport, not an unconstrained lockfile change.

Do not add an advisory ignore merely to make a check pass. Record the risk and any explicit, release-specific operator acceptance without calling the security gate a PASS; do not turn it into a standing waiver. Resolving a finding requires an assessed mitigation or dependency/architecture update and verification of every supported graph.

## Local verification and deployment

GitHub is the public source and Issues/PR entry point, not a build service. Do not add Actions workflows, hosted or self-hosted GitHub CI runners, or deployment credentials. Contributors provide local commands and actual results; the maintainer performs the local verification, build, image publication, and deployment directly.

Retain the source revision, tested artifact hashes, local verification results, image digest, and rollout read-back as release evidence. The local check scripts and pinned disposable PostgreSQL image remain available without GitHub Actions.
