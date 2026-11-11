//! Bounded public activity. Tool contents stay private; native TTS diagnostics
//! are the sole exception. Signatures and redacted reasoning are never retained.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Activity {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub text: String,
    pub status: String,
    pub tool: Option<String>,
    pub elapsed_seconds: Option<f64>,
    #[serde(default)]
    pub truncated: bool,
}
#[derive(Clone, Serialize)]
pub struct ChatRun {
    pub run_id: String,
    pub call_id: String,
    pub user_index: usize,
    pub status: String,
    pub entries: Vec<Activity>,
}
pub struct Tracker {
    pub run: ChatRun,
    message: String,
    blocks: HashMap<String, String>,
    text_size: usize,
    answer_size: usize,
    full_cursor: HashMap<String, u64>,
    completed_blocks: HashSet<String>,
}
impl Tracker {
    pub fn new(call_id: &str, user_index: usize) -> Self {
        let mut tracker = Self {
            run: ChatRun {
                run_id: uuid::Uuid::new_v4().to_string(),
                call_id: call_id.into(),
                user_index,
                status: "running".into(),
                entries: vec![],
            },
            message: "initial".into(),
            blocks: HashMap::new(),
            text_size: 0,
            answer_size: 0,
            full_cursor: HashMap::new(),
            completed_blocks: HashSet::new(),
        };
        tracker.upsert(
            "connection",
            "status",
            "Connessione a Claude",
            "running",
            None,
        );
        tracker
    }
    fn upsert(&mut self, id: &str, kind: &str, label: &str, status: &str, tool: Option<&str>) {
        if self.run.entries.iter().any(|e| e.id == id) {
            return;
        }
        if self.run.entries.len() >= 200 {
            return;
        }
        self.run.entries.push(Activity {
            id: id.into(),
            kind: kind.into(),
            label: label.chars().take(512).collect(),
            text: String::new(),
            status: status.into(),
            tool: tool.map(|s| s.chars().take(512).collect()),
            elapsed_seconds: None,
            truncated: false,
        });
    }
    fn status(&mut self, id: &str, status: &str) {
        if let Some(e) = self.run.entries.iter_mut().find(|e| e.id == id) {
            if !["complete", "denied", "cancelled", "error"].contains(&e.status.as_str()) {
                e.status = status.into();
            }
        }
    }
    fn append(&mut self, id: &str, text: &str, full: bool) {
        if let Some(e) = self.run.entries.iter_mut().find(|e| e.id == id) {
            // The full assistant message repeats the already streamed summary.
            if full && !e.text.is_empty() {
                return;
            }
            let is_text = e.kind == "text";
            let total = if is_text {
                &mut self.answer_size
            } else {
                &mut self.text_size
            };
            let per_block = if is_text { 524288 } else { 16384 };
            let budget = if is_text { 1048576 } else { 131072 };
            for ch in text.chars() {
                if e.text.len() + ch.len_utf8() > per_block || *total + ch.len_utf8() > budget {
                    e.truncated = true;
                    break;
                }
                e.text.push(ch);
                *total += ch.len_utf8();
            }
        }
    }
    fn block(&mut self, message: &str, index: u64, block: &Value, full: bool) {
        let key = format!("{message}:{index}");
        match block["type"].as_str() {
            Some("thinking") => {
                if full {
                    // CLI assistant frames can contain only one completed block, indexed at zero.
                    // Locate its already streamed block by message id and public summary.
                    let text = block["thinking"].as_str().unwrap_or_default();
                    if self.complete_public_block(&key, message, "reasoning", text) {
                        return;
                    }
                }
                self.upsert(
                    &key,
                    "reasoning",
                    "Ragionamento · riepilogo",
                    if full { "complete" } else { "running" },
                    None,
                );
                self.append(&key, block["thinking"].as_str().unwrap_or_default(), full);
                if full {
                    self.completed_blocks.insert(key.clone());
                }
                self.blocks.insert(key.clone(), key);
            }
            Some("tool_use") => {
                if let (Some(id), Some(tool)) = (block["id"].as_str(), block["name"].as_str()) {
                    self.upsert(id, "tool", tool, "preparing", Some(tool));
                    self.blocks.insert(key, id.into());
                }
            }
            Some("text") => {
                if full {
                    let text = block["text"].as_str().unwrap_or_default();
                    if self.complete_public_block(&key, message, "text", text) {
                        return;
                    }
                }
                self.upsert(
                    &key,
                    "text",
                    "Composizione della risposta",
                    if full { "complete" } else { "running" },
                    None,
                );
                self.append(&key, block["text"].as_str().unwrap_or_default(), full);
                if full {
                    self.completed_blocks.insert(key.clone());
                }
                self.blocks.insert(key.clone(), key);
            }
            // redacted_thinking and signature_delta are deliberately ignored.
            _ => {}
        }
    }
    fn complete_public_block(&mut self, key: &str, message: &str, kind: &str, text: &str) -> bool {
        let matches = |entry: &&Activity| {
            entry.kind == kind
                && entry.id.starts_with(&format!("{message}:"))
                && !self.completed_blocks.contains(&entry.id)
                && text.starts_with(&entry.text)
        };
        let entry = self
            .run
            .entries
            .iter()
            .filter(matches)
            .find(|entry| entry.id == key)
            .or_else(|| self.run.entries.iter().rev().filter(matches).next());
        let Some((id, seen)) = entry.map(|entry| (entry.id.clone(), entry.text.len())) else {
            return false;
        };
        self.append(&id, &text[seen..], false);
        self.status(&id, "complete");
        self.completed_blocks.insert(id);
        true
    }
    pub fn observe(&mut self, value: &Value) {
        match value["type"].as_str() {
            Some("stream_event") => {
                let event = &value["event"];
                let index = event["index"].as_u64().unwrap_or(0);
                let key = format!("{}:{index}", self.message);
                match event["type"].as_str() {
                    Some("message_start") => {
                        self.message = event["message"]["id"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                        self.status("connection", "complete");
                    }
                    Some("content_block_start") => {
                        self.block(&self.message.clone(), index, &event["content_block"], false)
                    }
                    Some("content_block_delta") if event["delta"]["type"] == "thinking_delta" => {
                        self.append(
                            &key,
                            event["delta"]["thinking"].as_str().unwrap_or_default(),
                            false,
                        )
                    }
                    Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                        self.append(
                            &key,
                            event["delta"]["text"].as_str().unwrap_or_default(),
                            false,
                        );
                    }
                    Some("content_block_stop") => {
                        if let Some(id) = self.blocks.get(&key).cloned() {
                            if self
                                .run
                                .entries
                                .iter()
                                .any(|e| e.id == id && e.kind != "tool")
                            {
                                self.status(&id, "complete");
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some("assistant") => {
                let message = &value["message"];
                let id = message["id"].as_str().unwrap_or(&self.message).to_owned();
                if let Some(blocks) = message["content"].as_array() {
                    // Native CLI emits one completed block per assistant frame.
                    let first = *self.full_cursor.get(&id).unwrap_or(&0);
                    self.full_cursor
                        .insert(id.clone(), first + blocks.len() as u64);
                    for (index, block) in blocks.iter().enumerate() {
                        self.block(&id, first + index as u64, block, true);
                    }
                }
            }
            Some("user") => {
                if let Some(blocks) = value["message"]["content"].as_array() {
                    for block in blocks {
                        if block["type"] == "tool_result" {
                            if let Some(id) = block["tool_use_id"].as_str() {
                                self.status(
                                    id,
                                    if block["is_error"].as_bool().unwrap_or(false) {
                                        "error"
                                    } else {
                                        "complete"
                                    },
                                );
                                let integrated_error = block["is_error"] == true && self.run.entries.iter()
                                    .any(|entry|entry.id==id && entry.tool.as_deref().is_some_and(|tool|crate::tts::is_tool(tool) || crate::web_tools::is_tool(tool)));
                                if integrated_error {
                                    if let Some(error)=crate::tts::public_error(&block["content"]) {
                                        self.append(id,&error,true);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some("tool_progress") => {
                if let Some(id) = value["tool_use_id"].as_str() {
                    let tool = value["tool_name"].as_str().unwrap_or("Strumento");
                    self.upsert(id, "tool", tool, "running", Some(tool));
                    self.status(id, "running");
                    if let Some(e) = self.run.entries.iter_mut().find(|e| e.id == id) {
                        e.elapsed_seconds = value["elapsed_time_seconds"]
                            .as_f64()
                            .filter(|v| v.is_finite() && *v >= 0.);
                    }
                }
            }
            Some("system") if value["subtype"] == "init" => self.status("connection", "complete"),
            Some("system") if value["subtype"] == "permission_denied" => {
                self.permission(value, "denied")
            }
            _ => {}
        }
    }
    pub fn observe_codex(&mut self, value: &Value) {
        let method=value["method"].as_str().unwrap_or_default(); let p=&value["params"];
        if method=="thread/started" { self.status("connection","complete"); }
        let item=&p["item"];
        let raw_id=p["itemId"].as_str().or_else(||item["id"].as_str()).unwrap_or_default();
        if raw_id.is_empty() { return; }
        let id=format!("codex:{raw_id}");
        match method {
            "item/agentMessage/delta" => {self.upsert(&id,"text","Composizione della risposta","running",None);self.append(&id,p["delta"].as_str().unwrap_or_default(),false);},
            "item/reasoning/summaryTextDelta" => {self.upsert(&id,"reasoning","Ragionamento · riepilogo","running",None);self.append(&id,p["delta"].as_str().unwrap_or_default(),false);},
            "item/started" | "item/completed" => {
                let complete=method=="item/completed";let status=if complete {"complete"}else{"running"};
                match item["type"].as_str() {
                    Some("agentMessage")=>{self.upsert(&id,"text","Composizione della risposta",status,None);if complete {self.append(&id,item["text"].as_str().unwrap_or_default(),true);}self.status(&id,status);},
                    Some("reasoning")=>{self.upsert(&id,"reasoning","Ragionamento · riepilogo",status,None);if complete {let text=item["summary"].as_array().map(|a|a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n\n")).unwrap_or_default();if !text.is_empty(){self.append(&id,&text,true);}}self.status(&id,status);},
                    Some("mcpToolCall")=>{let tool=format!("mcp__{}__{}",item["server"].as_str().unwrap_or(""),item["tool"].as_str().unwrap_or(""));self.upsert(&id,"tool",&tool,status,Some(&tool));let status=match item["status"].as_str(){Some("failed")=>"error",Some("declined")=>"denied",_=>status};self.status(&id,status);if let Some(e)=self.run.entries.iter_mut().find(|e|e.id==id){e.elapsed_seconds=item["durationMs"].as_f64().map(|v|v/1000.);}},
                    Some("webSearch")=>{self.upsert(&id,"tool","Ricerca web",status,Some("web_search"));self.status(&id,status);},
                    Some("commandExecution" | "fileChange" | "imageView" | "collabAgentToolCall")=>{
                        let (label,tool)=match item["type"].as_str().unwrap() {
                            "commandExecution"=>("Esecuzione comando","exec_command"),
                            "fileChange"=>("Modifica file","apply_patch"),
                            "imageView"=>("Lettura immagine","view_image"),
                            _=>("Agente","agent"),
                        };
                        self.upsert(&id,"tool",label,status,Some(tool));
                        let status=match item["status"].as_str(){Some("failed")=>"error",Some("declined")=>"denied",_=>status};
                        self.status(&id,status);
                    },
                    _=>{}
                }
            },_=>{}
        }
    }
    pub fn permission(&mut self, request: &Value, status: &str) {
        let tool = request["tool_name"].as_str().unwrap_or("Strumento");
        let id = request["tool_use_id"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| {
                self.run
                    .entries
                    .iter()
                    .rev()
                    .find(|e| {
                        e.tool.as_deref() == Some(tool)
                            && ["preparing", "waiting", "running"].contains(&e.status.as_str())
                    })
                    .map(|e| e.id.clone())
            })
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.upsert(&id, "tool", tool, status, Some(tool));
        self.status(&id, status);
    }
    pub fn observe_gemini(&mut self, update: &Value) {
        self.status("connection", "complete");
        if update["sessionUpdate"] == "agent_message_chunk" && update["content"]["type"] == "text" {
            self.upsert("gemini-answer", "text", "Risposta", "running", None);
            self.append("gemini-answer", update["content"]["text"].as_str().unwrap_or_default(), false);
            return;
        }
        let Some(id) = update["toolCallId"].as_str() else { return; };
        let status = match update["status"].as_str() { Some("completed") => "complete", Some("failed") => "error", Some("pending") => "preparing", _ => "running" };
        self.upsert(id, "tool", update["title"].as_str().unwrap_or("Strumento Antigravity"), status, Some("antigravity_tool"));
        self.status(id, status);
    }
    pub fn reconcile_answer(&mut self, answer: &str) {
        let texts: Vec<&str> = self
            .run
            .entries
            .iter()
            .filter(|e| e.kind == "text")
            .map(|e| e.text.as_str())
            .collect();
        if texts.last().is_some_and(|t| t.trim() == answer.trim())
            || ["", "\n", "\n\n"]
                .iter()
                .any(|sep| !texts.is_empty() && texts.join(sep).trim() == answer.trim())
        {
            return;
        }
        // Some CLI results contain a final response without corresponding stream blocks.
        // Keep it at the end rather than replacing an earlier tool preamble.
        self.upsert("final-answer", "text", "Risposta", "complete", None);
        self.append("final-answer", answer, true);
    }
    pub fn finish(&mut self, error: Option<&str>, cancelled: bool) {
        self.run.status = if cancelled {
            "cancelled"
        } else if error.is_some() {
            "error"
        } else {
            "complete"
        }
        .into();
        for entry in &mut self.run.entries {
            if ["preparing", "waiting", "running"].contains(&entry.status.as_str()) {
                // A successful run does not prove an unreported tool operation succeeded.
                entry.status = if cancelled {
                    "cancelled"
                } else if error.is_some() {
                    "error"
                } else if entry.kind == "tool" {
                    "unknown"
                } else {
                    "complete"
                }
                .into();
            }
        }
        self.upsert(
            "finish",
            "status",
            if cancelled {
                "Risposta interrotta"
            } else if error.is_some() {
                "Risposta non completata"
            } else {
                "Risposta completata"
            },
            &self.run.status.clone(),
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_voice_errors_are_visible_but_other_tool_contents_stay_private() {
        let mut tracker=Tracker::new("synthetic",0);
        tracker.observe(&serde_json::json!({"type":"assistant","message":{"id":"m","content":[
            {"type":"tool_use","id":"speech","name":"mcp__parlo_tts__generate_speech"},
            {"type":"tool_use","id":"read","name":"Read"}]}}));
        tracker.observe(&serde_json::json!({"type":"user","message":{"content":[
            {"type":"tool_result","tool_use_id":"speech","is_error":true,"content":[{"type":"text","text":"Piper: Strumenti voce mancanti."}]},
            {"type":"tool_result","tool_use_id":"read","is_error":true,"content":"PRIVATE FILE CONTENTS"}]}}));
        let speech=tracker.run.entries.iter().find(|entry|entry.id=="speech").unwrap();
        assert_eq!(speech.status,"error"); assert_eq!(speech.text,"Piper: Strumenti voce mancanti.");
        assert!(!serde_json::to_string(&tracker.run).unwrap().contains("PRIVATE FILE CONTENTS"));
    }
    #[test]
    fn codex_preserves_text_tool_text_order_and_only_public_reasoning() {
        let mut t=Tracker::new("call",0);
        for value in [
            serde_json::json!({"method":"item/agentMessage/delta","params":{"itemId":"first","delta":"Controllo il documento."}}),
            serde_json::json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":"first","text":"Controllo il documento."}}}),
            serde_json::json!({"method":"item/started","params":{"item":{"type":"mcpToolCall","id":"tool","server":"drive","tool":"read","status":"inProgress"}}}),
            serde_json::json!({"method":"item/completed","params":{"item":{"type":"mcpToolCall","id":"tool","server":"drive","tool":"read","status":"completed","result":{"secret":"not stored"}}}}),
            serde_json::json!({"method":"item/completed","params":{"item":{"type":"reasoning","id":"reason","summary":["Confronto con la call."],"content":["private reasoning"]}}}),
            serde_json::json!({"method":"item/agentMessage/delta","params":{"itemId":"second","delta":"Il documento conferma la data."}}),
            serde_json::json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":"second","text":"Il documento conferma la data."}}}),
        ] {t.observe_codex(&value);}
        let entries=&t.run.entries[1..];assert_eq!(entries.iter().map(|e|e.kind.as_str()).collect::<Vec<_>>(),vec!["text","tool","reasoning","text"]);
        assert_eq!(entries[0].text,"Controllo il documento.");assert_eq!(entries[1].status,"complete");assert_eq!(entries[2].text,"Confronto con la call.");assert!(!serde_json::to_string(&t.run).unwrap().contains("private reasoning"));assert!(!serde_json::to_string(&t.run).unwrap().contains("not stored"));
    }
    #[test]
    fn repeated_text_around_a_tool_remains_two_distinct_steps() {
        let mut t = Tracker::new("call", 0);
        for block in [
            serde_json::json!({"type":"text","text":"Controllo."}),
            serde_json::json!({"type":"tool_use","id":"tool","name":"mcp__test__check"}),
            serde_json::json!({"type":"text","text":"Controllo."}),
        ] {
            t.observe(
                &serde_json::json!({"type":"assistant","message":{"id":"m","content":[block]}}),
            );
        }
        assert_eq!(
            t.run
                .entries
                .iter()
                .filter(|entry| entry.kind == "text")
                .count(),
            2
        );
        assert_eq!(t.run.entries[2].kind, "tool");
    }
    use serde_json::json;
    #[test]
    fn tools_and_text_preserve_event_order_without_duplicate_completed_blocks() {
        let mut t = Tracker::new("call", 0);
        t.observe(
            &json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"m"}}}),
        );
        for (index, block) in [
            json!({"type":"tool_use","id":"one","name":"mcp__drive__read"}),
            json!({"type":"text","text":""}),
            json!({"type":"tool_use","id":"two","name":"mcp__drive__write"}),
            json!({"type":"text","text":""}),
        ]
        .iter()
        .enumerate()
        {
            t.observe(&json!({"type":"stream_event","event":{"type":"content_block_start","index":index,"content_block":block}}));
            let full = if block["type"] == "text" {
                let text = if index == 1 {
                    "Ho trovato il file. Ora preparo la bozza."
                } else {
                    "La bozza è pronta."
                };
                t.observe(&json!({"type":"stream_event","event":{"type":"content_block_delta","index":index,"delta":{"type":"text_delta","text":text}}}));
                json!({"type":"text","text":text})
            } else {
                block.clone()
            };
            t.observe(
                &json!({"type":"stream_event","event":{"type":"content_block_stop","index":index}}),
            );
            t.observe(&json!({"type":"assistant","message":{"id":"m","content":[full]}}));
        }
        t.reconcile_answer("La bozza è pronta.");
        let steps: Vec<_> = t
            .run
            .entries
            .iter()
            .filter(|e| e.kind == "tool" || e.kind == "text")
            .map(|e| (e.kind.as_str(), e.text.as_str()))
            .collect();
        assert_eq!(
            steps,
            vec![
                ("tool", ""),
                ("text", "Ho trovato il file. Ora preparo la bozza."),
                ("tool", ""),
                ("text", "La bozza è pronta.")
            ]
        );
        t.observe(&json!({"type":"tool_progress","tool_use_id":"one","tool_name":"mcp__drive__read","elapsed_time_seconds":5.}));
        assert_eq!(t.run.entries[1].id, "one");
        assert_eq!(t.run.entries[3].id, "two");
    }
    #[test]
    fn result_only_response_is_appended_after_tools_and_preamble() {
        let mut t = Tracker::new("call", 0);
        for block in [
            json!({"type":"text","text":"Cerco il documento."}),
            json!({"type":"tool_use","id":"read","name":"mcp__drive__read"}),
        ] {
            t.observe(&json!({"type":"assistant","message":{"id":"m","content":[block]}}));
        }
        t.reconcile_answer("Il documento dice questo.");
        assert_eq!(t.run.entries[1].text, "Cerco il documento.");
        assert_eq!(t.run.entries[2].kind, "tool");
        assert_eq!(t.run.entries[3].text, "Il documento dice questo.");
    }
    #[test]
    fn streamed_summary_is_deduplicated_and_tool_stops_only_on_result() {
        let mut t = Tracker::new("call", 2);
        for event in [
            json!({"type":"message_start","message":{"id":"m"}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Controllo le scadenze."}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SECRET"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tool1","name":"mcp__drive__search"}}),
            json!({"type":"content_block_stop","index":1}),
        ] {
            t.observe(&json!({"type":"stream_event","event":event}));
        }
        t.observe(&json!({"type":"assistant","message":{"id":"m","content":[{"type":"thinking","thinking":"Controllo le scadenze.","signature":"SECRET"},{"type":"tool_use","id":"tool1","name":"mcp__drive__search"},{"type":"redacted_thinking","data":"SECRET"}]}}));
        assert_eq!(
            t.run
                .entries
                .iter()
                .filter(|e| e.kind == "reasoning")
                .count(),
            1
        );
        assert_eq!(t.run.entries[1].text, "Controllo le scadenze.");
        assert_eq!(t.run.entries[2].status, "preparing");
        t.permission(
            &json!({"tool_name":"mcp__drive__search","tool_use_id":"tool1"}),
            "waiting",
        );
        t.observe(&json!({"type":"tool_progress","tool_use_id":"tool1","tool_name":"mcp__drive__search","elapsed_time_seconds":3.}));
        t.observe(&json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tool1","content":"PRIVATE RESULT"}]}}));
        assert_eq!(t.run.entries[2].status, "complete");
        let saved = serde_json::to_string(&t.run).unwrap();
        assert!(!saved.contains("SECRET") && !saved.contains("PRIVATE RESULT"));
        for event in [
            json!({"type":"content_block_start","index":3,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":3,"delta":{"type":"thinking_delta","thinking":"Valuto il risultato dello strumento."}}),
        ] {
            t.observe(&json!({"type":"stream_event","event":event}));
        }
        t.observe(&json!({"type":"assistant","message":{"id":"m","content":[{"type":"thinking","thinking":"Valuto il risultato dello strumento."}]}}));
        assert_eq!(
            t.run
                .entries
                .iter()
                .filter(|e| e.kind == "reasoning")
                .count(),
            2
        );
        assert_eq!(t.run.entries.last().unwrap().status, "complete");
    }
    #[test]
    fn reasoning_is_bounded_and_denial_is_not_overwritten() {
        let mut t = Tracker::new("call", 0);
        t.block(
            "m",
            0,
            &json!({"type":"thinking","thinking":"è".repeat(20000)}),
            true,
        );
        assert!(t.run.entries[1].text.len() <= 16384);
        t.permission(
            &json!({"tool_name":"mcp__drive__search","tool_use_id":"x"}),
            "denied",
        );
        t.observe(&json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"x","is_error":true}]}}));
        t.finish(None, true);
        assert_eq!(t.run.entries[2].status, "denied");
        assert_eq!(t.run.status, "cancelled");
    }
}
