use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

const DEVELOP_URI: &str = "content://com.rokid.aiui.develop";
const STAGING_DIR: &str = "/sdcard/aiui/package/.staging/adb/";
const PACKAGE_DIR: &str = "/sdcard/aiui/package";
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) struct InstallOptions<'a> {
    pub definition: Option<&'a Path>,
    pub serial: Option<&'a str>,
    pub optimize: bool,
    pub opt_level: u8,
    pub engine: Option<&'a str>,
}

pub(crate) struct PageLaunchOptions<'a> {
    pub path: Option<&'a str>,
    pub card: bool,
    pub params: Option<&'a str>,
    pub params_file: Option<&'a Path>,
    pub serial: Option<&'a str>,
}

pub(crate) struct WidgetLaunchOptions<'a> {
    pub path: &'a str,
    pub position: Option<usize>,
    pub params: Option<&'a str>,
    pub params_file: Option<&'a Path>,
    pub serial: Option<&'a str>,
}

pub(crate) struct ResolvedDefinition {
    pub input: PathBuf,
    pub is_project: bool,
    pub value: Value,
    pub app: Value,
}

pub(crate) fn resolve_definition(
    input: &Path,
    definition: Option<&Path>,
) -> Result<ResolvedDefinition> {
    let input = input
        .canonicalize()
        .with_context(|| format!("failed to resolve input {}", input.display()))?;
    let is_project = input.is_dir();
    if !is_project
        && (!input.is_file() || input.extension().and_then(|value| value.to_str()) != Some("aix"))
    {
        bail!(
            "input must be a project directory or .aix file: {}",
            input.display()
        );
    }
    let (app, generated_agent_id, agents_text) = read_launch_metadata(&input, is_project)?;
    let default_definition = is_project.then(|| input.join("agent.json"));
    let definition_path = definition
        .map(PathBuf::from)
        .or(default_definition.filter(|path| path.exists()));
    let mut value: Value = if let Some(path) = definition_path {
        serde_json::from_slice(&fs::read(path.canonicalize()?)?)?
    } else {
        build_definition(&app, &generated_agent_id, agents_text.as_deref())
    };
    value["agentId"] = Value::String(generated_agent_id);
    validate_definition(&value)?;
    Ok(ResolvedDefinition {
        input,
        is_project,
        value,
        app,
    })
}

pub(crate) fn launch_page(input: &Path, options: PageLaunchOptions<'_>) -> Result<()> {
    let resolved = resolve_definition(input, None)?;
    let agent_id = validate_definition(&resolved.value)?.to_owned();
    let (path, _) = resolve_open_route(&resolved.app, options.path, Some("blank"))?;
    let params = parse_open_params(options.params, options.params_file)?;
    let serial = select_device(options.serial)?;
    open_request(
        &serial,
        &agent_id,
        &path,
        if options.card { "_current" } else { "_blank" },
        params,
    )?;
    Ok(())
}

fn open_request(
    serial: &str,
    agent_id: &str,
    path: &str,
    target: &str,
    params: Value,
) -> Result<Value> {
    let request = serde_json::json!({
        "agentId": agent_id,
        "path": path,
        "target": target,
        "params": params,
    });
    let result = perform_open_request(serial, &request)?;
    finish_open_request(serial, &request, &result)?;
    Ok(result)
}

fn perform_open_request(serial: &str, request: &Value) -> Result<Value> {
    let request_json = serde_json::to_string(request)?;
    parse_develop_result(&run_open_content_call(serial, &request_json)?)
}

fn finish_open_request(serial: &str, request: &Value, result: &Value) -> Result<()> {
    require_ok(result, "open")?;
    let target = match request.get("target").and_then(Value::as_str) {
        Some("_widget") => "Widget",
        Some("_current") => "Card",
        _ => "Page",
    };
    println!("✔ {target} launch requested");
    println!(
        "  Agent: {}",
        display_value(request.get("agentId"), "unknown")
    );
    println!("  Path: {}", display_value(request.get("path"), "unknown"));
    println!("  Device: {serial}");
    println!("  Display: request accepted; rendering is handled asynchronously");
    Ok(())
}

pub(crate) fn device_status(serial: Option<&str>) -> Result<()> {
    let serial = select_device(serial)?;
    let snapshot = read_widget_snapshot(&serial)?;
    print_device_status(&serial, &snapshot)?;
    Ok(())
}

fn print_device_status(serial: &str, snapshot: &Value) -> Result<()> {
    let environment = snapshot
        .get("environment")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let developer_mode = match environment {
        "dev" => "enabled",
        "prod" => "disabled",
        _ => "unknown",
    };
    let runtime = if snapshot.get("ready").and_then(Value::as_bool) == Some(true) {
        "ready"
    } else {
        "not ready"
    };
    let epoch = display_value(snapshot.get("environmentEpoch"), "unknown");
    let version = display_value(snapshot.get("configurationVersion"), "none");
    println!("Device: {serial}");
    println!("Developer Mode: {developer_mode}");
    println!("Widget Runtime: {runtime}");
    println!("Environment Epoch: {epoch}");
    println!("Configuration Version: {version}");

    let Some(configuration) = widget_configuration(snapshot)? else {
        println!("Widget Layout: not configured");
        print_installed_agents(&read_installed_agents(serial)?);
        return Ok(());
    };
    println!(
        "Widget Layout: {} columns, {} cells",
        display_value(configuration.get("totalColumnCount"), "?"),
        display_value(configuration.get("totalGridCount"), "?")
    );
    print_widget_group("Permanent Widgets", configuration.get("permanentModules"));
    print_widget_group(
        "Dynamic Widget Placements",
        configuration.get("overlayModules"),
    );
    print_installed_agents(&read_installed_agents(serial)?);
    Ok(())
}

