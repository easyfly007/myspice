#[derive(Debug, Clone)]
pub struct NetlistAst {
    pub title: Option<String>,
    pub statements: Vec<Stmt>,
    pub errors: Vec<ParseError>,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Device(DeviceStmt),
    Control(ControlStmt),
    Comment(String),
    Empty,
}

#[derive(Debug, Clone)]
pub struct DeviceStmt {
    pub name: String,
    pub kind: DeviceKind,
    pub nodes: Vec<String>,
    pub model: Option<String>,
    pub control: Option<String>,
    pub value: Option<String>,
    pub params: Vec<Param>,
    pub raw: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ControlStmt {
    pub command: String,
    pub kind: ControlKind,
    pub args: Vec<String>,
    pub params: Vec<Param>,
    pub model_name: Option<String>,
    pub model_type: Option<String>,
    pub subckt_name: Option<String>,
    pub subckt_ports: Vec<String>,
    pub raw: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum ControlKind {
    Param,
    Model,
    Subckt,
    Ends,
    Include,
    Op,
    Dc,
    Tran,
    End,
    Other,
}

#[derive(Debug, Clone)]
pub enum DeviceKind {
    R,
    C,
    L,
    V,
    I,
    D,
    M,
    E,
    G,
    F,
    H,
    X,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ElaboratedNetlist {
    pub instances: Vec<DeviceStmt>,
    pub control_count: usize,
    pub error_count: usize,
}

#[derive(Debug, Clone)]
pub struct SubcktDef {
    pub name: String,
    pub ports: Vec<String>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

pub fn parse_netlist_file(path: &std::path::Path) -> NetlistAst {
    let mut errors = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let content = read_with_includes(path, &mut visited, &mut errors);
    let mut ast = parse_netlist(&content);
    ast.errors.extend(errors);
    ast
}

pub fn parse_netlist(input: &str) -> NetlistAst {
    let mut title = None;
    let mut statements = Vec::new();
    let mut errors = Vec::new();
    let mut pending_line = String::new();

    for (index, raw_line) in input.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = raw_line.trim();

        if trimmed.is_empty() {
            statements.push(Stmt::Empty);
            continue;
        }

        if trimmed.starts_with('*') {
            statements.push(Stmt::Comment(trimmed.to_string()));
            continue;
        }

        if trimmed.starts_with('+') {
            if pending_line.is_empty() {
                errors.push(ParseError {
                    line: line_no,
                    message: "续行没有对应的上一行".to_string(),
                });
                continue;
            }
            pending_line.push(' ');
            pending_line.push_str(trimmed.trim_start_matches('+').trim());
            continue;
        }

        if !pending_line.is_empty() {
            parse_statement(&pending_line, line_no, &mut title, &mut statements, &mut errors);
            pending_line.clear();
        }

        pending_line = trimmed.to_string();
    }

    if !pending_line.is_empty() {
        parse_statement(&pending_line, input.lines().count(), &mut title, &mut statements, &mut errors);
    }

    NetlistAst {
        title,
        statements,
        errors,
    }
}

fn parse_statement(
    line: &str,
    line_no: usize,
    title: &mut Option<String>,
    statements: &mut Vec<Stmt>,
    errors: &mut Vec<ParseError>,
) {
    let mut iter = line.split_whitespace();
    let first = match iter.next() {
        Some(token) => token,
        None => {
            statements.push(Stmt::Empty);
            return;
        }
    };

    if first.starts_with('.') {
        let command = first.to_ascii_lowercase();
        let tokens: Vec<&str> = iter.collect();
        let (args, params) = split_args_params(&tokens);
        let kind = map_control_kind(&command);
        let mut model_name = None;
        let mut model_type = None;
        let mut subckt_name = None;
        let mut subckt_ports = Vec::new();

        if command == ".title" {
            let rest = args.join(" ");
            if !rest.is_empty() {
                *title = Some(rest);
            }
        }

        if matches!(kind, ControlKind::Model) {
            if args.len() >= 2 {
                model_name = Some(args[0].clone());
                model_type = Some(args[1].clone());
            } else {
                errors.push(ParseError {
                    line: line_no,
                    message: "model 语句缺少 name/type".to_string(),
                });
            }
        }

        if matches!(kind, ControlKind::Subckt) {
            if !args.is_empty() {
                subckt_name = Some(args[0].clone());
                if args.len() > 1 {
                    subckt_ports = args[1..].to_vec();
                }
            } else {
                errors.push(ParseError {
                    line: line_no,
                    message: "subckt 语句缺少名称".to_string(),
                });
            }
        }

        statements.push(Stmt::Control(ControlStmt {
            command,
            kind,
            args,
            params,
            model_name,
            model_type,
            subckt_name,
            subckt_ports,
            raw: line.to_string(),
            line: line_no,
        }));
        return;
    }

    let kind = match first.chars().next().unwrap_or(' ') {
        'R' | 'r' => DeviceKind::R,
        'C' | 'c' => DeviceKind::C,
        'L' | 'l' => DeviceKind::L,
        'V' | 'v' => DeviceKind::V,
        'I' | 'i' => DeviceKind::I,
        'D' | 'd' => DeviceKind::D,
        'M' | 'm' => DeviceKind::M,
        'E' | 'e' => DeviceKind::E,
        'G' | 'g' => DeviceKind::G,
        'F' | 'f' => DeviceKind::F,
        'H' | 'h' => DeviceKind::H,
        'X' | 'x' => DeviceKind::X,
        _ => DeviceKind::Unknown,
    };

    if matches!(kind, DeviceKind::Unknown) {
        errors.push(ParseError {
            line: line_no,
            message: format!("未知器件类型: {}", first),
        });
    }

    let tokens: Vec<&str> = iter.collect();
    let (args, params) = split_args_params(&tokens);
    let (nodes, model, value) = split_device_fields(&kind, &args);
    let control = extract_control_name(&kind, &args);
    validate_device_fields(first, &kind, &nodes, &model, &control, &value, line_no, errors);

    statements.push(Stmt::Device(DeviceStmt {
        name: first.to_string(),
        kind,
        nodes,
        model,
        control,
        value,
        params,
        raw: line.to_string(),
        line: line_no,
    }));
}

fn split_args_params(tokens: &[&str]) -> (Vec<String>, Vec<Param>) {
    let mut args = Vec::new();
    let mut params = Vec::new();

    for token in tokens {
        if let Some((key, value)) = token.split_once('=') {
            params.push(Param {
                key: key.to_string(),
                value: value.to_string(),
            });
        } else {
            args.push(token.to_string());
        }
    }

    (args, params)
}

fn split_device_fields(
    kind: &DeviceKind,
    args: &[String],
) -> (Vec<String>, Option<String>, Option<String>) {
    if args.is_empty() {
        return (Vec::new(), None, None);
    }

    let mut nodes = Vec::new();
    let mut model = None;
    let mut value = None;

    match kind {
        DeviceKind::R | DeviceKind::C | DeviceKind::L | DeviceKind::V | DeviceKind::I => {
            if args.len() >= 3 {
                nodes.extend_from_slice(&args[0..2]);
                value = Some(args[2].clone());
            } else {
                nodes.extend_from_slice(args);
            }
        }
        DeviceKind::D => {
            if args.len() >= 3 {
                nodes.extend_from_slice(&args[0..2]);
                model = Some(args[2].clone());
            } else {
                nodes.extend_from_slice(args);
            }
        }
        DeviceKind::M => {
            if args.len() >= 5 {
                nodes.extend_from_slice(&args[0..4]);
                model = Some(args[4].clone());
            } else {
                nodes.extend_from_slice(args);
            }
        }
        DeviceKind::E | DeviceKind::G => {
            if args.len() >= 5 {
                nodes.extend_from_slice(&args[0..4]);
                value = Some(args[4].clone());
            } else {
                nodes.extend_from_slice(args);
            }
        }
        DeviceKind::F | DeviceKind::H => {
            if args.len() >= 4 {
                nodes.extend_from_slice(&args[0..2]);
                value = Some(args[3].clone());
            } else {
                nodes.extend_from_slice(args);
            }
        }
        DeviceKind::X => {
            if args.len() >= 2 {
                nodes.extend_from_slice(&args[0..args.len() - 1]);
                model = Some(args[args.len() - 1].clone());
            } else {
                nodes.extend_from_slice(args);
            }
        }
        DeviceKind::Unknown => {
            nodes.extend_from_slice(args);
        }
    }

    (nodes, model, value)
}

fn map_control_kind(command: &str) -> ControlKind {
    match command {
        ".param" => ControlKind::Param,
        ".model" => ControlKind::Model,
        ".subckt" => ControlKind::Subckt,
        ".ends" => ControlKind::Ends,
        ".include" => ControlKind::Include,
        ".op" => ControlKind::Op,
        ".dc" => ControlKind::Dc,
        ".tran" => ControlKind::Tran,
        ".end" => ControlKind::End,
        _ => ControlKind::Other,
    }
}

fn validate_device_fields(
    name: &str,
    kind: &DeviceKind,
    nodes: &[String],
    model: &Option<String>,
    control: &Option<String>,
    value: &Option<String>,
    line_no: usize,
    errors: &mut Vec<ParseError>,
) {
    if matches!(kind, DeviceKind::Unknown) {
        return;
    }

    if nodes.is_empty() {
        errors.push(ParseError {
            line: line_no,
            message: format!("器件缺少节点定义: {}", name),
        });
        return;
    }

    match kind {
        DeviceKind::R
        | DeviceKind::C
        | DeviceKind::L
        | DeviceKind::V
        | DeviceKind::I => {
            if nodes.len() != 2 {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 需要 2 个节点", name),
                });
            }
            if value.is_none() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少数值", name),
                });
            }
        }
        DeviceKind::D => {
            if nodes.len() != 2 {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 需要 2 个节点", name),
                });
            }
            if model.is_none() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少模型名", name),
                });
            }
        }
        DeviceKind::M => {
            if nodes.len() < 4 {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 需要至少 4 个节点", name),
                });
            }
            if model.is_none() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少模型名", name),
                });
            }
        }
        DeviceKind::E | DeviceKind::G => {
            if nodes.len() != 4 {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 需要 4 个节点", name),
                });
            }
            if value.is_none() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少增益值", name),
                });
            }
        }
        DeviceKind::F | DeviceKind::H => {
            if nodes.len() != 2 {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 需要 2 个节点", name),
                });
            }
            if control.is_none() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少控制源", name),
                });
            }
            if value.is_none() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少增益值", name),
                });
            }
        }
        DeviceKind::X => {
            if nodes.is_empty() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少节点", name),
                });
            }
            if model.is_none() {
                errors.push(ParseError {
                    line: line_no,
                    message: format!("{} 缺少子电路名", name),
                });
            }
        }
        DeviceKind::Unknown => {}
    }
}

