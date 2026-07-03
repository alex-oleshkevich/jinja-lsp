// REQ-REL-01..14: CI and release workflow static verification.
// These tests parse the workflow YAML and verify required structural properties
// without running the actual CI pipeline.

use std::collections::HashSet;

fn ci_workflow() -> serde_yaml::Value {
    let raw = include_str!("../.github/workflows/ci.yml");
    serde_yaml::from_str(raw).expect("ci.yml must be valid YAML")
}

fn release_workflow() -> serde_yaml::Value {
    let raw = include_str!("../.github/workflows/release.yml");
    serde_yaml::from_str(raw).expect("release.yml must be valid YAML")
}

// ─── REQ-REL-01: Lint gate has clippy and rustfmt ────────────────────────────

#[test]
fn ci_lint_job_runs_clippy_and_rustfmt() {
    let ci = ci_workflow();
    let lint_steps = ci["jobs"]["lint"]["steps"]
        .as_sequence()
        .expect("lint steps");
    let step_content: String = serde_yaml::to_string(lint_steps).unwrap();
    assert!(
        step_content.contains("rustfmt"),
        "lint job must run rustfmt"
    );
    assert!(step_content.contains("clippy"), "lint job must run clippy");
}

// ─── REQ-REL-02: Test gate uses cargo nextest ────────────────────────────────

#[test]
fn ci_test_job_uses_nextest() {
    let ci = ci_workflow();
    let test_steps = ci["jobs"]["test"]["steps"]
        .as_sequence()
        .expect("test steps");
    let step_content: String = serde_yaml::to_string(test_steps).unwrap();
    assert!(
        step_content.contains("nextest"),
        "test job must use cargo nextest"
    );
}

// ─── REQ-REL-03: Python E2E job runs pytest-lsp ──────────────────────────────

#[test]
fn ci_e2e_job_runs_pytest_lsp() {
    let ci = ci_workflow();
    let e2e_steps = ci["jobs"]["e2e"]["steps"].as_sequence().expect("e2e steps");
    let step_content: String = serde_yaml::to_string(e2e_steps).unwrap();
    assert!(
        step_content.contains("pytest-lsp"),
        "e2e job must install pytest-lsp"
    );
    assert!(step_content.contains("pytest"), "e2e job must run pytest");
}

// ─── REQ-REL-04: Lint and test matrix covers three OSes ──────────────────────

#[test]
fn ci_matrix_covers_three_oses() {
    let ci = ci_workflow();
    for job in &["lint", "test"] {
        let matrix_os = ci["jobs"][*job]["strategy"]["matrix"]["os"]
            .as_sequence()
            .unwrap_or_else(|| panic!("{job} must have matrix.os"));
        let oses: HashSet<&str> = matrix_os.iter().filter_map(|v| v.as_str()).collect();
        assert!(oses.contains("ubuntu-latest"), "{job}: must include ubuntu");
        assert!(oses.contains("macos-latest"), "{job}: must include macos");
        assert!(
            oses.contains("windows-latest"),
            "{job}: must include windows"
        );
    }
}

// ─── REQ-REL-05: Release builds four cross-compiled targets ──────────────────

#[test]
fn release_builds_four_targets() {
    let rel = release_workflow();
    let matrix = rel["jobs"]["build"]["strategy"]["matrix"]["include"]
        .as_sequence()
        .expect("build matrix.include");
    let targets: HashSet<&str> = matrix.iter().filter_map(|e| e["target"].as_str()).collect();

    assert!(
        targets.contains("x86_64-unknown-linux-gnu"),
        "must build linux x86_64"
    );
    assert!(
        targets.contains("aarch64-unknown-linux-gnu"),
        "must build linux aarch64"
    );
    assert!(
        targets.contains("aarch64-apple-darwin"),
        "must build macos arm64"
    );
    assert!(
        targets.contains("x86_64-pc-windows-msvc"),
        "must build windows x86_64"
    );
    assert_eq!(targets.len(), 4, "must build exactly 4 targets");
}

// ─── REQ-REL-09: Release workflow gates on tag == Cargo.toml version ─────────

#[test]
fn release_has_version_gate_job() {
    let rel = release_workflow();
    let jobs = rel["jobs"].as_mapping().expect("jobs");
    assert!(
        jobs.contains_key("version-gate"),
        "release.yml must have a version-gate job"
    );
}

// ─── REQ-REL-11: Both CI and release use --locked ────────────────────────────

#[test]
fn workflows_use_locked_flag() {
    let ci = ci_workflow();
    let rel = release_workflow();
    let ci_str = serde_yaml::to_string(&ci).unwrap();
    let rel_str = serde_yaml::to_string(&rel).unwrap();
    assert!(ci_str.contains("--locked"), "ci.yml must use --locked");
    assert!(
        rel_str.contains("--locked"),
        "release.yml must use --locked"
    );
}

// ─── REQ-REL-13: Release generates provenance attestation ────────────────────

#[test]
fn release_generates_provenance() {
    let rel = release_workflow();
    let build_steps = rel["jobs"]["build"]["steps"]
        .as_sequence()
        .expect("build steps");
    let step_content: String = serde_yaml::to_string(build_steps).unwrap();
    assert!(
        step_content.contains("attest") || step_content.contains("provenance"),
        "build job must generate provenance attestation"
    );
}

// ─── REQ-REL-08: CHANGELOG exists and has unreleased section ─────────────────

#[test]
fn changelog_exists_and_has_unreleased() {
    let changelog = include_str!("../CHANGELOG.md");
    assert!(
        changelog.contains("Unreleased"),
        "CHANGELOG.md must have an [Unreleased] section"
    );
    assert!(
        changelog.contains("SemVer")
            || changelog.contains("semver")
            || changelog.contains("Keep a Changelog"),
        "CHANGELOG.md must reference versioning policy"
    );
}

// ─── REQ-REL-10: Maturin wheels job covers four platforms ────────────────────

#[test]
fn release_builds_maturin_wheels() {
    let rel = release_workflow();
    let rel_str = serde_yaml::to_string(&rel).unwrap();
    assert!(
        rel_str.contains("maturin"),
        "release.yml must use maturin for PyPI wheels"
    );
}

// ─── REQ-REL-14: Publish ordering — GitHub Release before crates.io before PyPI ─

#[test]
fn release_publish_job_depends_on_build_and_wheels() {
    let rel = release_workflow();
    let needs = rel["jobs"]["publish"]["needs"]
        .as_sequence()
        .expect("publish.needs");
    let deps: Vec<&str> = needs.iter().filter_map(|v| v.as_str()).collect();
    assert!(deps.contains(&"build"), "publish must need: build");
    assert!(deps.contains(&"wheels"), "publish must need: wheels");
}