struct InstalledAgent {
    file_name: String,
    file_bytes: u64,
    metadata: Value,
}

fn read_installed_agents(serial: &str) -> Result<Vec<InstalledAgent>> {
    let files_command = format!(
        "find {PACKAGE_DIR} -maxdepth 1 -type f -name '*.aix' -exec stat -c '%n|%s' '{{}}' ';'"
    );
    let output = run_adb(serial, &["shell", &files_command])?;
    let mut files: Vec<_> = output
        .lines()
        .filter_map(|line| {
            let (file, bytes) = line.rsplit_once('|')?;
            let file_name = Path::new(file).file_name()?.to_str()?.to_owned();
            let file_bytes = bytes.parse().ok()?;
            file_name
                .ends_with(".aix")
                .then_some((file_name, file_bytes))
        })
        .collect();
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let manifest_command = format!(
        "if [ -f {PACKAGE_DIR}/developer_manifest.json ]; then cat {PACKAGE_DIR}/developer_manifest.json; fi"
    );
    let manifest_output = run_adb(serial, &["shell", &manifest_command])?;
    let manifest = if manifest_output.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&manifest_output)
            .context("developer_manifest.json is not valid JSON")?
    };
    let packages = manifest
        .get("packages")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    Ok(files
        .into_iter()
        .map(|(file_name, file_bytes)| InstalledAgent {
            metadata: packages
                .iter()
                .find(|package| package.get("fileName").and_then(Value::as_str) == Some(&file_name))
                .cloned()
                .unwrap_or(Value::Null),
            file_name,
            file_bytes,
        })
        .collect())
}

fn print_installed_agents(agents: &[InstalledAgent]) {
    println!("Installed Agents ({}):", agents.len());
    if agents.is_empty() {
        println!("  none");
        return;
    }
    for agent in agents {
        let fallback_name = agent
            .file_name
            .strip_suffix(".aix")
            .unwrap_or(&agent.file_name);
        println!(
            "  {}",
            display_value(agent.metadata.get("agentName"), fallback_name)
        );
        println!(
            "    ID: {}",
            display_value(agent.metadata.get("agentId"), "unknown")
        );
        println!(
            "    Package: {} ({})",
            agent.file_name,
            format_file_size(agent.file_bytes)
        );
        let native = display_value(agent.metadata.get("nativeVersion"), "");
        let ink = display_value(agent.metadata.get("inkVersion"), "");
        let mut versions = Vec::new();
        if !native.is_empty() {
            versions.push(format!("native {native}"));
        }
        if !ink.is_empty() {
            versions.push(format!("Ink {ink}"));
        }
        if !versions.is_empty() {
            println!("    Runtime: {}", versions.join(", "));
        }
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn display_value(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::String(value)) if !value.is_empty() => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        _ => fallback.to_owned(),
    }
}

fn print_widget_group(title: &str, modules: Option<&Value>) {
    let modules = modules
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    println!("{title} ({}):", modules.len());
    if modules.is_empty() {
        println!("  none");
        return;
    }
    for module in modules {
        println!(
            "  {}: {} -> {}",
            display_value(module.get("name"), "unnamed"),
            display_value(module.get("agentId"), "unknown agent"),
            display_value(module.get("path"), "unknown path")
        );
        println!(
            "    position {}, size {}x{}",
            display_value(module.get("startIndex"), "?"),
            display_value(module.get("columnSize"), "?"),
            display_value(module.get("rowSize"), "?")
        );
    }
}

pub(crate) fn set_developer_mode(serial: Option<&str>, enabled: bool) -> Result<()> {
    let serial = select_device(serial)?;
    let mode = if enabled { "dev" } else { "prod" };
    let result = parse_develop_result(&run_adb(&serial, &content_call("switch", Some(mode)))?)?;
    require_ok(&result, "switch")?;
    println!(
        "✔ Developer Mode {}",
        if enabled { "enabled" } else { "disabled" }
    );
    println!("  Device: {serial}");
    println!("  Widgets: reloaded; dynamic Widgets must be launched again");
    Ok(())
}

fn read_widget_snapshot(serial: &str) -> Result<Value> {
    let result = parse_develop_result(&run_adb(serial, &content_call("widget-snapshot", None))?)?;
    require_ok(&result, "widget-snapshot")?;
    Ok(result)
}

fn widget_configuration(snapshot: &Value) -> Result<Option<Value>> {
    let Some(raw) = snapshot.get("configurationJson") else {
        return Ok(None);
    };
    if raw.is_null() || raw.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    let configuration = if let Some(raw) = raw.as_str() {
        serde_json::from_str(raw).context("configurationJson is not valid JSON")?
    } else {
        raw.clone()
    };
    if !configuration.is_object() {
        bail!("configurationJson must contain a JSON object");
    }
    Ok(Some(configuration))
}

