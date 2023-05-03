use std::rc::Rc;

use gitlab::types::Project;
use trustfall_core::interpreter::Typename;

#[derive(Debug, Clone)]
pub enum Vertex {
    // ...
    UserGitlabRepos(UserGitlabRepos),
    GitlabRepo(GitlabRepo),
    RepoFile(Rc<RepoFile>),
}

impl Typename for Vertex {
    fn typename(&self) -> &'static str {
        match self {
            Vertex::UserGitlabRepos(..) => "UserGitlabRepos",
            Vertex::GitlabRepo(..) => "GitlabRepo",
            Vertex::RepoFile(..) => "RepoFile",
        }
    }
}

impl Vertex {
    pub fn as_gitlab_repo(&self) -> Option<&GitlabRepo> {
        match self {
            Self::GitlabRepo(repo) => Some(repo),
            _ => None,
        }
    }

    pub fn as_repo_file(&self) -> Option<&RepoFile> {
        match self {
            Self::RepoFile(file) => Some(file),
            _ => None,
        }
    }

    pub fn as_repo_list(&self) -> Option<&UserGitlabRepos> {
        match self {
            Self::UserGitlabRepos(repos) => Some(repos),
            _ => None,
        }
    }
}

impl From<GitlabRepo> for Vertex {
    fn from(repo: GitlabRepo) -> Self {
        Self::GitlabRepo(repo)
    }
}

impl From<RepoFile> for Vertex {
    fn from(file: RepoFile) -> Self {
        Self::RepoFile(file.into())
    }
}

#[derive(Debug, Clone)]
pub struct UserGitlabRepos {
    pub repos: Vec<GitlabRepo>,
}

#[derive(Debug, Clone)]
pub struct GitlabRepo {
    pub id: String,
    pub url: String,
    pub repo_files: Vec<Rc<RepoFile>>,
}

#[derive(Debug, Clone)]
pub struct RepoFile {
    pub path: String,
    pub content: String,
}
