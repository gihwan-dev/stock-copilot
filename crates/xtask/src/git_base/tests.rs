use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::harness_workspace_tests::{fresh_directory, repository_root};

use super::{GRAFT_FILE, HEAD, REDIRECTING_VARIABLES, git, git_with, output, resolve_base};

const IDENTITY: [&str; 6] = [
    "-c",
    "user.name=harness",
    "-c",
    "user.email=harness@example.invalid",
    "-c",
    "commit.gpgsign=false",
];

#[derive(Debug)]
struct Repositories {
    real: PathBuf,
    fake: PathBuf,
    ignore: PathBuf,
    shallow: PathBuf,
}

#[derive(Debug)]
struct Case {
    variable: &'static str,
    environment: Vec<(String, String)>,
    probe: Vec<String>,
}

fn plain(directory: &Path, arguments: &[&str]) -> String {
    let finished = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(IDENTITY)
        .args(arguments)
        .output()
        .expect("run git");
    assert!(finished.status.success(), "git {arguments:?}");
    String::from_utf8_lossy(&finished.stdout).trim().to_owned()
}

fn repositories(name: &str) -> Repositories {
    let root = fresh_directory(&repository_root(), name);
    let real = root.join("real");
    let fake = root.join("fake");
    for directory in [&real, &fake] {
        fs::create_dir_all(directory).expect("create repository");
        plain(directory, &["init", "--quiet", "--initial-branch=main"]);
    }
    fs::write(fake.join("fake.rs"), "fn fake() {}\n").expect("write fake file");
    plain(&fake, &["add", "fake.rs"]);
    plain(&fake, &["commit", "--quiet", "-m", "fake"]);
    fs::write(fake.join("stranger.rs"), "fn stranger() {}\n").expect("write stranger");
    fs::write(real.join("a.rs"), "fn first() {}\n").expect("write first");
    plain(&real, &["add", "a.rs"]);
    plain(&real, &["commit", "--quiet", "-m", "first"]);
    plain(&real, &["switch", "--quiet", "-c", "attributes"]);
    fs::write(real.join(".gitattributes"), "* -diff\n").expect("write attributes");
    plain(&real, &["add", ".gitattributes"]);
    plain(&real, &["commit", "--quiet", "-m", "attributes"]);
    plain(&real, &["switch", "--quiet", "main"]);
    fs::write(real.join("a.rs"), "fn first() {}\nfn second() {}\n").expect("write second");
    plain(&real, &["commit", "--quiet", "--all", "-m", "second"]);
    fs::write(
        real.join("a.rs"),
        "fn first() {}\nfn second() {}\nfn third() {}\n",
    )
    .expect("write working change");
    fs::write(real.join("intruder.rs"), "fn intruder() {}\n").expect("write untracked");
    let ignore = root.join("ignore");
    fs::write(&ignore, "intruder.rs\n").expect("write ignore file");
    let shallow = root.join("shallow");
    fs::write(
        &shallow,
        format!("{}\n", plain(&real, &["rev-parse", "HEAD"])),
    )
    .expect("write shallow file");
    Repositories {
        real,
        fake,
        ignore,
        shallow,
    }
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

fn case(variable: &'static str, environment: &[(&str, String)], probe: &[&str]) -> Case {
    Case {
        variable,
        environment: environment
            .iter()
            .map(|(name, value)| (String::from(*name), value.clone()))
            .collect(),
        probe: probe.iter().map(|part| String::from(*part)).collect(),
    }
}

const UNTRACKED: [&str; 3] = ["ls-files", "--others", "--exclude-standard"];

const DIFF: [&str; 5] = ["diff", "--no-ext-diff", "--no-color", "HEAD", "--"];

fn repository_cases(repositories: &Repositories) -> Vec<Case> {
    let fake_git = repositories.fake.join(".git");
    let fake_head = plain(&repositories.fake, &["rev-parse", "HEAD"]);
    let untracked = UNTRACKED;
    vec![
        case(
            "GIT_DIR",
            &[("GIT_DIR", text(&fake_git))],
            &["rev-parse", "HEAD"],
        ),
        case(
            "GIT_WORK_TREE",
            &[("GIT_WORK_TREE", text(&repositories.fake))],
            &untracked,
        ),
        case(
            "GIT_INDEX_FILE",
            &[("GIT_INDEX_FILE", text(&fake_git.join("index")))],
            &["ls-files", "--cached"],
        ),
        case(
            "GIT_OBJECT_DIRECTORY",
            &[("GIT_OBJECT_DIRECTORY", text(&fake_git.join("objects")))],
            &["cat-file", "-t", HEAD],
        ),
        case(
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            &[(
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                text(&fake_git.join("objects")),
            )],
            &["cat-file", "-t", fake_head.as_str()],
        ),
        case(
            "GIT_COMMON_DIR",
            &[("GIT_COMMON_DIR", text(&fake_git))],
            &["cat-file", "-t", HEAD],
        ),
    ]
}

fn sibling(repositories: &Repositories, name: &str) -> PathBuf {
    repositories
        .real
        .parent()
        .map_or_else(PathBuf::new, |parent| parent.join(name))
}

fn configuration_cases(repositories: &Repositories) -> Vec<Case> {
    let ignore = text(&repositories.ignore);
    let config = sibling(repositories, "config");
    fs::write(&config, format!("[core]\n\texcludesFile = {ignore}\n")).expect("write config");
    let attributes = sibling(repositories, "attributes");
    fs::write(&attributes, "* -diff\n").expect("write attributes");
    let system = sibling(repositories, "system");
    fs::write(
        &system,
        format!("[core]\n\tattributesFile = {}\n", text(&attributes)),
    )
    .expect("write system config");
    let untracked = UNTRACKED;
    let diff = DIFF;
    vec![
        case(
            "GIT_SHALLOW_FILE",
            &[("GIT_SHALLOW_FILE", text(&repositories.shallow))],
            &["rev-list", "--count", HEAD],
        ),
        case(
            "GIT_CONFIG_GLOBAL",
            &[("GIT_CONFIG_GLOBAL", text(&config))],
            &untracked,
        ),
        case(
            "GIT_CONFIG_SYSTEM",
            &[("GIT_CONFIG_SYSTEM", text(&system))],
            &diff,
        ),
        case(
            "GIT_CONFIG_COUNT",
            &[
                ("GIT_CONFIG_COUNT", String::from("1")),
                ("GIT_CONFIG_KEY_0", String::from("core.excludesFile")),
                ("GIT_CONFIG_VALUE_0", ignore.clone()),
            ],
            &untracked,
        ),
        case(
            "GIT_CONFIG_PARAMETERS",
            &[(
                "GIT_CONFIG_PARAMETERS",
                format!("'core.excludesfile'='{ignore}'"),
            )],
            &untracked,
        ),
        case(
            "GIT_DIFF_OPTS",
            &[("GIT_DIFF_OPTS", String::from("--unified=0"))],
            &diff,
        ),
        case(
            "GIT_ATTR_SOURCE",
            &[("GIT_ATTR_SOURCE", String::from("attributes"))],
            &diff,
        ),
    ]
}

fn cases(repositories: &Repositories) -> Vec<Case> {
    let mut all = repository_cases(repositories);
    all.extend(configuration_cases(repositories));
    all
}

fn probe(mut command: Command, arguments: &[String]) -> Result<Vec<u8>, String> {
    command.args(arguments);
    output(command, "probe")
}

#[test]
fn every_redirecting_variable_has_a_case() {
    let repositories = repositories("git-base-cases");
    let covered: Vec<&str> = cases(&repositories)
        .iter()
        .map(|case| case.variable)
        .collect();
    assert_eq!(covered, REDIRECTING_VARIABLES.to_vec());
}

#[test]
fn redirecting_variables_change_plain_git_but_not_the_harness() {
    let repositories = repositories("git-base-redirect");
    let mismatches: Vec<String> = cases(&repositories)
        .iter()
        .filter_map(|case| {
            let clean = probe(git(&repositories.real), &case.probe);
            let mut exposed = Command::new("git");
            exposed
                .arg("-C")
                .arg(&repositories.real)
                .envs(case.environment.iter().map(|(name, value)| (name, value)));
            let control = probe(exposed, &case.probe);
            let guarded = probe(git_with(&repositories.real, &case.environment), &case.probe);
            if control == clean {
                Some(format!(
                    "{}: the hostile value changes nothing: {clean:?}",
                    case.variable
                ))
            } else if guarded == clean {
                None
            } else {
                Some(format!("{}: reached the harness git", case.variable))
            }
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn grafts_cannot_make_a_commit_an_ancestor() {
    let repositories = repositories("git-base-grafts");
    let real = &repositories.real;
    let side = plain(real, &["rev-parse", "attributes"]);
    let head = plain(real, &["rev-parse", "HEAD"]);
    assert!(
        resolve_base(real, &side).is_err(),
        "side commit is not an ancestor"
    );
    let grafts = real.join(".git/info/grafts");
    fs::create_dir_all(grafts.parent().expect("info directory")).expect("create info");
    fs::write(&grafts, format!("{head} {side}\n")).expect("write grafts");
    let control = Command::new("git")
        .arg("-C")
        .arg(real)
        .args(["merge-base", "--is-ancestor", &side, &head])
        .status()
        .expect("run git");
    assert!(control.success(), "plain git follows grafts");
    assert!(resolve_base(real, &side).is_err(), "harness ignores grafts");
    let hostile = [(String::from(GRAFT_FILE.0), grafts.display().to_string())];
    let status = git_with(real, &hostile)
        .args(["merge-base", "--is-ancestor", &side, &head])
        .status()
        .expect("run git");
    assert!(!status.success(), "harness overrides GIT_GRAFT_FILE");
    assert_eq!(
        resolve_base(real, HEAD).expect("HEAD is its own ancestor"),
        head
    );
}