fn empty_widget_configuration() -> Value {
    serde_json::json!({
        "totalGridCount": 4,
        "totalColumnCount": 2,
        "moduleBorderStyle": 0,
        "permanentModules": [],
        "overlayModules": [],
    })
}

pub(crate) fn show_widget_layout(serial: Option<&str>) -> Result<()> {
    let serial = select_device(serial)?;
    let snapshot = read_widget_snapshot(&serial)?;
    let configuration = widget_configuration(&snapshot)?;
    println!("Widget Layout");
    println!("  Device: {serial}");
    println!(
        "  Environment: {}",
        display_value(snapshot.get("environment"), "unknown")
    );
    println!(
        "  Runtime: {}",
        if snapshot.get("ready").and_then(Value::as_bool) == Some(true) {
            "ready"
        } else {
            "not ready"
        }
    );
    println!(
        "  Configuration: {}",
        display_value(snapshot.get("configurationVersion"), "none")
    );
    if let Some(configuration) = configuration {
        println!(
            "  Grid: {} columns, {} cells",
            display_value(configuration.get("totalColumnCount"), "?"),
            display_value(configuration.get("totalGridCount"), "?")
        );
        print_widget_group("Permanent Widgets", configuration.get("permanentModules"));
        print_widget_group(
            "Dynamic Widget Placements",
            configuration.get("overlayModules"),
        );
    } else {
        println!("  Layout: not configured");
    }
    Ok(())
}

pub(crate) fn clear_widget_layout(serial: Option<&str>) -> Result<()> {
    let serial = select_device(serial)?;
    let snapshot = read_widget_snapshot(&serial)?;
    let Some(mut configuration) = widget_configuration(&snapshot)? else {
        println!("✔ Widget layout is already empty");
        println!("  Device: {serial}");
        return Ok(());
    };
    let permanent_empty = configuration
        .get("permanentModules")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty);
    let overlay_empty = configuration
        .get("overlayModules")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty);
    if permanent_empty && overlay_empty {
        println!("✔ Widget layout is already empty");
        println!("  Device: {serial}");
        return Ok(());
    }
    configuration["permanentModules"] = serde_json::json!([]);
    configuration["overlayModules"] = serde_json::json!([]);
    let result = apply_widget_configuration(&serial, &configuration)?;
    println!("✔ Widget layout cleared");
    println!("  Device: {serial}");
    println!(
        "  Configuration: {}",
        display_value(result.get("configurationVersion"), "updated")
    );
    Ok(())
}

fn apply_widget_configuration(serial: &str, configuration: &Value) -> Result<Value> {
    let inbox = prepare_inbox(serial)?;
    let temporary = TemporaryDirectory::new()?;
    let local = temporary.path.join("widget-config.json");
    let bytes = serde_json::to_vec_pretty(configuration)?;
    if bytes.len() > 64 * 1024 {
        bail!("generated widget-config.json is larger than 64 KiB");
    }
    fs::write(&local, bytes)?;
    let remote = format!("{}/widget-config.json", inbox.trim_end_matches('/'));
    run_adb(
        serial,
        &[
            "push",
            local.to_str().context("temporary path is not UTF-8")?,
            &remote,
        ],
    )?;
    let result = parse_develop_result(&run_adb(serial, &content_call("widget-apply", None))?)?;
    require_ok(&result, "widget-apply")?;
    Ok(result)
}

pub(crate) fn launch_widget(input: &Path, options: WidgetLaunchOptions<'_>) -> Result<()> {
    let resolved = resolve_definition(input, None)?;
    let agent_id = validate_definition(&resolved.value)?.to_owned();
    let (columns, rows) = widget_family_size(&resolved.app, options.path)?;
    let params = parse_open_params(options.params, options.params_file)?;
    let serial = select_device(options.serial)?;
    let snapshot = read_widget_snapshot(&serial)?;
    if snapshot.get("environment").and_then(Value::as_str) != Some("dev") {
        bail!("launch-widget requires Developer Mode; run `aix device set-dev`");
    }
    let mut configuration =
        widget_configuration(&snapshot)?.unwrap_or_else(empty_widget_configuration);
    let changed = upsert_overlay_widget(
        &mut configuration,
        &agent_id,
        options.path,
        columns,
        rows,
        options.position,
    )?;
    let mut layout_result = if changed {
        Some(apply_widget_configuration(&serial, &configuration)?)
    } else {
        None
    };
    if let Some(result) = &layout_result {
        print_widget_layout_applied(result);
    }
    let request = serde_json::json!({
        "agentId": agent_id,
        "path": options.path,
        "target": "_widget",
        "params": params,
    });
    let mut open_result = perform_open_request(&serial, &request)?;
    if !changed
        && open_result.get("errorCode").and_then(Value::as_str) == Some("WIDGET_PLACEMENT_MISSING")
    {
        layout_result = Some(apply_widget_configuration(&serial, &configuration)?);
        if let Some(result) = &layout_result {
            print_widget_layout_applied(result);
        }
        open_result = perform_open_request(&serial, &request)?;
    }
    finish_open_request(&serial, &request, &open_result)?;
    Ok(())
}

fn print_widget_layout_applied(result: &Value) {
    println!("✔ Widget layout applied");
    println!(
        "  Configuration: {}",
        display_value(result.get("configurationVersion"), "updated")
    );
    println!(
        "  Environment: {}",
        display_value(result.get("environment"), "unknown")
    );
    println!("  Note: all Widgets were reloaded; other dynamic Widgets must be launched again");
}

