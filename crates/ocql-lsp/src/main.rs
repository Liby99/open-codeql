use std::collections::HashMap;
use std::error::Error;

use log::info;
use lsp_server::{Connection, Message, Notification};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    PublishDiagnostics,
};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, InitializeResult, Position,
    PublishDiagnosticsParams, Range, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    env_logger::init();
    info!("ocql-lsp starting");

    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL,
        )),
        ..Default::default()
    };

    let init_result = InitializeResult {
        capabilities: server_capabilities,
        server_info: Some(ServerInfo {
            name: "ocql-lsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };

    let init_result_json = serde_json::to_value(init_result)?;
    connection.initialize(init_result_json)?;

    main_loop(connection)?;
    io_threads.join()?;

    info!("ocql-lsp shut down");
    Ok(())
}

fn main_loop(connection: Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut documents: HashMap<Uri, String> = HashMap::new();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                // We don't handle other requests yet
            }
            Message::Notification(notif) => {
                if notif.method == DidOpenTextDocument::METHOD {
                    let params: lsp_types::DidOpenTextDocumentParams =
                        serde_json::from_value(notif.params)?;
                    let uri = params.text_document.uri.clone();
                    let text = params.text_document.text.clone();
                    documents.insert(uri.clone(), text.clone());
                    publish_diagnostics(&connection, uri, &text)?;
                } else if notif.method == DidChangeTextDocument::METHOD {
                    let params: lsp_types::DidChangeTextDocumentParams =
                        serde_json::from_value(notif.params)?;
                    let uri = params.text_document.uri.clone();
                    // We use full sync, so the last content change is the full document
                    if let Some(change) = params.content_changes.into_iter().last() {
                        documents.insert(uri.clone(), change.text.clone());
                        publish_diagnostics(&connection, uri, &change.text)?;
                    }
                } else if notif.method == DidCloseTextDocument::METHOD {
                    let params: lsp_types::DidCloseTextDocumentParams =
                        serde_json::from_value(notif.params)?;
                    documents.remove(&params.text_document.uri);
                    // Clear diagnostics on close
                    let clear = PublishDiagnosticsParams {
                        uri: params.text_document.uri,
                        diagnostics: vec![],
                        version: None,
                    };
                    connection.sender.send(Message::Notification(Notification {
                        method: PublishDiagnostics::METHOD.to_string(),
                        params: serde_json::to_value(clear)?,
                    }))?;
                }
            }
            Message::Response(_) => {}
        }
    }

    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    text: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let diagnostics = diagnose(text);

    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };

    connection.sender.send(Message::Notification(Notification {
        method: PublishDiagnostics::METHOD.to_string(),
        params: serde_json::to_value(params)?,
    }))?;

    Ok(())
}

/// Parse the document and produce diagnostics from any errors.
fn diagnose(text: &str) -> Vec<Diagnostic> {
    match ocql_ql_parser::parse_source_file(text) {
        Ok(_) => vec![],
        Err(err) => {
            let (range, message) = parse_error_to_range_and_message(text, &err);
            vec![Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("ocql".to_string()),
                message,
                ..Default::default()
            }]
        }
    }
}

/// Convert a LALRPOP ParseError into a line/column range and human-readable message.
fn parse_error_to_range_and_message(
    text: &str,
    err: &ocql_ql_parser::ParseError,
) -> (Range, String) {
    match err {
        lalrpop_util::ParseError::UnrecognizedToken {
            token: (start, _tok, end),
            expected,
        } => {
            let start_pos = offset_to_position(text, *start);
            let end_pos = offset_to_position(text, *end);
            let expected_str = format_expected(expected);
            let message = format!("unexpected token; expected {expected_str}");
            (Range::new(start_pos, end_pos), message)
        }
        lalrpop_util::ParseError::UnrecognizedEof { location, expected } => {
            let pos = offset_to_position(text, *location);
            let expected_str = format_expected(expected);
            let message = format!("unexpected end of file; expected {expected_str}");
            (Range::new(pos, pos), message)
        }
        lalrpop_util::ParseError::InvalidToken { location } => {
            let pos = offset_to_position(text, *location);
            let message = "invalid token".to_string();
            (Range::new(pos, pos), message)
        }
        lalrpop_util::ParseError::ExtraToken {
            token: (start, _tok, end),
        } => {
            let start_pos = offset_to_position(text, *start);
            let end_pos = offset_to_position(text, *end);
            let message = "extra token".to_string();
            (Range::new(start_pos, end_pos), message)
        }
        lalrpop_util::ParseError::User { error } => {
            let pos = Position::new(0, 0);
            let message = format!("lexical error: {error}");
            (Range::new(pos, pos), message)
        }
    }
}

/// Convert a byte offset into a line/column Position.
fn offset_to_position(text: &str, offset: usize) -> Position {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32;
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = (offset - line_start) as u32;
    Position::new(line, character)
}

/// Format a list of expected tokens into a human-readable string.
fn format_expected(expected: &[String]) -> String {
    // Clean up LALRPOP's expected token names
    let cleaned: Vec<&str> = expected
        .iter()
        .map(|s| {
            let s = s.trim_matches('"');
            match s {
                "upper_ident" => "type name",
                "lower_ident" => "identifier",
                "int_lit" => "integer",
                "float_lit" => "number",
                "string_lit" => "string",
                other => other,
            }
        })
        .collect();

    match cleaned.len() {
        0 => "something".to_string(),
        1 => cleaned[0].to_string(),
        _ if cleaned.len() <= 5 => {
            let (head, tail) = cleaned.split_at(cleaned.len() - 1);
            format!("{} or {}", head.join(", "), tail[0])
        }
        _ => {
            // Too many options — show first few
            format!("{}, ...", cleaned[..4].join(", "))
        }
    }
}
