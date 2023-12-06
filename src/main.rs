mod gitlab_bot;

use crate::gitlab_bot::GitlabBot;
use std::env;
use std::thread::sleep;
use std::time::Duration;

// 1 - если branch можно смержить - мержим
fn _main() {
    loop {
        let bot = GitlabBot::new(
            get_env("GITLAB_HOST"),
            get_env("GITLAB_TOKEN"),
            get_env("GITLAB_PROJECT"),
            get_env("GITLAB_BOT_NAME"),
        );
        bot.run();
        sleep(Duration::from_secs(60 * 15))
    }
}

fn main() {
    let bot = GitlabBot::new(
        get_env("GITLAB_HOST"),
        get_env("GITLAB_TOKEN"),
        get_env("GITLAB_PROJECT"),
        get_env("GITLAB_BOT_NAME"),
    );
    bot.run();
}

fn get_env(key: &str) -> String {
    env::var(key).expect(format!("ENV {:} not set", key).as_str())
}