fn widget_family_size(app: &Value, path: &str) -> Result<(usize, usize)> {
    let family = app
        .get("widgets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|widget| widget.get("path").and_then(Value::as_str) == Some(path))
        .and_then(|widget| widget.get("family").and_then(Value::as_str))
        .with_context(|| format!("Widget path or family is not declared in app.json: {path}"))?;
    let (rows, columns) = family
        .split_once('x')
        .context("Widget family must use the <rows>x<columns> format")?;
    let rows: usize = rows.parse().context("Widget family row count is invalid")?;
    let columns: usize = columns
        .parse()
        .context("Widget family column count is invalid")?;
    if rows == 0 || columns == 0 {
        bail!("Widget family dimensions must be greater than zero");
    }
    Ok((columns, rows))
}

fn module_cells(module: &Value, total_columns: usize) -> Result<Vec<usize>> {
    let start = module
        .get("startIndex")
        .and_then(Value::as_u64)
        .context("Widget module startIndex is missing")? as usize;
    let columns = module
        .get("columnSize")
        .and_then(Value::as_u64)
        .context("Widget module columnSize is missing")? as usize;
    let rows = module
        .get("rowSize")
        .and_then(Value::as_u64)
        .context("Widget module rowSize is missing")? as usize;
    let start_column = start % total_columns;
    if columns == 0 || rows == 0 || start_column + columns > total_columns {
        bail!("Widget module does not fit the configured grid");
    }
    Ok((0..rows)
        .flat_map(|row| (0..columns).map(move |column| start + row * total_columns + column))
        .collect())
}

fn upsert_overlay_widget(
    configuration: &mut Value,
    agent_id: &str,
    path: &str,
    columns: usize,
    rows: usize,
    requested_position: Option<usize>,
) -> Result<bool> {
    let total_cells = configuration
        .get("totalGridCount")
        .and_then(Value::as_u64)
        .unwrap_or(4) as usize;
    let total_columns = configuration
        .get("totalColumnCount")
        .and_then(Value::as_u64)
        .unwrap_or(2) as usize;
    if total_cells == 0 || total_columns == 0 {
        bail!("Widget grid dimensions must be greater than zero");
    }
    for module in configuration
        .get("permanentModules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if module.get("agentId").and_then(Value::as_str) == Some(agent_id)
            && module.get("path").and_then(Value::as_str) == Some(path)
        {
            bail!("Widget is already configured as permanent and cannot be launched dynamically");
        }
    }
    let original = configuration.clone();
    let permanent = configuration
        .get("permanentModules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let overlays = configuration
        .get_mut("overlayModules")
        .and_then(Value::as_array_mut)
        .context("overlayModules must be an array")?;
    let existing = overlays.iter().find(|module| {
        module.get("agentId").and_then(Value::as_str) == Some(agent_id)
            && module.get("path").and_then(Value::as_str) == Some(path)
    });
    let existing_position = existing
        .and_then(|module| module.get("startIndex").and_then(Value::as_u64))
        .map(|value| value as usize);
    let existing_name = existing
        .and_then(|module| module.get("name").and_then(Value::as_str))
        .filter(|name| is_custom_module_name(name))
        .map(str::to_owned);
    overlays.retain(|module| {
        !(module.get("agentId").and_then(Value::as_str) == Some(agent_id)
            && module.get("path").and_then(Value::as_str) == Some(path))
    });
    let fits = |position: usize, modules: &[Value]| -> Result<bool> {
        let candidate =
            serde_json::json!({"startIndex": position, "columnSize": columns, "rowSize": rows});
        let cells = module_cells(&candidate, total_columns)?;
        if cells.iter().any(|cell| *cell >= total_cells) {
            return Ok(false);
        }
        for module in modules.iter().chain(permanent.iter()) {
            if module_cells(module, total_columns)?
                .iter()
                .any(|cell| cells.contains(cell))
            {
                return Ok(false);
            }
        }
        Ok(true)
    };
    let preferred = requested_position.or(existing_position);
    let position = if let Some(position) = preferred {
        position
    } else {
        match (0..total_cells).find(|position| fits(*position, overlays).unwrap_or(false)) {
            Some(position) => position,
            None => {
                overlays.clear();
                (0..total_cells)
                    .find(|position| fits(*position, overlays).unwrap_or(false))
                    .context(
                        "no position can fit this Widget without removing a permanent Widget",
                    )?
            }
        }
    };
    let candidate =
        serde_json::json!({"startIndex": position, "columnSize": columns, "rowSize": rows});
    let cells = module_cells(&candidate, total_columns)?;
    if cells.iter().any(|cell| *cell >= total_cells) {
        bail!("requested Widget position does not fit the configured grid");
    }
    if permanent.iter().any(|module| {
        module_cells(module, total_columns)
            .map(|occupied| occupied.iter().any(|cell| cells.contains(cell)))
            .unwrap_or(true)
    }) {
        bail!("requested Widget position conflicts with a permanent Widget");
    }
    let overlay_conflict = overlays.iter().any(|module| {
        module_cells(module, total_columns)
            .map(|occupied| occupied.iter().any(|cell| cells.contains(cell)))
            .unwrap_or(true)
    });
    if overlay_conflict {
        overlays.retain(|module| {
            module_cells(module, total_columns)
                .map(|occupied| !occupied.iter().any(|cell| cells.contains(cell)))
                .unwrap_or(false)
        });
    }
    let module_name = existing_name
        .filter(|name| {
            !permanent
                .iter()
                .chain(overlays.iter())
                .any(|module| module.get("name").and_then(Value::as_str) == Some(name.as_str()))
        })
        .unwrap_or_else(|| {
            (1..)
                .map(|index| format!("custom_{index}"))
                .find(|candidate| {
                    !permanent.iter().chain(overlays.iter()).any(|module| {
                        module.get("name").and_then(Value::as_str) == Some(candidate.as_str())
                    })
                })
                .expect("custom Widget module names are unbounded")
        });
    overlays.push(serde_json::json!({
        "name": module_name,
        "startIndex": position,
        "columnSize": columns,
        "rowSize": rows,
        "agentId": agent_id,
        "path": path,
    }));
    Ok(*configuration != original)
}

fn is_custom_module_name(name: &str) -> bool {
    name.strip_prefix("custom_").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
    })
}

