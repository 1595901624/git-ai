use assert_cmd::Command;
use git_ai::git::repo_storage::RepoStorage;
use git_ai::git::test_utils::TmpRepo;

#[test]
fn mock_ai_checkpoint_accepts_agent_id_arguments() {
    let repo = TmpRepo::new().unwrap();
    repo.write_file("README.md", "AI-authored content\n", false)
        .unwrap();

    Command::cargo_bin("git-ai")
        .unwrap()
        .current_dir(repo.path())
        .args([
            "checkpoint",
            "mock_ai",
            "README.md",
            "--tool",
            "custom-codex",
            "--id",
            "session-123",
            "--model",
            "gpt-test",
        ])
        .assert()
        .success();

    let storage = RepoStorage::for_repo_path(repo.repo().path());
    let checkpoints = storage
        .working_log_for_base_commit("initial")
        .read_all_checkpoints()
        .unwrap();
    let agent_id = checkpoints
        .last()
        .and_then(|checkpoint| checkpoint.agent_id.as_ref())
        .expect("mock_ai checkpoint should store an AgentId");

    assert_eq!(agent_id.tool, "custom-codex");
    assert_eq!(agent_id.id, "session-123");
    assert_eq!(agent_id.model, "gpt-test");
}

#[test]
fn mock_ai_checkpoint_preserves_default_agent_id() {
    let repo = TmpRepo::new().unwrap();
    repo.write_file("README.md", "AI-authored content\n", false)
        .unwrap();

    Command::cargo_bin("git-ai")
        .unwrap()
        .current_dir(repo.path())
        .args(["checkpoint", "mock_ai", "README.md"])
        .assert()
        .success();

    let storage = RepoStorage::for_repo_path(repo.repo().path());
    let checkpoints = storage
        .working_log_for_base_commit("initial")
        .read_all_checkpoints()
        .unwrap();
    let agent_id = checkpoints
        .last()
        .and_then(|checkpoint| checkpoint.agent_id.as_ref())
        .expect("mock_ai checkpoint should store an AgentId");

    assert_eq!(agent_id.tool, "some-ai");
    assert_eq!(agent_id.id, "ai-thread");
    assert_eq!(agent_id.model, "unknown");
}
