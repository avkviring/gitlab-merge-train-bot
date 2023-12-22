## Intro

Gitlab bot for merge train improvment.

Every run:

- merge all ready MR with assignee marge-bot
- rebase all MR with assignee marge-bot
- cancel pipeline for not rebased MR with assignee marge-bot
- notitfy user about problem

## Setup

Create user in Gitlab (marge-bot).
Create token for user.

Configure CI variables:
- GITLAB_HOST
- GITLAB_TOKEN
- GITLAB_PROJECT
- GITLAB_BOT_NAME


Add stage and setup Gitlab Schedules for run.

```
stages:  
  - bot  

run-bot:
  only:
    - schedules
  stage: bot
  image:  ghcr.io/avkviring/gitlab-merge-train-bot:0.0.13
  script:
    - /gitlab-merge-train-bot
```

