//! A Language Server Protocol implementation for `.swede` files.
//!
//! Thin wrapper over `swede-analysis`: publishes coded diagnostics on
//! open/change/save, and serves the document outline and go-to-definition.
//! Highlighting comes from the editor's bundled tree-sitter queries, not here.

use std::collections::HashMap;

use swede_analysis as analysis;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    documents: Mutex<HashMap<Url, String>>,
}

impl Backend {
    async fn analyze(&self, uri: Url, text: String) {
        self.documents.lock().await.insert(uri.clone(), text.clone());
        let diagnostics = analysis::diagnostics(&text);
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "swede-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "swede-lsp ready").await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.analyze(params.text_document.uri, params.text_document.text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync: the final change carries the entire document text.
        if let Some(change) = params.content_changes.into_iter().last() {
            self.analyze(params.text_document.uri, change.text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = self.documents.lock().await.get(&uri).cloned();
        if let Some(text) = text {
            self.analyze(uri, text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.lock().await.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> tower_lsp::jsonrpc::Result<Option<DocumentSymbolResponse>> {
        let text = self.documents.lock().await.get(&params.text_document.uri).cloned();
        Ok(text.map(|text| DocumentSymbolResponse::Nested(analysis::document_symbols(&text))))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> tower_lsp::jsonrpc::Result<Option<Vec<TextEdit>>> {
        let text = self.documents.lock().await.get(&params.text_document.uri).cloned();
        Ok(text.map(|text| analysis::formatting(&text)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let pos = params.text_document_position_params;
        let text = self.documents.lock().await.get(&pos.text_document.uri).cloned();
        Ok(text.and_then(|text| analysis::definition(&pos.text_document.uri, &text, pos.position)))
    }
}

#[tokio::main]
async fn main() {
    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Mutex::new(HashMap::new()),
    });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
}
