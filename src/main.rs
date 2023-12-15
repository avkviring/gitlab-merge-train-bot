mod gitlab_bot;

use crate::gitlab_bot::GitlabBot;
use std::env;
use std::thread::sleep;
use std::time::Duration;

// 2 - запускать два rebase для каждого target-branch
fn _main() {
    loop {
        println!("start iteration");
        let bot = GitlabBot::new(
            get_env("GITLAB_HOST"),
            get_env("GITLAB_TOKEN"),
            get_env("GITLAB_PROJECT"),
            get_env("GITLAB_BOT_NAME"),
        );
        bot.run();
        println!("end iteration");
        sleep(Duration::from_secs(60));
        println!();
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
