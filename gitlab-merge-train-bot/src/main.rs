mod gitlab_bot;
use crate::gitlab_bot::GitlabBot;
use std::env;
use std::thread::sleep;
use std::time::Duration;

// 1 - если branch можно смержить - мержим
fn main() {
    loop {
        let bot = GitlabBot::new(
            env::var("GITLAB_HOST").unwrap(),
            env::var("GITLAB_TOKEN").unwrap(),
            env::var("GITLAB_PROJECT").unwrap(),
            env::var("GITLAB_BOT_NAME").unwrap(),
        );
        bot.run();
        sleep(Duration::from_secs(60 * 10));
    }
}