pub fn elaborate_netlist(ast: &NetlistAst) -> ElaboratedNetlist {
    let mut errors = ast.errors.clone();
    let (top_level, subckts, subckt_errors) = extract_subckts(&ast.statements);
    errors.extend(subckt_errors);

    let param_table = build_param_table(&top_level);
    let mut instances = Vec::new();
    let mut control_count = 0;

    for stmt in top_level {
        match stmt {
            Stmt::Device(device) => {
                if matches!(device.kind, DeviceKind::X) {
                    if let Some(subckt_name) = device.model.as_deref() {
                        if let Some(def) = subckts.iter().find(|d| d.name == subckt_name) {
                            let expanded = expand_subckt_instance(&device, def, &mut errors);
                            for mut inst in expanded {
                                apply_params_to_device(&param_table, &mut inst);
                                instances.push(inst);
                            }
                            continue;
                        }
                    }
                    errors.push(ParseError {
                        line: device.line,
                        message: format!("子电路未定义: {:?}", device.model),
                    });
                    let mut fallback = device.clone();
                    apply_params_to_device(&param_table, &mut fallback);
                    instances.push(fallback);
                } else {
                    let mut inst = device.clone();
                    apply_params_to_device(&param_table, &mut inst);
                    instances.push(inst);
                }
            }
            Stmt::Control(_) => {
                control_count += 1;
            }
            _ => {}
        }
    }

    ElaboratedNetlist {
        instances,
        control_count,
        error_count: errors.len(),
    }
}

