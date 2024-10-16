use gitlab::api::merge_requests::MergeRequestState;
use gitlab::api::projects::merge_requests::MergeRequests;
use gitlab::api::projects::pipelines::Pipelines;
use gitlab::api::{projects, Query};
use gitlab::webhooks::{MergeStatus, StatusState};
use gitlab::{api, Gitlab};
use serde::Deserialize;
use std::thread;
use std::time::Duration;

// добавить resolved на сообщения бота
pub struct GitlabBot {
    pub client: Gitlab,
    pub project: String,
    pub name: String,
}

impl GitlabBot {
    pub fn new(host: String, token: String, project: String, name: String) -> GitlabBot {
        let client = Gitlab::new(host, token).unwrap();
        Self {
            client,
            project,
            name,
        }
    }

    pub(crate) fn run(self) {
        self.reassign_cannotbemerged_to_author();
        //self.reassing_failed_to_author();
        self.merge_all(self.get_mrs());

        thread::sleep(Duration::from_secs(5));

        self.cancel_not_rebased_pipelines(self.get_mrs());
        self.rebase_all(self.get_mrs());
    }

    fn reassign_cannotbemerged_to_author(&self) {
        self
            .get_mrs()
            .into_iter()
            .filter(|mr| mr.merge_status == MergeStatus::CannotBeMerged)
            .for_each(|mr| {
                self.set_assignee(&mr);
                let message = format!("MR has merge conflict. Help me @{:}", mr.author.username);
                self.create_discussion_note(mr, message);
            });
    }

    fn reassing_failed_to_author(&self) {
        self
            .get_mrs()
            .into_iter()
            .filter(|mr| {
                match self.get_pipelines(mr) {
                    Some(pipeline) => {
                        pipeline.status == StatusState::Failed
                    }
                    None => {
                        false
                    }
                }
            })
            .for_each(|mr| {
                self.set_assignee(&mr);
                let message = format!("MR has failed pipeline. Help me @{:}", mr.author.username);
                self.create_discussion_note(mr, message);
            });
    }

    fn set_assignee(&self, mr: &SelfMergeRequest) {
        let request = projects::merge_requests::EditMergeRequest::builder()
            .project(self.project.as_str())
            .merge_request(mr.iid)
            .assignee(mr.author.id)
            .build()
            .unwrap();


        match api::ignore(request).query(&self.client) {
            Ok(_) => {
                println!("reasing autor {:}", mr.title)
            }
            Err(e) => {
                println!("fail reasing autor {:} {:?}", mr.title, e)
            }
        }
    }


    fn merge_all(&self, mrs: Vec<SelfMergeRequest>) {
        mrs.into_iter().for_each(|mr| {
            self.merge(&mr);
        });
    }

    fn rebase_all(&self, mut mrs: Vec<SelfMergeRequest>) {
        mrs
            .iter()
            .filter(|mr| !mr.has_conflicts)
            .filter(|mr| !self.is_rebased(&mr))
            .for_each(|mr| {
                self.rebase(mr);
            });
    }

    fn cancel_not_rebased_pipelines(&self, mrs: Vec<SelfMergeRequest>) {
        mrs
            .iter()
            .filter(|mr| !self.is_rebased(&mr))
            .for_each(|mr| {
                let pipeline = self.get_pipelines(mr);
                match pipeline {
                    None => {}
                    Some(pipeline) => {
                        let cancel_request = projects::pipelines::CancelPipeline::builder()
                            .project(self.project.as_str())
                            .pipeline(pipeline.id)
                            .build()
                            .unwrap();
                        match api::ignore(cancel_request).query(&self.client) {
                            Ok(_) => {
                                println!("cancel pipeline {:?}", mr.title)
                            }
                            Err(e) => {
                                println!("error cancel pipeline {:?}", mr.title)
                            }
                        }
                    }
                }
            });
    }


