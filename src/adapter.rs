use std::rc::Rc;
use std::sync::Arc;

use crate::vertex::{GitlabRepo, RepoFile, Vertex};
use gitlab::api::projects::repository::files::FileRawBuilder;
use gitlab::api::projects::repository::TreeBuilder;
use gitlab::api::raw;
use gitlab::types::Project;
use gitlab::{
    api::{
        paged,
        projects::{ProjectBuilder, ProjectsBuilder},
        Client, Paged, Query,
    },
    Gitlab, GitlabBuilder,
};
use gitlab::{ObjectType, RepoTreeObject};
use tokio::runtime::Runtime;
use trustfall::provider::{resolve_coercion_with, resolve_neighbors_with, BasicAdapter};
use trustfall_core::interpreter::Typename;
use trustfall_core::{
    interpreter::{
        Adapter, ContextIterator, ContextOutcomeIterator, ResolveEdgeInfo, ResolveInfo,
        VertexIterator,
    },
    ir::{EdgeParameters, FieldValue},
};

lazy_static! {
    // instantiate a global gitlab client
    static ref GITLAB_CLIENT: Gitlab = {
        let mut glb: GitlabBuilder = GitlabBuilder::new(
            std::env::var("GITLAB_HOST").unwrap(),
            std::env::var("GITLAB_API_TOKEN").unwrap(),
        );
        glb.cert_insecure();
        glb.build().expect("Failed to initialize the Gitlab Client, check your env vars")
    };
}

#[derive(Debug, Clone)]
pub struct GitlabAdapter {}
impl Default for GitlabAdapter {
    fn default() -> Self {
        Self::new()
    }
}
impl GitlabAdapter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_gitlab_repos(language: Option<String>) -> VertexIterator<'static, Vertex> {
        println!("Getting gitlab repos!");
        let mut pb = ProjectsBuilder::default();

        if let Some(lang) = language {
            let pb = pb.with_programming_language(lang);
        }

        let projects = pb.build().unwrap();

        let pjs: Vec<Project> = paged(projects, gitlab::api::Pagination::Limit(50))
            .query(&*GITLAB_CLIENT)
            .expect("Failed to get all projects");

        let mut vertices = Vec::new();
        for pj in pjs {
            vertices.push(Vertex::GitlabRepo(GitlabRepo {
                id: pj.id.to_string(),
                url: pj.http_url_to_repo,
                name: pj.name,
                description: pj.description.unwrap_or(String::new()),
                repo_files: Vec::new(),
            }));
        }
        Box::new(vertices.into_iter())
    }

    pub fn get_files_for_repo(
        id: String,
        ref_: Option<String>,
        path: Option<String>,
    ) -> VertexIterator<'static, Vertex> {
        let mut tb = TreeBuilder::default();
        tb.project(id.clone()).recursive(true);

        if let Some(p) = path {
            tb.path(p);
        };

        if let Some(r) = ref_.clone() {
            tb.ref_(r);
        };

        let tbe = tb.build().unwrap();

        let files: Result<Vec<RepoTreeObject>, _> =
            paged(tbe, gitlab::api::Pagination::Limit(50)).query(&*GITLAB_CLIENT);

        match files {
            Ok(f) => {
                let mut nodes: Vec<RepoFile> = Vec::new();

                for file in f {
                    let ref_ = ref_.clone();
                    match file.type_ {
                        ObjectType::Tree => continue,
                        ObjectType::Blob => {
                            let mut raw_fb = FileRawBuilder::default();
                            raw_fb.project(id.clone()).file_path(file.path.clone());

                            if let Some(r) = ref_.clone() {
                                raw_fb.ref_(r);
                            }

                            let fbe = raw_fb.build().unwrap();
                            let contents =    raw(fbe).query(&*GITLAB_CLIENT)
                            .expect("Failed to get raw file contents, does this file exit on the branch?");

                            let content = String::from_utf8(contents).unwrap();

                            nodes.push(RepoFile {
                                path: file.path,
                                content,
                            });
                        }
                    }
                }

                Box::new(nodes.into_iter().map(|n| Vertex::RepoFile(n.into())))
            }
            Err(f) => {
                println!("Failed to get files for repo: {:?}", f);
                let output: Vec<Vertex> = Vec::new();
                Box::new(output.into_iter().map(|_| {
                    Vertex::RepoFile(
                        RepoFile {
                            path: String::new(),
                            content: String::new(),
                        }
                        .into(),
                    )
                }))
            }
        }
    }
}

macro_rules! impl_item_property {
    ($contexts:ident, $attr:ident) => {
        Box::new($contexts.map(|ctx| {
            let value = ctx
                .active_vertex()
                .map(|t| {
                    if let Some(s) = t.as_gitlab_repo() {
                        s.$attr.clone()
                    } else if let Some(j) = vertex.as_repo_file() {
                        j.$attr.clone().into()
                    } else {
                        unreachable!()
                    }
                })
                .into();

            (ctx, value)
        }))
    };
}

