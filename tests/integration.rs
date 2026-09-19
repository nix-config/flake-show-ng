//! `fsn` integration suite root: env gate, prerequisites, and the shared driver.
//!
//! Every scenario is data in `common::scenarios::SCENARIOS`; this file only
//! decides whether the suite runs (the `FSN_IT` gate) and hands the fixture root
//! to the harness.  When `FSN_IT` is unset the suite prints its skip line and
//! succeeds, which keeps bare `cargo test` and the `nix build` sandbox-safe.

use std::env;

#[path = "common/mod.rs"]
mod common;

#[test]
fn integration_scenarios() {
    if env::var("FSN_IT").ok().as_deref() != Some("1") {
        println!("skip: integration tests are gated behind FSN_IT=1");
        return;
    }
    let tests = common::tests_dir();
    let preflight = common::preflight(&tests);
    common::run_all(&tests, &preflight);
}
