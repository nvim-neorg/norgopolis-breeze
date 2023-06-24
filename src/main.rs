use norgopolis_module::{
    invoker_service::Service, module_communication::MessagePack, Code, Module, Status,
};
use tokio_stream::wrappers::ReceiverStream;
use tree_sitter::{Language, Query, QueryCursor};
use std::collections::HashMap;

#[derive(serde::Deserialize)]
struct ParseQueryArguments {
    path: String,
    query: String,
    num_jobs: Option<usize>,
}

struct ParseQueryResult {
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
    type Stream = ReceiverStream<Result<MessagePack, Status>>;

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

                let files = ws.files();
                let (tx, rx) = tokio::sync::mpsc::channel(files.len());

                neorg_breeze::breeze::parse_files(
                    files,
                    self.language,
                    directory_args.num_jobs,
                    |ast| {
                        let query = match Query::new(self.language, &directory_args.query) {
                            Ok(value) => value,
                            Err(err) => return todo!(),
                        };


                        let cursor = QueryCursor::new();

                        tx.send(Ok(MessagePack::encode()));
                    },
                );

                Ok(ReceiverStream::new(rx))
            }
            _ => Err(Status::new(Code::NotFound, "Requested function not found!")),
        }
    }
}

#[tokio::main]
async fn main() {
    Module::start(Breeze::new(tree_sitter_norg::language()))
        .await
        .unwrap()
}
