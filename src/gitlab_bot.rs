use gitlab::api::projects::merge_requests::{MergeRequestState, MergeRequests};
use gitlab::api::projects::pipelines::Pipelines;
use gitlab::api::{projects, Query};
use gitlab::{api, Gitlab, MergeRequest, PipelineBasic, Project, StatusState};

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
        self.merge_ready(self.get_mrs());
        self.rebase_first(self.get_mrs());
    }

    fn rebase_first(&self, mrs: Vec<(MergeRequest, Vec<PipelineBasic>)>) {
        mrs.iter().any(|item| {
            let pipelines = &item.1;
            if !pipelines.is_empty()
                && pipelines.iter().all(|p| {
                    p.status == StatusState::Success
                        || p.status == StatusState::Canceled
                        || p.status == StatusState::Manual
                })
            {
                self.rebase(&item.0)
            } else {
                false
            }
        });
    }

    fn merge_ready(&self, mrs: Vec<(MergeRequest, Vec<PipelineBasic>)>) {
        mrs.iter().for_each(|item| {
            let pipelines = &item.1;
            if !pipelines.is_empty()
                && pipelines.iter().all(|p| {
                    p.status == StatusState::Success
                        || p.status == StatusState::Canceled
                        || p.status == StatusState::Manual
                })
            {
                self.merge(&item.0);
            }
        });
    }

    fn is_assignee_to_marge_bot(mr: &MergeRequest) -> bool {
        match mr.assignees.as_ref() {
            None => false,
            Some(users) => users.iter().any(|user| user.name == "marge-bot"),
        }
    }
    fn rebase(&self, merge_request: &MergeRequest) -> bool {
        let merge_request_iid = merge_request.iid.value();
        let rebase = projects::merge_requests::RebaseMergeRequest::builder()
            .project(self.project.as_str())
            .merge_request(merge_request_iid)
            .build()
            .unwrap();
        match api::ignore(rebase).query(&self.client) {
            Ok(_) => {
                println!("rebase {:?}", merge_request.title);
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
        match api::raw(rebase).query(&self.client) {
            Ok(_) => {
                println!("merged {:?}", merge_request.title)
            }
            Err(e) => {
                println!("merge error {:?} {:?}", merge_request.title, e)
            }
        }
    }

    fn get_mrs(&self) -> Vec<(MergeRequest, Vec<PipelineBasic>)> {
        let mut mrs: Vec<MergeRequest> = MergeRequests::builder()
            .project(self.project.as_str())
            .state(MergeRequestState::Opened)
            .build()
            .unwrap()
            .query(&self.client)
            .unwrap();

        mrs.sort_by_key(|it| it.iid);

        mrs.into_iter()
            .filter(GitlabBot::is_assignee_to_marge_bot)
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
}