fn read_with_includes(
    path: &std::path::Path,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
    errors: &mut Vec<ParseError>,
) -> String {
    if !visited.insert(path.to_path_buf()) {
        errors.push(ParseError {
            line: 0,
            message: format!("include 循环引用: {}", path.display()),
        });
        return String::new();
    }

    let content = std::fs::read_to_string(path).unwrap_or_else(|_| {
        errors.push(ParseError {
            line: 0,
            message: format!("无法读取文件: {}", path.display()),
        });
        String::new()
    });

    let mut out = String::new();
    let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.to_ascii_lowercase().starts_with(".include") {
            let include_path = trimmed
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .trim_matches('"');
            if include_path.is_empty() {
                errors.push(ParseError {
                    line: 0,
                    message: format!("include 语句缺少路径: {}", path.display()),
                });
                continue;
            }
            let include_file = base_dir.join(include_path);
            let nested = read_with_includes(&include_file, visited, errors);
            out.push_str(&nested);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    out
}

fn build_param_table(statements: &[Stmt]) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    for stmt in statements {
        if let Stmt::Control(ctrl) = stmt {
            if matches!(ctrl.kind, ControlKind::Param) {
                for param in &ctrl.params {
                    params.insert(param.key.to_ascii_lowercase(), param.value.clone());
                }
            }
        }
    }
    params
}