fn resolve_open_route(
    app: &Value,
    requested_path: Option<&str>,
    requested_target: Option<&str>,
) -> Result<(String, &'static str)> {
    let pages: Vec<&str> = app
        .get("pages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let widgets: Vec<&str> = app
        .get("widgets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|widget| widget.get("path").and_then(Value::as_str))
        .collect();
    let path = requested_path
        .or_else(|| pages.first().copied())
        .context("no launch path was provided and app.json declares no pages")?;
    let is_page = pages.contains(&path);
    let is_widget = widgets.contains(&path);
    if !is_page && !is_widget {
        bail!("launch path is not declared in app.json pages or widgets: {path}");
    }
    let target = requested_target.unwrap_or(if is_widget { "widget" } else { "blank" });
    let protocol_target = match target.trim_start_matches('_') {
        "blank" if is_page => "_blank",
        "current" if is_page => "_current",
        "widget" if is_widget => "_widget",
        "widget" => bail!("target widget requires a path declared in app.json widgets"),
        "blank" | "current" => bail!("target {target} requires a path declared in app.json pages"),
        _ => bail!("target must be blank, current, or widget"),
    };
    Ok((path.to_owned(), protocol_target))
}

fn parse_open_params(inline: Option<&str>, file: Option<&Path>) -> Result<Value> {
    let value = match (inline, file) {
        (Some(_), Some(_)) => bail!("--params cannot be used with --params-file"),
        (Some(json), None) => serde_json::from_str(json).context("--params must be valid JSON")?,
        (None, Some(path)) => serde_json::from_slice(&fs::read(path)?)
            .with_context(|| format!("{} must contain valid JSON", path.display()))?,
        (None, None) => serde_json::json!({}),
    };
    if !value.is_object() {
        bail!("launch params must be a JSON object");
    }
    Ok(value)
}

pub(crate) fn install_agent(input: &Path, options: InstallOptions<'_>) -> Result<()> {
    let resolved = resolve_definition(input, options.definition)?;
    let definition = resolved.value;
    let agent_id = validate_definition(&definition)?.to_owned();
    let serial = select_device(options.serial)?;
    let temporary = TemporaryDirectory::new()?;
    let package_path = temporary.path.join("package.aix");
    let staged_definition = temporary.path.join("agent.json");
    fs::write(&staged_definition, serde_json::to_vec_pretty(&definition)?)?;

    if resolved.is_project {
        super::pack_directory(
            &resolved.input,
            &package_path,
            options.optimize,
            options.opt_level,
            options.engine,
        )?;
    } else {
        fs::copy(&resolved.input, &package_path)?;
    }
    let package_bytes = fs::metadata(&package_path)?.len();
    if package_bytes > MAX_PACKAGE_BYTES {
        bail!("package is larger than 128 MiB ({package_bytes} bytes)");
    }

    let inbox = prepare_inbox(&serial)?;
    let remote_package = format!("{}/package.aix", inbox.trim_end_matches('/'));
    let remote_definition = format!("{}/agent.json", inbox.trim_end_matches('/'));
    run_adb(
        &serial,
        &["shell", "rm", "-f", &remote_package, &remote_definition],
    )?;

    run_adb(
        &serial,
        &[
            "push",
            package_path.to_str().context("package path is not UTF-8")?,
            staged_definition
                .to_str()
                .context("Definition path is not UTF-8")?,
            &inbox,
        ],
    )?;
    let applied = parse_develop_result(&run_adb(&serial, &content_call("apply", None))?)?;
    require_empty_error_code(&applied, "apply")?;
    if applied.get("agentId").and_then(Value::as_str) != Some(agent_id.as_str()) {
        bail!("apply agentId does not match agent.json");
    }
    let outcome = applied
        .get("outcome")
        .and_then(Value::as_str)
        .context("apply outcome is missing")?;
    if !["CREATED", "UPDATED", "UNCHANGED", "REPAIRED"].contains(&outcome) {
        bail!("unexpected apply outcome: {outcome}");
    }
    let operation_id = applied
        .get("operationId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("apply operationId is missing")?;

    let status = parse_develop_result(&run_adb(
        &serial,
        &content_call("status", Some(operation_id)),
    )?)?;
    require_empty_error_code(&status, "status")?;
    let publish_state = status
        .get("publishState")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");
    if ["FAILED_RETRYABLE", "FAILED_PERMANENT"].contains(&publish_state) {
        bail!("upload stopped with publishState={publish_state}");
    }

    let agent_name = definition
        .get("agentName")
        .and_then(Value::as_str)
        .unwrap_or(&agent_id);
    println!("✔ Agent installed: {agent_name}");
    println!("  ID: {agent_id}");
    println!("  Device: {serial}");
    println!("  Result: {}", outcome.to_ascii_lowercase());
    println!(
        "  Phone upload: {}",
        if publish_state == "UPLOADED" {
            "confirmed".to_owned()
        } else {
            publish_state.to_ascii_lowercase()
        }
    );
    println!("  Cloud indexing: not verified");
    Ok(())
}

fn read_launch_metadata(input: &Path, is_project: bool) -> Result<(Value, String, Option<String>)> {
    if is_project {
        let app: Value = serde_json::from_slice(&fs::read(input.join("app.json"))?)?;
        let state_dir = input.join(".aix");
        let id_path = state_dir.join("agent-id");
        let id = if id_path.exists() {
            let value = fs::read_to_string(&id_path)?.trim().to_owned();
            if value.is_empty() {
                bail!("{} is empty", id_path.display());
            }
            value
        } else {
            fs::create_dir_all(&state_dir)?;
            let value = Uuid::new_v4().to_string();
            fs::write(&id_path, format!("{value}\n"))?;
            value
        };
        let agents_path = input.join("AGENTS.md");
        let agents = agents_path
            .exists()
            .then(|| fs::read_to_string(agents_path))
            .transpose()?;
        return Ok((app, format_agent_id(&id)?, agents));
    }
    let reader = aix::AixReader::new(fs::read(input)?)?;
    let version = String::from_utf8(reader.read_file("VERSION")?)?;
    let app: Value = serde_json::from_slice(&reader.read_file("app.json")?)?;
    let agents = reader
        .read_file("AGENTS.md")
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    Ok((app, format_agent_id(version.trim())?, agents))
}

fn format_agent_id(id: &str) -> Result<String> {
    let normalized = id
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    if normalized.is_empty() {
        bail!("AIX version/agent ID cannot be normalized");
    }
    Ok(format!("develop.rokid.agent.{normalized}"))
}

fn build_definition(app: &Value, agent_id: &str, agents_text: Option<&str>) -> Value {
    let name = string_field(app, &["agentName", "name"]).unwrap_or("AIX Agent");
    let description = string_field(app, &["agentDesc", "description"])
        .map(str::to_owned)
        .or_else(|| first_paragraph(agents_text).map(str::to_owned))
        .unwrap_or_else(|| name.to_owned());
    serde_json::json!({
        "schemaVersion": 1,
        "agentId": agent_id,
        "agentName": name,
        "agentDesc": description,
        "agentPrompt": string_field(app, &["agentPrompt"]).unwrap_or(""),
        "agentLogo": string_field(app, &["agentLogo", "logo", "icon"]).unwrap_or(""),
        "nativeVersion": string_field(app, &["nativeVersion", "version"]).unwrap_or("1.0.0"),
        "inkVersion": string_field(app, &["inkVersion", "engine"]).unwrap_or(">=0.17.0"),
        "permissions": app.get("permissions").and_then(Value::as_array).cloned().unwrap_or_default(),
    })
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
    })
}

fn first_paragraph(markdown: Option<&str>) -> Option<&str> {
    markdown?
        .split("\n\n")
        .map(str::trim)
        .find(|part| !part.is_empty() && !part.starts_with('#') && !part.starts_with('<'))
}

fn validate_definition(definition: &Value) -> Result<&str> {
    let agent_id = definition
        .as_object()
        .and_then(|object| object.get("agentId"))
        .and_then(Value::as_str)
        .context("agent.json agentId is missing")?;
    let parts: Vec<_> = agent_id.split('.').collect();
    if parts.len() < 4
        || parts[0] != "develop"
        || parts[1] != "rokid"
        || parts[2..].iter().any(|part| part.is_empty())
    {
        bail!("agent.json agentId must use the develop.rokid.<business>.<id> format");
    }
    Ok(agent_id)
}

fn select_device(requested: Option<&str>) -> Result<String> {
    let output = run_command("adb", &["devices"])?;
    let online: Vec<_> = output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            (fields.next()? == "device").then(|| serial.to_owned())
        })
        .collect();
    if let Some(requested) = requested {
        if !online.iter().any(|serial| serial == requested) {
            bail!("ADB device is not online or authorized: {requested}");
        }
        return Ok(requested.to_owned());
    }
    if online.is_empty() {
        bail!("no online, authorized ADB device found");
    }
    if online.len() == 1 {
        return Ok(online[0].clone());
    }
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        bail!("multiple ADB devices found; pass --serial <serial> in a non-interactive shell");
    }

    let mut stderr = std::io::stderr();
    writeln!(stderr, "Select a Rokid Glasses device:")?;
    for (index, serial) in online.iter().enumerate() {
        writeln!(stderr, "  {}) {serial}", index + 1)?;
    }
    loop {
        write!(stderr, "Device [1-{}]: ", online.len())?;
        stderr.flush()?;
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer)? == 0 {
            bail!("device selection cancelled");
        }
        if let Ok(index) = answer.trim().parse::<usize>() {
            if let Some(serial) = index.checked_sub(1).and_then(|index| online.get(index)) {
                return Ok(serial.clone());
            }
        }
        writeln!(stderr, "Enter a number between 1 and {}.", online.len())?;
    }
}

