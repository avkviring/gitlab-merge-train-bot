use std::thread;
use std::time::Duration;
use gitlab::api::projects::merge_requests::{MergeRequestState, MergeRequests};
use gitlab::api::projects::pipelines::Pipelines;
use gitlab::api::{projects, Query};
use gitlab::{api, Commit, Gitlab, MergeRequest, MergeStatus, PipelineBasic, StatusState};
use rand::seq::SliceRandom;
use rand::thread_rng;

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
        self.reassing_failed_to_author();
        self.merge_all(self.get_mrs());
        self.rebase_first(self.get_mrs());
    }

    fn reassign_cannotbemerged_to_author(&self) {
        self
            .get_mrs()
            .into_iter()
            .filter(|item| item.0.merge_status == MergeStatus::CannotBeMerged)
            .for_each(|item| {
                self.set_assignee(&item);
                let message = format!("MR has merge conflict. Help me @{:}", item.0.author.username);
                self.create_discussion_comment(item.0, message);
            });
    }

    fn reassing_failed_to_author(&self) {
        self
            .get_mrs()
            .into_iter()
            .filter(|item| item.1.iter().any(|p| p.status == StatusState::Failed))
            .for_each(|item| {
                self.set_assignee(&item);
                let message = format!("MR has failed pipeline. Help me @{:}", item.0.author.username);
                self.create_discussion_comment(item.0, message);
            });
    }

    fn set_assignee(&self, item: &(MergeRequest, Vec<PipelineBasic>)) {
        let request = projects::merge_requests::EditMergeRequest::builder()
            .project(self.project.as_str())
            .merge_request(item.0.iid.value())
            .assignee(item.0.author.id.value())
            .build()
            .unwrap();


        match api::ignore(request).query(&self.client) {
            Ok(_) => {
                println!("reasing autor {:?}", item.0.title)
            }
            Err(e) => {
                println!("fail reasing autor {:?} {:?}", item.0.title, e)
            }
        }
    }

    fn create_discussion_comment(&self, mr: MergeRequest, message: String) {
        let discussions_request = projects::merge_requests::discussions::CreateMergeRequestDiscussion::builder()
            .project(self.project.as_str())
            .merge_request(mr.iid.value())
            .body(message)
            .build()
            .unwrap();
        api::ignore(discussions_request).query(&self.client).unwrap();
    }

    fn merge_all(&self, mrs: Vec<(MergeRequest, Vec<PipelineBasic>)>) {
        mrs.into_iter().for_each(|item| {
            self.merge(&item.0);
        });
    }

    fn rebase_first(&self, mut mrs: Vec<(MergeRequest, Vec<PipelineBasic>)>) {
        let mut commits = self.get_branch_commit("main");
        let first_main_commits = commits.remove(0);
        mrs.sort_by_key(|mr| mr.0.iid.value());
        mrs
            .iter()
            .filter(|item| !item.0.has_conflicts)
            .filter(|item| !self.is_rebased(&first_main_commits, item))
            .take(2)
            .for_each(|item| {
                self.rebase(&item.0);
            });
    }


    fn rebase(&self, merge_request: &MergeRequest) -> bool {
        let merge_request_iid = merge_request.iid.value();
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

    fn merge(&self, merge_request: &MergeRequest) {
        let merge_request_iid = merge_request.iid.value();
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

    fn get_mrs(&self) -> Vec<(MergeRequest, Vec<PipelineBasic>)> {
        let mrs: Vec<MergeRequest> = MergeRequests::builder()
            .project(self.project.as_str())
            .state(MergeRequestState::Opened)
            .build()
            .unwrap()
            .query(&self.client)
            .unwrap();

        mrs
            .into_iter()
            .filter(|item| self.is_assignee_to_marge_bot(item))
            .map(|mr| {
                let pipelines = self.get_pipelines(&mr);
                (mr, pipelines)
            })
            .collect()
    }

    fn get_pipelines(&self, mr: &MergeRequest) -> Vec<PipelineBasic> {
        match &mr.sha {
            None => Vec::new(),
            Some(sha) => Pipelines::builder()
                .project(self.project.as_str())
                .sha(sha.value())
                .build()
                .unwrap()
                .query(&self.client)
                .unwrap(),
        }
    }

    fn get_branch_commit(&self, branch: &str) -> Vec<Commit> {
        let commit_request = projects::repository::commits::Commits::builder()
            .project(self.project.as_str())
            .ref_name(branch)
            .build()
            .unwrap();
        let commits: Vec<Commit> = commit_request.query(&self.client).unwrap();
        commits
    }

    fn is_rebased(&self, first_main_commits: &Commit, item: &(MergeRequest, Vec<PipelineBasic>)) -> bool {
        self.get_branch_commit(item.0.source_branch.as_str()).iter().any(|c| c.id == first_main_commits.id)
    }

    fn is_assignee_to_marge_bot(&self, mr: &MergeRequest) -> bool {
        match mr.assignees.as_ref() {
            None => false,
            Some(users) => users.iter().any(|user| user.name == self.name),
        }
    }
}
