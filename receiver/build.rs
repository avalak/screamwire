fn main() {
    let hash = std::env::var("GIT_COMMIT_HASH").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", hash);
}
