//! Repro / regression guard for PD-23 / GH #1677: the attribution-recovery
//! diff was unbounded.
//!
//! The daemon's fast-forward `update-ref` path calls
//! `post_commit_from_working_log(Some(old), new)` where `old` is the *old branch
//! tip from before a `git pull`*. Recovery (`recovery_committed_hunks`) then
//! diffed the entire `old..new` range with `diff_added_lines(old, new, None)`
//! (no pathspec), buffering the whole `git diff -U0` output plus one `u32` per
//! added line into memory. On a pull that fast-forwards across a large range
//! this is the 20GB+ blow-up.
//!
//! The fix bounds the recovery diff to the finalized commit's *immediate
//! parent*. This test measures process peak RSS around the real `diff_added_lines`
//! path for both the full pulled range (the bug) and the immediate-parent range
//! (the fix), proving the unbounded allocation scales with the whole range while
//! the bounded one does not.

#![cfg(target_os = "linux")]

use git_ai::git::repository::find_repository_in_path;
use std::fs;
use std::process::Command;

/// Peak resident set size of this process so far, in KiB (Linux `VmHWM`).
fn peak_rss_kb() -> u64 {
    let status = fs::read_to_string("/proc/self/status").expect("read /proc/self/status");
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest
                .trim()
                .trim_end_matches(" kB")
                .trim()
                .parse()
                .expect("parse VmHWM");
        }
    }
    panic!("VmHWM not found in /proc/self/status");
}

fn git(cwd: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .expect("git spawn");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn rev_parse(cwd: &std::path::Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Build a repo where `old` tip and the latest tip differ by a large amount of
/// added content (simulating a `git pull` that fast-forwards across many
/// commits), while the *final* commit alone is tiny. Returns (old_tip, new_tip).
fn build_large_ff_range(
    dir: &std::path::Path,
    big_commits: usize,
    lines_each: usize,
) -> (String, String) {
    git(dir, &["init", "-b", "main", "."]);
    git(dir, &["config", "user.email", "t@git-ai.local"]);
    git(dir, &["config", "user.name", "git-ai test"]);
    git(dir, &["config", "commit.gpgsign", "false"]);

    fs::write(dir.join("base.txt"), "base\n").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "old tip"]);
    let old_tip = rev_parse(dir);

    // Intervening commits a fast-forward pull would drag in: each adds a large
    // file. old_tip..new_tip therefore spans a large diff.
    let line = "the quick brown fox jumps over the lazy dog padding padding padding\n";
    for c in 0..big_commits {
        let body = line.repeat(lines_each);
        fs::write(dir.join(format!("pulled_{c}.txt")), &body).unwrap();
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-m", &format!("pulled commit {c}")]);
    }

    // The newly-pulled tip itself only changes one small file.
    fs::write(dir.join("final.txt"), "final change\n").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "final"]);
    let new_tip = rev_parse(dir);

    (old_tip, new_tip)
}

#[test]
fn recovery_full_range_diff_blows_up_memory_vs_immediate_parent() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // ~200 commits x 20k lines x ~68 bytes ≈ ~270MB of added text across
    // old..new. Kept CI-friendly while still dwarfing a single commit by orders
    // of magnitude (real reports were multi-GB of pulled history).
    let (old_tip, new_tip) = build_large_ff_range(dir, 200, 20_000);

    let repo = find_repository_in_path(dir.to_str().unwrap()).unwrap();

    // The immediate parent of new_tip is what the fix diffs instead of the
    // far-behind `old_tip`.
    let immediate_parent = repo
        .find_commit(new_tip.clone())
        .unwrap()
        .parent(0)
        .unwrap()
        .id();

    let baseline = peak_rss_kb();

    // BOUNDED path (the fix): diff only the finalized commit's own changes.
    let bounded_hunks = repo
        .diff_added_lines(&immediate_parent, &new_tip, None)
        .unwrap();
    let after_bounded = peak_rss_kb();

    // UNBOUNDED path (the bug): diff the entire old..new pulled range.
    let full_hunks = repo.diff_added_lines(&old_tip, &new_tip, None).unwrap();
    let after_full = peak_rss_kb();

    let bounded_growth = after_bounded.saturating_sub(baseline);
    let full_growth = after_full.saturating_sub(after_bounded);

    eprintln!(
        "baseline={baseline}KB after_bounded={after_bounded}KB after_full={after_full}KB \
         bounded_growth={bounded_growth}KB full_growth={full_growth}KB \
         bounded_files={} full_files={}",
        bounded_hunks.len(),
        full_hunks.len()
    );

    // The bug: the full-range diff sees every pulled file; the bounded diff sees
    // only the final commit's single file.
    assert_eq!(
        bounded_hunks.len(),
        1,
        "bounded diff must see only final.txt"
    );
    assert!(
        full_hunks.len() >= 200,
        "full-range diff materialized the whole pulled range ({} files)",
        full_hunks.len()
    );

    // The full-range diff's peak-RSS growth dwarfs the bounded path's: this is
    // the unbounded allocation that reached 20GB on real pulls. The bounded path
    // (what `recovery_committed_hunks` now uses) stays flat.
    assert!(
        full_growth > bounded_growth.saturating_mul(20).max(50_000),
        "expected full-range diff to allocate far more than the bounded diff: \
         bounded_growth={bounded_growth}KB full_growth={full_growth}KB"
    );
}
