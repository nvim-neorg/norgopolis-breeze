use norgopolis_module::{
    invoker_service::Service, module_communication::MessagePack, Code, Module, Status,
};
use std::{collections::HashMap, path::PathBuf};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tree_sitter::{Language, Query, QueryCursor};

#[derive(serde::Deserialize)]
struct ParseQueryArguments {
    path: String,
    query: String,
    num_jobs: Option<usize>,
}

#[derive(serde::Serialize)]
struct ParseQueryResult {
    file: String,
    captures: HashMap<String, Vec<String>>,
}

struct Breeze {
    language: Language,
}

impl Breeze {
    pub fn new(language: Language) -> Self {
        Breeze { language }
    }
}

#[norgopolis_module::async_trait]
impl Service for Breeze {
    type Stream = UnboundedReceiverStream<Result<MessagePack, Status>>;

    async fn call(
        &self,
        function: String,
        args: Option<MessagePack>,
    ) -> Result<Self::Stream, Status> {
        match function.as_str() {
            "parse-query" => {
                let directory_args: ParseQueryArguments = args
                    .unwrap()
                    .decode()
                    .map_err(|err| Status::new(Code::InvalidArgument, err.to_string()))?;

                let ws = neorg_dirman::workspace::Workspace {
                    name: "".into(),
                    path: directory_args.path.into(),
                };

                // Bounded channel involves async which in closures does not work very well.
                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

                let cloned_language = self.language;

                ts_breeze::breeze::parse_files(
                    ws.files(),
                    self.language,
                    directory_args.num_jobs,
                    move |ast: tree_sitter::Tree, (file, src): (PathBuf, String)| {
                        let query = match Query::new(cloned_language, &directory_args.query) {
                            Ok(value) => value,
                            Err(err) => {
                                tx.send(Err(Status::new(Code::InvalidArgument, err.message)))
                                    .unwrap();
                                return;
                            }
                        };

                        let mut cursor = QueryCursor::new();
                        let mut hashmap = HashMap::<String, Vec<String>>::new();

                        for (query_capture, ix) in
                            cursor.captures(&query, ast.root_node(), src.as_bytes())
                        {
                            query_capture.captures.iter().fold(
                                &mut hashmap,
                                |hashmap, &capture| {
                                    let range = capture.node.range();
                                    let content = src[range.start_byte..range.end_byte].to_string();

                                    hashmap
                                        .entry(query.capture_names()[ix].clone())
                                        .and_modify(|vec| vec.push(content))
                                        .or_default();
                                    hashmap
                                },
                            );
                        }

                        tx.send(Ok(MessagePack::encode(ParseQueryResult {
                            file: file.to_string_lossy().into(),
                            captures: hashmap,
                        })
                        .unwrap()))
                            .unwrap();
                    },
                );

                Ok(UnboundedReceiverStream::new(rx))
            }
            _ => Err(Status::new(Code::NotFound, "Requested function not found!")),
        }
    }
}

#[tokio::main]
async fn main() {
    Module::new().start(Breeze::new(tree_sitter_norg::language()))
        .await
        .unwrap()
}
