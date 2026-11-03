//! Translate question display fields and typed answers without changing native question identities.
use super::*;

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Answer { #[serde(default)] selected:Vec<usize>, #[serde(default)] text:String }

fn pending(app: &AppCtx,sid: &str,id: &str) -> Result<Value,String> {
    let request=app.chat().pending_permission(sid,id)?;
    if request["tool_name"]!="AskUserQuestion" || request["subtype"]=="elicitation" {return Err("translation:structure".into());}
    Ok(request)
}

fn fields(input: &Value) -> Result<Vec<String>,String> {
    let questions=input["questions"].as_array().filter(|q|!q.is_empty()).ok_or("translation:structure")?;
    let mut fields=Vec::new();
    for q in questions {
        for key in ["header","question"] {fields.push(q[key].as_str().ok_or("translation:structure")?.to_string());}
        if let Some(value)=q["placeholder"].as_str() {fields.push(value.into());}
        for option in q["options"].as_array().ok_or("translation:structure")? {
            fields.push(option["label"].as_str().ok_or("translation:structure")?.to_string());
            if let Some(value)=option["description"].as_str() {fields.push(value.into());}
        }
    }
    Ok(fields)
}

fn replace_fields(input: &Value,translated: Vec<String>) -> Result<Value,String> {
    let mut result=input.clone();let mut translated=translated.into_iter();
    for q in result["questions"].as_array_mut().ok_or("translation:structure")? {
        for key in ["header","question","placeholder"] {
            if q[key].is_string() {q[key]=json!(translated.next().ok_or("translation:structure")?);}
        }
        for option in q["options"].as_array_mut().ok_or("translation:structure")? {
            for key in ["label","description"] {
                if option[key].is_string() {option[key]=json!(translated.next().ok_or("translation:structure")?);}
            }
        }
    }
    if translated.next().is_some() {return Err("translation:structure".into());}
    Ok(result)
}

fn translate_fields(app: &AppCtx,cfg: &config::Config,source: &str,target: &str,fields: &[String],id: &str) -> Result<Vec<String>,String> {
    config::language(source,true)?;config::language(target,false)?;
    if fields.iter().map(|s|s.chars().count()).sum::<usize>()>LIMIT {return Err("translation:too_large".into());}
    if source==target {return Ok(fields.to_vec());}
    let hash=digest(&format!("question-fields-v1\0{}",serde_json::to_string(fields).unwrap()));
    let cached:Option<String>=app.db().conn.lock().unwrap().query_row(
        "SELECT result FROM translation_cache WHERE hash=?1 AND source=?2 AND target=?3 AND version=?4",
        params![hash,source,target,cfg.version],|r|r.get(0)).optional().map_err(db_error)?;
    if let Some(cached)=cached {return serde_json::from_str(&cached).map_err(|_|"translation:storage".into());}
    let indices:Vec<_>=fields.iter().enumerate().filter_map(|(i,s)|(!s.trim().is_empty()).then_some(i)).collect();
    let selected=indices.iter().map(|i|fields[*i].clone()).collect::<Vec<_>>();
    let mut result=fields.to_vec();
    if !selected.is_empty() {
        let protected=text::protect_fields(&selected);
        let (lines,_)=request_fragments(app,cfg,source,target,&protected,id,&hash,selected.iter().map(|s|s.chars().count()).sum())?;
        for (i,value) in indices.into_iter().zip(protected.restore_fields(&lines)?) {result[i]=value;}
    }
    app.db().conn.lock().unwrap().execute("INSERT OR REPLACE INTO translation_cache(hash,source,target,version,result) VALUES(?1,?2,?3,?4,?5)",
        params![hash,source,target,cfg.version,serde_json::to_string(&result).unwrap()]).map_err(db_error)?;
    Ok(result)
}

fn settings(app: &AppCtx,sid: &str) -> Result<(config::Config,config::Preferences,bool),String> {
    let conn=app.db().conn.lock().unwrap();session(&conn,sid)?;
    let cfg=config::read_config(&conn)?;let prefs=config::read_preferences(&conn,sid)?;
    let enabled=config::enabled_for(&conn,&cfg,sid)?;Ok((cfg,prefs,enabled))
}

/// `manual` is a translation the user asked for on one question, so it does not depend on automatic replies.
fn still_current(app: &AppCtx,sid: &str,cfg: &config::Config,prefs: &config::Preferences,outgoing: bool,manual: bool) -> Result<(),String> {
    let (current,next,enabled)=settings(app,sid)?;
    let same=if outgoing {next.outgoing && next.source==prefs.source && next.target==prefs.target}
        else {(manual || next.replies) && next.reply_source==prefs.reply_source && next.reply_target==prefs.reply_target};
    if !enabled || current.version!=cfg.version || current.generation!=cfg.generation || !same {return Err("translation:canceled".into());}
    Ok(())
}

pub(super) fn question(app: &AppCtx,sid: &str,id: &str,manual: bool) -> Result<Value,String> {
    let request=pending(app,sid,id)?;let (cfg,prefs,enabled)=settings(app,sid)?;
    if !enabled || !manual && !prefs.replies {return Err("translation:disabled".into());}
    let started=Instant::now();
    let translated=translate_fields(app,&cfg,&prefs.reply_source,&prefs.reply_target,&fields(&request["input"] )?,id)?;
    let input=replace_fields(&request["input"],translated)?;
    let _gate=GATE.lock().unwrap();still_current(app,sid,&cfg,&prefs,false,manual)?;
    if pending(app,sid,id)?!=request {return Err("translation:canceled".into());}
    Ok(json!({"input":input,"originalInput":request["input"],"version":cfg.version,"generation":cfg.generation,
        "source":prefs.reply_source,"target":prefs.reply_target,"durationMs":started.elapsed().as_millis() as u64}))
}

/// Rebuild answers from host-owned questions; only explicitly typed answers enter outgoing translation.
fn answered_input(input: &Value,answers: &[Answer]) -> Result<(Value,Vec<(String,String)>),String> {
    let questions=input["questions"].as_array().ok_or("translation:structure")?;
    if questions.is_empty() || questions.len()!=answers.len() {return Err("translation:structure".into());}
    let mut values=serde_json::Map::new();let mut typed=Vec::new();
    for (q,answer) in questions.iter().zip(answers) {
        let key=q["question"].as_str().ok_or("translation:structure")?;
        let options=q["options"].as_array().ok_or("translation:structure")?;
        let text=answer.text.trim();
        let value=if !text.is_empty() {
            if !answer.selected.is_empty() || !options.is_empty() && q["allowOther"]==false {return Err("translation:structure".into());}
            typed.push((key.to_string(),text.to_string()));text.to_string()
        } else if !answer.selected.is_empty() {
            if q["multiSelect"]!=true && answer.selected.len()!=1 {return Err("translation:structure".into());}
            let mut seen=HashSet::new();let mut labels=Vec::new();
            for i in &answer.selected {
                if !seen.insert(i) {return Err("translation:structure".into());}
                labels.push(options.get(*i).and_then(|o|o["label"].as_str()).ok_or("translation:structure")?);
            }
            labels.join(", ")
        } else if q["allowEmpty"]==true && (options.is_empty() || q["allowOther"]!=false) {String::new()}
        else {return Err("translation:structure".into());};
        if values.insert(key.to_string(),json!(value)).is_some() {return Err("translation:structure".into());}
    }
    let mut result=input.clone();result["answers"]=Value::Object(values);Ok((result,typed))
}

pub fn answer(app: &AppCtx,sid: &str,id: &str,answers: &Value,send_original: bool) -> Result<(),String> {
    let request=pending(app,sid,id)?;
    let answers:Vec<Answer>=serde_json::from_value(answers.clone()).map_err(|_|"translation:structure")?;
    let (mut input,typed)=answered_input(&request["input"],&answers)?;
    let (cfg,prefs,enabled)=settings(app,sid)?;
    let translating=enabled && prefs.outgoing && !send_original && !typed.is_empty();
    if translating {
        let originals=typed.iter().map(|(_,text)|text.clone()).collect::<Vec<_>>();
        let translated=translate_fields(app,&cfg,&prefs.source,&prefs.target,&originals,id)?;
        for ((key,_),text) in typed.into_iter().zip(translated) {input["answers"][key]=json!(text);}
    }
    let _gate=GATE.lock().unwrap();
    if translating {still_current(app,sid,&cfg,&prefs,true,false)?;}
    if pending(app,sid,id)?!=request {return Err("translation:canceled".into());}
    app.chat().respond_permission(app,sid,id,true,Some(input),None,None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_fields_keep_metadata_and_selected_labels_out_of_translation() {
        let input=json!({"metadata":{"source":"plan"},"questions":[
            {"header":"Plan","question":"First?","placeholder":"Other","options":[{"label":"A","description":"First","native":1},{"label":"B"}],"multiSelect":true},
            {"header":"Next","question":"Second?","options":[],"allowEmpty":true}
        ]});
        let original=fields(&input).unwrap();
        let display=replace_fields(&input,original.iter().map(|s|format!("translated {s}")).collect()).unwrap();
        assert_eq!(display["metadata"],input["metadata"]);assert_eq!(display["questions"][0]["options"][0]["native"],1);
        let answers:Vec<Answer>=serde_json::from_value(json!([{"selected":[1,0]},{"text":"  A typed answer  "}])).unwrap();
        let (answered,typed)=answered_input(&input,&answers).unwrap();
        assert_eq!(answered["questions"],input["questions"]);
        assert_eq!(answered["answers"],json!({"First?":"B, A","Second?":"A typed answer"}));
        assert_eq!(typed,vec![("Second?".to_string(),"A typed answer".to_string())]);
        for bad in [json!([]),json!([{"selected":[2]},{}]),json!([{"selected":[0,0]},{}]),json!([{"text":"custom","selected":[0]},{}])] {
            assert!(answered_input(&input,&serde_json::from_value::<Vec<Answer>>(bad).unwrap()).is_err());
        }
    }

    #[test]
    fn question_protection_keeps_plain_text_code_and_newline_boundaries() {
        let fields=vec!["Keep `request_id` and install.sh\nKeep **literal** characters (Recommended).".into(),"Keep /tmp/file, --zip and https://example.com/download".into()];
        let protected=text::protect_fields(&fields);
        assert!(protected.lines[0].contains("(Recommended)"));
        let lines=protected.lines.iter().map(|s|s.replace("Keep","保留")).collect::<Vec<_>>();
        assert_eq!(protected.restore_fields(&lines).unwrap(),fields.iter().map(|s|s.replace("Keep","保留")).collect::<Vec<_>>());
        assert!(protected.restore_fields(&lines[..1]).is_err());
        let mut corrupt=lines.clone();corrupt[0].push_str(&protected.tokens[0].0);
        assert!(protected.restore_fields(&corrupt).is_err());
    }
}