fn content_call<'a>(method: &'a str, arg: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec![
        "shell",
        "content",
        "call",
        "--uri",
        DEVELOP_URI,
        "--method",
        method,
    ];
    if let Some(arg) = arg {
        args.extend(["--arg", arg]);
    }
    args
}

fn prepare_inbox(serial: &str) -> Result<String> {
    let prepared = parse_develop_result(&run_adb(serial, &content_call("prepare", None))?)?;
    require_empty_error_code(&prepared, "prepare")?;
    if prepared.get("ready").and_then(Value::as_bool) != Some(true) {
        bail!("prepare did not report ready=true");
    }
    Ok(prepared
        .get("inboxPath")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .unwrap_or(STAGING_DIR.trim_end_matches('/'))
        .to_owned())
}

fn run_adb(serial: &str, args: &[&str]) -> Result<String> {
    let mut adb_args = vec!["-s", serial];
    adb_args.extend_from_slice(args);
    run_command("adb", &adb_args)
}

fn run_open_content_call(serial: &str, request_json: &str) -> Result<String> {
    let remote_command = format!(
        "content call --uri {DEVELOP_URI} --method open --arg {}",
        shell_quote(request_json)
    );
    run_adb(serial, &["shell", &remote_command])
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn run_command(command: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .with_context(|| format!("failed to start {command}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        });
        bail!("{command} {} failed: {}", args.join(" "), message.trim());
    }
    String::from_utf8(output.stdout).context("command output is not UTF-8")
}

pub(crate) fn parse_develop_result(text: &str) -> Result<Value> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed).context("invalid result JSON");
    }
    let markers: Vec<_> = text.match_indices("result_data").collect();
    if markers.len() != 1 {
        bail!("expected exactly one result_data JSON value");
    }
    let tail = &text[markers[0].0 + "result_data".len()..];
    let tail = tail
        .trim_start()
        .strip_prefix('=')
        .context("result_data is missing '='")?
        .trim_start();
    if !tail.starts_with('{') {
        bail!("result_data is not a JSON object");
    }
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in tail.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        } else if character == '{' {
            depth += 1;
        } else if character == '}' {
            depth -= 1;
            if depth == 0 {
                return serde_json::from_str(&tail[..=index]).context("invalid result_data JSON");
            }
        }
    }
    bail!("incomplete result_data JSON")
}

