use crate::vertex::{GitlabRepo, RepoFile, Vertex};
use chrono::{DateTime, Utc};
use gitlab::api::projects::repository::files::FileRawBuilder;
use gitlab::api::projects::repository::TreeBuilder;
use gitlab::api::raw;
use gitlab::types::Project;
use gitlab::{
    api::{paged, projects::ProjectsBuilder, Query},
    Gitlab, GitlabBuilder,
};
use gitlab::{ObjectType, RepoTreeObject};

use trustfall::provider::{resolve_neighbors_with, BasicAdapter};
use trustfall_core::interpreter::Typename;
use trustfall_core::{
    interpreter::{ContextIterator, ContextOutcomeIterator, VertexIterator},
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
pub struct GitlabAdapter {
    page_limit: usize,
}
impl Default for GitlabAdapter {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! extract_string_param {
    ($obj:expr, $param:expr) => {
        $obj.get($param)
            .map(|v| match v {
                FieldValue::String(s) => Some(s.clone()),
                FieldValue::Null => None,
                _ => unreachable!(),
            })
            .unwrap_or(None)
    };
}

macro_rules! extract_bool_param {
    ($obj:expr, $param:expr) => {
        $obj.get($param)
            .map(|v| match v {
                FieldValue::Boolean(s) => Some(s.clone()),
                FieldValue::Null => None,
                _ => unreachable!(),
            })
            .unwrap_or(None)
    };
}

macro_rules! extract_dt_param {
    ($obj:expr, $param:expr) => {
        $obj.get($param)
            .map(|v| match v {
                // note: this needs to be clone to solve lifetime issues arising
                // from the generic nature of FieldValue and the fact we need to parse
                FieldValue::DateTimeUtc(s) => Some(s.clone()),
                FieldValue::String(s) => Some(
                    DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap(),
                ),
                FieldValue::Null => None,
                _ => unreachable!(),
            })
            .unwrap_or(None)
    };
}

#[derive(Debug, Clone)]

pub struct GitlabProjectsGetParams {
    pub query_string: Option<String>,
    pub search_namespaces: Option<bool>,
    pub language: Option<String>,
    pub membership: Option<bool>,
    pub last_activity_after: Option<DateTime<Utc>>,
    pub last_activity_before: Option<DateTime<Utc>>,
}

impl From<&EdgeParameters> for GitlabProjectsGetParams {
    fn from(p: &EdgeParameters) -> Self {
        let query_string = extract_string_param!(p, "query");
        let search_namespaces = extract_bool_param!(p, "search_namespaces");

        let language = extract_string_param!(p, "language");
        let membership = extract_bool_param!(p, "membership");

        let last_activity_before = extract_dt_param!(p, "last_activity_before");
        let last_activity_after = extract_dt_param!(p, "last_activity_after");

        Self {
            query_string,
            search_namespaces,
            language,
            membership,
            last_activity_after,
            last_activity_before,
        }
    }
}

impl GitlabAdapter {
    pub fn new() -> Self {
        Self { page_limit: 20 }
    }

    /// Function to enscapsulate the logic of building a ProjectsBuilder, which is a bunch of optional fields,
    /// hence the `if let Some` statements
    pub fn build_projects_builder(params: GitlabProjectsGetParams) -> ProjectsBuilder<'static> {
        let mut pb = ProjectsBuilder::default();

        if let Some(query_string) = params.query_string {
            let pb = pb.search(query_string);
        }

        if let Some(search_namespaces) = params.search_namespaces {
            let pb = pb.search_namespaces(search_namespaces);
        }

        if let Some(lang) = params.language {
            let pb = pb.with_programming_language(lang);
        }

        if let Some(membership) = params.membership {
            let pb = pb.membership(membership);
        }

        if let Some(last_activity_after) = params.last_activity_after {
            let pb: &mut ProjectsBuilder = pb.last_activity_after(last_activity_after);
        }

        if let Some(last_activity_before) = params.last_activity_before {
            let pb = pb.last_activity_before(last_activity_before);
        }

        pb
    }

    pub fn get_gitlab_repos(
        &self,
        params: GitlabProjectsGetParams,
    ) -> VertexIterator<'static, Vertex> {
        println!("Getting gitlab repos w/ params: {:?}", &params);
        let pb = Self::build_projects_builder(params);

        let projects = pb.build().unwrap();

        let pjs: Vec<Project> = paged(projects, gitlab::api::Pagination::Limit(self.page_limit))
            .query(&*GITLAB_CLIENT)
            .expect("Failed to get all projects");

        let mut vertices = Vec::with_capacity(pjs.len());
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

                            let content = String::from_utf8_lossy(contents.as_slice());

                            nodes.push(RepoFile {
                                path: file.path,
                                content: content.to_string(),
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
        match edge_name {
            "GitlabRepos" => self.get_gitlab_repos(parameters.into()),
            _ => unreachable!("unknown starting edge name: {}", edge_name),
        }
    }

    /// #TODO: currently not needed in our schema, but may need to implement once we
    /// have edges that need to be joined
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