fn apply_params_to_device(params: &std::collections::HashMap<String, String>, device: &mut DeviceStmt) {
    if let Some(value) = device.value.clone() {
        if let Some(replaced) = resolve_param(params, &value) {
            device.value = Some(replaced);
        }
    }
    if let Some(model) = device.model.clone() {
        if let Some(replaced) = resolve_param(params, &model) {
            device.model = Some(replaced);
        }
    }
    for param in &mut device.params {
        if let Some(replaced) = resolve_param(params, &param.value) {
            param.value = replaced;
        }
    }
}

fn resolve_param(
    params: &std::collections::HashMap<String, String>,
    token: &str,
) -> Option<String> {
    params.get(&token.to_ascii_lowercase()).cloned()
}

fn extract_subckts(statements: &[Stmt]) -> (Vec<Stmt>, Vec<SubcktDef>, Vec<ParseError>) {
    let mut top_level = Vec::new();
    let mut subckts = Vec::new();
    let mut errors = Vec::new();
    let mut idx = 0;

    while idx < statements.len() {
        match &statements[idx] {
            Stmt::Control(ctrl) if matches!(ctrl.kind, ControlKind::Subckt) => {
                let name = ctrl.subckt_name.clone().unwrap_or_else(|| "unknown".to_string());
                let ports = ctrl.subckt_ports.clone();
                let line = ctrl.line;
                idx += 1;
                let mut body = Vec::new();
                let mut found_ends = false;

                while idx < statements.len() {
                    match &statements[idx] {
                        Stmt::Control(end_ctrl) if matches!(end_ctrl.kind, ControlKind::Ends) => {
                            found_ends = true;
                            idx += 1;
                            break;
                        }
                        stmt => {
                            body.push(stmt.clone());
                            idx += 1;
                        }
                    }
                }

                if !found_ends {
                    errors.push(ParseError {
                        line,
                        message: format!("subckt {} 缺少 .ends", name),
                    });
                }

                subckts.push(SubcktDef {
                    name,
                    ports,
                    body,
                    line,
                });
            }
            stmt => {
                top_level.push(stmt.clone());
                idx += 1;
            }
        }
    }

    (top_level, subckts, errors)
}

fn expand_subckt_instance(
    instance: &DeviceStmt,
    def: &SubcktDef,
    errors: &mut Vec<ParseError>,
) -> Vec<DeviceStmt> {
    let mut expanded = Vec::new();
    let mut port_map = std::collections::HashMap::new();

    if def.ports.len() != instance.nodes.len() {
        errors.push(ParseError {
            line: instance.line,
            message: format!(
                "子电路端口数量不匹配: {} 期望 {} 实际 {}",
                def.name,
                def.ports.len(),
                instance.nodes.len()
            ),
        });
    }

    for (port, node) in def.ports.iter().zip(instance.nodes.iter()) {
        port_map.insert(port.clone(), node.clone());
    }

    for stmt in &def.body {
        match stmt {
            Stmt::Device(dev) => {
                let mut cloned = dev.clone();
                cloned.name = format!("{}.{}", instance.name, dev.name);
                cloned.nodes = dev
                    .nodes
                    .iter()
                    .map(|node| map_subckt_node(instance, &port_map, node))
                    .collect();
                expanded.push(cloned);
            }
            _ => {
                // TODO: 目前仅展开子电路内的器件语句
            }
        }
    }

    expanded
}

fn map_subckt_node(
    instance: &DeviceStmt,
    port_map: &std::collections::HashMap<String, String>,
    node: &str,
) -> String {
    if let Some(mapped) = port_map.get(node) {
        return mapped.clone();
    }
    format!("{}:{}", instance.name, node)
}

fn extract_control_name(kind: &DeviceKind, args: &[String]) -> Option<String> {
    match kind {
        DeviceKind::F | DeviceKind::H => args.get(2).cloned(),
        _ => None,
    }
}
