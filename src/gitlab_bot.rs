use gitlab::{api, Commit, Gitlab, MergeRequest, MergeStatus, PipelineBasic, StatusState};
use gitlab::api::{projects, Query};
use gitlab::api::projects::merge_requests::{MergeRequests, MergeRequestState};
use gitlab::api::projects::pipelines::Pipelines;

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
        self.reassing_failed_to_author();
        self.merge_all(self.get_mrs());
        self.rebase_first(self.get_mrs());
    }

    fn reassign_cannotbemerged_to_author(&self) {
        self
            .get_mrs()
            .into_iter()
            .filter(|mr| mr.merge_status == MergeStatus::CannotBeMerged)
            .for_each(|mr| {
                self.set_assignee(&mr);
                let message = format!("MR has merge conflict. Help me @{:}", mr.author.username);
                self.create_discussion_comment(mr, message);
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
                self.create_discussion_comment(mr, message);
            });
    }

    fn set_assignee(&self, mr: &MergeRequest) {
        let request = projects::merge_requests::EditMergeRequest::builder()
            .project(self.project.as_str())
            .merge_request(mr.iid.value())
            .assignee(mr.author.id.value())
            .build()
            .unwrap();


        match api::ignore(request).query(&self.client) {
            Ok(_) => {
                println!("reasing autor {:?}", mr.title)
            }
            Err(e) => {
                println!("fail reasing autor {:?} {:?}", mr.title, e)
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

    fn merge_all(&self, mrs: Vec<MergeRequest>) {
        mrs.into_iter().for_each(|mr| {
            self.merge(&mr);
        });
    }

    fn rebase_first(&self, mut mrs: Vec<MergeRequest>) {
        mrs.sort_by_key(|mr| mr.iid.value());
        mrs
            .iter()
            .filter(|mr| !mr.has_conflicts)
            .filter(|mr| !self.is_rebased(&mr))
            .take(2)
            .for_each(|mr| {
                self.rebase(mr);
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

    fn get_mrs(&self) -> Vec<MergeRequest> {
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
            .collect()
    }

    fn get_pipelines(&self, mr: &MergeRequest) -> Option<PipelineBasic> {
        let pp: Vec<PipelineBasic> = match &mr.sha {
            None => Vec::new(),
            Some(sha) => Pipelines::builder()
                .project(self.project.as_str())
                .sha(sha.value())
                .build()
                .unwrap()
                .query(&self.client)
                .unwrap(),
        };

        pp.into_iter().nth(0)
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

    fn is_rebased(&self, mr: &MergeRequest) -> bool {
        let target_commits = self.get_branch_commit(mr.target_branch.as_str());
        if target_commits.len() == 0 {
            false
        } else {
            let source_commits = self.get_branch_commit(mr.source_branch.as_str());
            source_commits.iter().any(|c| c.id == target_commits[0].id)
        }
    }

    fn is_assignee_to_marge_bot(&self, mr: &MergeRequest) -> bool {
        match mr.assignees.as_ref() {
            None => false,
            Some(users) => users.iter().any(|user| user.name == self.name),
        }
    }
}