    fn rebase(&self, merge_request: &SelfMergeRequest) -> bool {
        let merge_request_iid = merge_request.iid;
        let rebase = projects::merge_requests::RebaseMergeRequest::builder()
            .project(self.project.as_str())
            .merge_request(merge_request_iid)
            .build()
            .unwrap();
        match api::raw(rebase).query(&self.client) {
            Ok(r) => {
                println!("rebase {:?} {:?}", merge_request.title, String::from_utf8(r));
                true
            }
            Err(e) => {
                println!("rebase error {:?} {:?}", merge_request.title, e);
                false
            }
        }
    }

    fn merge(&self, merge_request: &SelfMergeRequest) {
        let merge_request_iid = merge_request.iid;
        let rebase = projects::merge_requests::MergeMergeRequest::builder()
            .project(self.project.as_str())
            .merge_request(merge_request_iid)
            .build()
            .unwrap();
        match api::ignore(rebase).query(&self.client) {
            Ok(_) => {
                println!("merged {:?}", merge_request.title);
            }
            Err(e) => {
                println!("merge error {:?} {:?}", merge_request.title, e);
            }
        }
    }


    fn get_mrs(&self) -> Vec<SelfMergeRequest> {
        let mrs: Vec<SelfMergeRequest> = MergeRequests::builder()
            .project(self.project.as_str())
            .state(MergeRequestState::Opened)
            .build()
            .unwrap()
            .query(&self.client)
            .unwrap();

        mrs
            .into_iter()
            .filter(|item| self.is_assignee_to_marge_bot(item))
            .collect()
    }

    fn get_pipelines(&self, mr: &SelfMergeRequest) -> Option<SelfPipeline> {
        let pp: Vec<SelfPipeline> = match &mr.sha {
            None => Vec::new(),
            Some(sha) => Pipelines::builder()
                .project(self.project.as_str())
                .sha(sha)
                .build()
                .unwrap()
                .query(&self.client)
                .unwrap(),
        };

        pp.into_iter().nth(0)
    }

    fn get_branch_commit(&self, branch: &str) -> Vec<SelfCommit> {
        let commit_request = projects::repository::commits::Commits::builder()
            .project(self.project.as_str())
            .ref_name(branch)
            .build()
            .unwrap();
        let commits: Vec<SelfCommit> = commit_request.query(&self.client).unwrap();
        commits
    }

    fn is_rebased(&self, mr: &SelfMergeRequest) -> bool {
        let target_commits = self.get_branch_commit(mr.target_branch.as_str());
        if target_commits.len() == 0 {
            false
        } else {
            let source_commits = self.get_branch_commit(mr.source_branch.as_str());
            source_commits.iter().any(|c| c.id == target_commits[0].id)
        }
    }

    fn is_assignee_to_marge_bot(&self, mr: &SelfMergeRequest) -> bool {
        match mr.assignees.as_ref() {
            None => false,
            Some(users) => users.iter().any(|user| user.name == self.name),
        }
    }

    fn create_discussion_note(&self, mr: SelfMergeRequest, message: String) {
        let note_request = projects::merge_requests::notes::CreateMergeRequestNote::builder()
            .project(self.project.as_str())
            .merge_request(mr.iid)
            .body(message)
            .build()
            .unwrap();

        match api::ignore(note_request).query(&self.client) {
            Ok(_) => {
                println!("create note in {:}", mr.title)
            }
            Err(e) => {
                println!("error create note {:?}", e)
            }
        }
    }
}


#[derive(Deserialize)]
pub struct SelfMergeRequest {
    has_conflicts: bool,
    iid: u64,
    title: String,
    sha: Option<String>,
    assignees: Option<Vec<SelfUser>>,
    target_branch: String,
    source_branch: String,
    merge_status: MergeStatus,
    author: SelfUser,
}

#[derive(Deserialize)]
pub struct SelfUser {
    name: String,
    username: String,
    id: u64,
}

#[derive(Deserialize)]
pub struct SelfPipeline {
    status: StatusState,
    id: u64,
}


#[derive(Deserialize)]
pub struct SelfCommit {
    id: String,
}