macro_rules! impl_property {
    ($contexts:ident, $conversion:ident, $attr:ident) => {
        Box::new($contexts.map(|ctx| {
            let vertex = ctx
                .active_vertex()
                .map(|vertex| vertex.$conversion().unwrap());
            let value = vertex.map(|t| t.$attr.clone()).into();

            (ctx, value)
        }))
    };

    ($contexts:ident, $conversion:ident, $var:ident, $b:block) => {
        Box::new($contexts.map(|ctx| {
            let vertex = ctx
                .active_vertex()
                .map(|vertex| vertex.$conversion().unwrap());
            let value = vertex.map(|$var| $b).into();

            (ctx, value)
        }))
    };
}

impl BasicAdapter<'static> for GitlabAdapter {
    type Vertex = Vertex;

    fn resolve_starting_vertices(
        &self,
        edge_name: &str,
        parameters: &EdgeParameters,
    ) -> VertexIterator<'static, Self::Vertex> {
        let language = parameters
            .get("language")
            .map(|v| match v {
                FieldValue::String(s) => Some(s.clone()),
                FieldValue::Null => None,
                _ => unreachable!(),
            })
            .unwrap_or(None);

        match edge_name {
            "UserGitlabRepos" => GitlabAdapter::get_gitlab_repos(language),
            _ => unreachable!("unknown starting edge name: {}", edge_name),
        }
    }

    fn resolve_coercion(
        &self,
        contexts: ContextIterator<'static, Self::Vertex>,
        type_name: &str,
        coerce_to_type: &str,
    ) -> ContextOutcomeIterator<'static, Self::Vertex, bool> {
        match (type_name, coerce_to_type) {
            _ => unreachable!(),
        }
    }

    fn resolve_property(
        &self,
        contexts: ContextIterator<'static, Self::Vertex>,
        type_name: &str,
        property_name: &str,
    ) -> ContextOutcomeIterator<'static, Self::Vertex, FieldValue> {
        match (type_name, property_name) {
            (_, "__typename") => Box::new(contexts.map(|ctx| {
                let value = match ctx.active_vertex() {
                    Some(vertex) => vertex.typename().into(),
                    None => FieldValue::Null,
                };

                (ctx, value)
            })),

            ("GitlabRepo", "url") => impl_property!(contexts, as_gitlab_repo, url),
            ("GitlabRepo", "id") => impl_property!(contexts, as_gitlab_repo, id),
            ("GitlabRepo", "name") => impl_property!(contexts, as_gitlab_repo, name),
            ("GitlabRepo", "description") => impl_property!(contexts, as_gitlab_repo, description),
            ("RepoFile", "path") => impl_property!(contexts, as_repo_file, path),
            ("RepoFile", "content") => impl_property!(contexts, as_repo_file, content),

            _ => unreachable!(),
        }
    }

    fn resolve_neighbors(
        &self,
        contexts: ContextIterator<'static, Self::Vertex>,
        type_name: &str,
        edge_name: &str,
        parameters: &EdgeParameters,
    ) -> ContextOutcomeIterator<'static, Self::Vertex, VertexIterator<'static, Self::Vertex>> {
        print!("type_name: {}, edge_name: {}", type_name, edge_name);

        match (type_name, edge_name) {
            ("UserGitlabRepos", "repos") => {
                let language = parameters
                    .get("language")
                    .map(|v| match v {
                        FieldValue::String(s) => Some(s.clone()),
                        FieldValue::Null => None,
                        _ => unreachable!(),
                    })
                    .unwrap_or(None);

                let edge_resolver =
                    move |vertex: &Self::Vertex| -> VertexIterator<'static, Self::Vertex> {
                        let _repo = vertex.as_repo_list();
                        // here is where we would use information sitting on the UserGitlabRepos object
                        // to do edge resolution, in our case though we only care about params since it is the root node
                        Box::new(GitlabAdapter::get_gitlab_repos(language.clone()))
                    };

                resolve_neighbors_with(contexts, edge_resolver)
            }
            ("GitlabRepo", "files") => {
                let ref_ = parameters
                    .get("ref")
                    .map(|v| match v {
                        FieldValue::String(s) => Some(s.clone()),
                        FieldValue::Null => None,
                        _ => unreachable!(),
                    })
                    .unwrap_or(None);
                let path = parameters
                    .get("path")
                    .map(|v| match v {
                        FieldValue::String(s) => Some(s.clone()),
                        FieldValue::Null => None,
                        _ => unreachable!(),
                    })
                    .unwrap_or(None);

                let edge_resolver =
                    move |vertex: &Self::Vertex| -> VertexIterator<'static, Self::Vertex> {
                        match vertex.as_gitlab_repo() {
                            Some(repo) => {
                                let id = repo.id.clone();

                                GitlabAdapter::get_files_for_repo(id, ref_.clone(), path.clone())
                            }
                            _ => unreachable!(),
                        }
                    };

                resolve_neighbors_with(contexts, edge_resolver)
            }
            _ => unreachable!(),
        }
    }
}
