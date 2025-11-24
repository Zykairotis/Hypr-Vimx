use std::path::PathBuf;

pub const UNIX_DOMAIN_SOCKET_FILE: &str = "/tmp/hints.socket";
pub const SOCKET_MESSAGE_SIZE: usize = 1024;
pub const DEFAULT_ALPHABET: &str = "asdfgqwertzxcvbhjklyuiopnm";

pub fn default_config_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".config")
        .join("hints")
        .join("config.json")
}
