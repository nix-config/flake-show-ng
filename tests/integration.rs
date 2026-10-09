//! `fsn` integration suite root: env gate, prerequisites, and the shared driver.
//!
//! Every scenario is data in `common::scenarios::SCENARIOS`; this file only
//! decides whether the suite runs and hands the fixture root to the harness.
//! The suite runs by default; `FSN_SKIP_INTEGRATION=1` disables it, printing a skip line and
//! succeeding.

use std::env;

#[path = "common/mod.rs"]
mod common;

#[test]
fn integration_scenarios() {
    if env::var("FSN_SKIP_INTEGRATION").ok().as_deref() == Some("1") {
        println!("skip: integration tests disabled by FSN_SKIP_INTEGRATION=1");
        return;
    }
    let tests = common::tests_dir();
    let preflight = common::preflight(&tests);
    common::run_all(&tests, &preflight);
}
