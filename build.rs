use std::process::Command;

fn main() {
    // https://docs.rs/diesel_migrations/2.2.0/diesel_migrations/macro.embed_migrations.html
    println!("cargo:rerun-if-changed=migrations/");

    // embed git hash in executable, referenced with env!()
    let output = Command::new("git").args(["describe", "--always", "--dirty"]).output();
    let git_hash = match output {
        Ok(o) => String::from_utf8(o.stdout).unwrap(),
        Err(_) => "unknown".to_string(),
    };
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/master");
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rustc-env=BUILD_TIME_GIT_HASH={git_hash}");
}