fn require_empty_error_code(result: &Value, stage: &str) -> Result<()> {
    if result.get("errorCode").and_then(Value::as_str) != Some("") {
        let code = result
            .get("errorCode")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        let message = result
            .get("message")
            .or_else(|| result.get("errorMessage"))
            .and_then(Value::as_str)
            .filter(|message| !message.is_empty());
        if let Some(message) = message {
            bail!("{stage} failed ({code}): {message}");
        }
        bail!("{stage} failed ({code})");
    }
    Ok(())
}

fn require_ok(result: &Value, stage: &str) -> Result<()> {
    require_empty_error_code(result, stage)?;
    if result.get("ok").and_then(Value::as_bool) != Some(true) {
        bail!("{stage} did not report ok=true");
    }
    Ok(())
}

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("aix-install-{}", Uuid::new_v4()));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_android_bundle_result_data() {
        let result = parse_develop_result(
            r#"Result: Bundle[{result_data={"errorCode":"","ready":true,"nested":{"value":"}"}}}]"#,
        )
        .unwrap();
        assert_eq!(result["ready"], true);
        assert_eq!(result["nested"]["value"], "}");
    }

    #[test]
    fn validates_develop_agent_id() {
        let valid = serde_json::json!({"agentId":"develop.rokid.mobility.ride"});
        assert_eq!(
            validate_definition(&valid).unwrap(),
            "develop.rokid.mobility.ride"
        );
        let invalid = serde_json::json!({"agentId":"production.rokid.mobility.ride"});
        assert!(validate_definition(&invalid).is_err());
    }

    #[test]
    fn formats_version_as_namespaced_agent_id() {
        assert_eq!(
            format_agent_id("550E8400-E29B-41D4-A716-446655440000").unwrap(),
            "develop.rokid.agent.550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn builds_definition_from_app_and_agents() {
        let app =
            serde_json::json!({"name":"Ride", "engine":"^0.17.0", "permissions":["INTERNET"]});
        let definition = build_definition(
            &app,
            "develop.rokid.agent.test",
            Some("# Instructions\n\nUse this Agent for rides."),
        );
        assert_eq!(definition["agentName"], "Ride");
        assert_eq!(definition["agentDesc"], "Use this Agent for rides.");
        assert_eq!(definition["inkVersion"], "^0.17.0");
    }

    #[test]
    fn directory_definition_reuses_persisted_agent_id() {
        let temporary = tempdir().unwrap();
        fs::write(
            temporary.path().join("app.json"),
            r#"{"name":"Stable Agent","description":"Stable description"}"#,
        )
        .unwrap();

        let first = resolve_definition(temporary.path(), None).unwrap();
        let second = resolve_definition(temporary.path(), None).unwrap();
        assert_eq!(first.value["agentId"], second.value["agentId"]);
        assert!(temporary.path().join(".aix/agent-id").is_file());
        assert_eq!(first.value["agentName"], "Stable Agent");
    }

    #[test]
    fn infers_page_and_widget_targets() {
        let app = serde_json::json!({
            "pages": ["pages/index/index"],
            "widgets": [{"path": "widgets/order/index", "family": "1x2"}]
        });
        assert_eq!(
            resolve_open_route(&app, None, None).unwrap(),
            ("pages/index/index".to_owned(), "_blank")
        );
        assert_eq!(
            resolve_open_route(&app, Some("widgets/order/index"), None).unwrap(),
            ("widgets/order/index".to_owned(), "_widget")
        );
        assert!(resolve_open_route(&app, Some("widgets/order/index"), Some("current")).is_err());
    }

    #[test]
    fn launch_params_must_be_an_object() {
        assert_eq!(
            parse_open_params(None, None).unwrap(),
            serde_json::json!({})
        );
        assert_eq!(
            parse_open_params(Some(r#"{"orderId":"123"}"#), None).unwrap(),
            serde_json::json!({"orderId":"123"})
        );
        assert!(parse_open_params(Some("[]"), None).is_err());
    }

    #[test]
    fn quotes_open_json_for_the_remote_shell() {
        assert_eq!(
            shell_quote(r#"{"value":"it's ready"}"#),
            r#"'{"value":"it'"'"'s ready"}'"#
        );
    }

    #[test]
    fn derives_widget_size_from_family() {
        let app = serde_json::json!({"widgets":[{"path":"widgets/order/index","family":"1x2"}]});
        assert_eq!(
            widget_family_size(&app, "widgets/order/index").unwrap(),
            (2, 1)
        );
    }

    #[test]
    fn upserts_and_reuses_overlay_widget_position() {
        let mut configuration = empty_widget_configuration();
        assert!(upsert_overlay_widget(
            &mut configuration,
            "develop.rokid.agent.test",
            "widgets/order/index",
            2,
            1,
            None,
        )
        .unwrap());
        assert_eq!(configuration["overlayModules"][0]["startIndex"], 0);
        assert_eq!(configuration["overlayModules"][0]["name"], "custom_1");
        assert!(!upsert_overlay_widget(
            &mut configuration,
            "develop.rokid.agent.test",
            "widgets/order/index",
            2,
            1,
            None,
        )
        .unwrap());
    }

    #[test]
    fn replaces_unsupported_widget_module_name() {
        let mut configuration = empty_widget_configuration();
        configuration["overlayModules"] = serde_json::json!([{
            "name":"aix_widgets_order_index", "startIndex":0, "columnSize":2, "rowSize":1,
            "agentId":"develop.rokid.agent.test", "path":"widgets/order/index"
        }]);
        assert!(upsert_overlay_widget(
            &mut configuration,
            "develop.rokid.agent.test",
            "widgets/order/index",
            2,
            1,
            None,
        )
        .unwrap());
        assert_eq!(configuration["overlayModules"][0]["name"], "custom_1");
    }

    #[test]
    fn preserves_compatible_widgets_and_clears_them_when_full() {
        let mut configuration = empty_widget_configuration();
        for (agent, widget) in [
            ("develop.rokid.agent.first", "widgets/first/index"),
            ("develop.rokid.agent.second", "widgets/second/index"),
        ] {
            assert!(upsert_overlay_widget(&mut configuration, agent, widget, 2, 1, None,).unwrap());
        }
        assert_eq!(configuration["overlayModules"].as_array().unwrap().len(), 2);

        assert!(upsert_overlay_widget(
            &mut configuration,
            "develop.rokid.agent.third",
            "widgets/third/index",
            2,
            1,
            None,
        )
        .unwrap());
        assert_eq!(configuration["overlayModules"].as_array().unwrap().len(), 1);
        assert_eq!(configuration["overlayModules"][0]["startIndex"], 0);
        assert_eq!(
            configuration["overlayModules"][0]["agentId"],
            "develop.rokid.agent.third"
        );
    }

    #[test]
    fn replaces_a_conflicting_dynamic_widget_by_default() {
        let mut configuration = empty_widget_configuration();
        configuration["overlayModules"] = serde_json::json!([{
            "name":"custom_1", "startIndex":0, "columnSize":2, "rowSize":1,
            "agentId":"develop.rokid.agent.first", "path":"widgets/first/index"
        }]);
        assert!(upsert_overlay_widget(
            &mut configuration,
            "develop.rokid.agent.second",
            "widgets/second/index",
            2,
            1,
            Some(0),
        )
        .unwrap());
        assert_eq!(configuration["overlayModules"].as_array().unwrap().len(), 1);
        assert_eq!(
            configuration["overlayModules"][0]["agentId"],
            "develop.rokid.agent.second"
        );
    }

    #[test]
    fn refuses_to_replace_permanent_widget_cells() {
        let mut configuration = empty_widget_configuration();
        configuration["permanentModules"] = serde_json::json!([{
            "name":"clock", "startIndex":0, "columnSize":1, "rowSize":1,
            "agentId":"builtin.clock", "path":"widgets/clock/index"
        }]);
        assert!(upsert_overlay_widget(
            &mut configuration,
            "develop.rokid.agent.test",
            "widgets/order/index",
            1,
            1,
            Some(0),
        )
        .is_err());
    }
}
