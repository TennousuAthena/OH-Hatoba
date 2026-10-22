use quote::ToTokens;
use std::{collections::BTreeMap, env, fs, path::PathBuf};
use syn::{FnArg, GenericArgument, Item, Pat, PathArguments, ReturnType, Type};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .canonicalize()
        .unwrap();
    let source = root.join("apps/desktop/src-tauri/src");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let contract_path = root.join("platform/harmonyos/contracts/upstream.generated.json");
    println!("cargo:rerun-if-changed={}", contract_path.display());
    let contracts: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(contract_path).unwrap()).unwrap();
    let contracts: BTreeMap<String, serde_json::Value> = contracts["commands"]
        .as_object()
        .unwrap()
        .values()
        .map(|v| (v["command"].as_str().unwrap().into(), v.clone()))
        .collect();
    let mut modules = String::new();
    let mut supported = Vec::new();
    let mut dispatch = String::from(
        "pub async fn dispatch(app: &host::AppHandle, command: &str, args: serde_json::Value) -> error::AppResult<serde_json::Value> { match command {\n",
    );
    let mut paths: Vec<_> = fs::read_dir(source.join("commands"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .collect();
    paths.sort();
    for mut path in paths {
        let module = path.file_stem().unwrap().to_str().unwrap().to_owned();
        if matches!(module.as_str(), "mod" | "local" | "cli") {
            continue;
        }
        if module == "app" {
            path = root.join("crates/hatoba-runtime/src/commands/app.rs");
        }
        println!("cargo:rerun-if-changed={}", path.display());
        modules.push_str(&format!(
            "#[path = {:?}] pub mod {module};\n",
            path.to_str().unwrap()
        ));
        let parsed = syn::parse_file(&fs::read_to_string(&path).unwrap()).unwrap();
        for item in parsed.items {
            let Item::Fn(function) = item else {
                continue;
            };
            let name = function.sig.ident.to_string();
            // The generated frontend contract, not a separately maintained native command list.
            let Some(contract) = contracts.get(&name) else {
                continue;
            };
            if !matches!(function.vis, syn::Visibility::Public(_)) {
                continue;
            }
            supported.push(name.clone());
            let mut declarations = String::new();
            let mut parameters = Vec::new();
            let wire = contract["parameters"].as_array().unwrap();
            let mut index = 0;
            for input in &function.sig.inputs {
                let FnArg::Typed(input) = input else {
                    panic!("Receiver in command {name}")
                };
                let Pat::Ident(pattern) = input.pat.as_ref() else {
                    panic!("Unknown argument in {name}")
                };
                let ident = pattern.ident.to_string();
                let Type::Path(ty) = input.ty.as_ref() else {
                    panic!("Unsupported command argument type in {name}")
                };
                let kind = ty.path.segments.last().unwrap().ident.to_string();
                if kind == "AppHandle" {
                    parameters.push("app.clone()".into());
                    continue;
                }
                if kind == "State" {
                    parameters.push("crate::state::state(app)".into());
                    continue;
                }
                let parameter = wire
                    .get(index)
                    .expect("Command parameter count changed; regenerate contracts")
                    .as_str()
                    .unwrap();
                let ty = input.ty.to_token_stream().to_string();
                if kind == "Channel" {
                    assert!(
                        contract["channels"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|v| v.as_u64() == Some(index as u64)),
                        "Channel position drift in {name}"
                    );
                    declarations.push_str(&format!("let {ident}: {ty} = app.channel(crate::host::channel_id(&args, {parameter:?})?)?;\n"));
                } else {
                    declarations.push_str(&format!(
                        "let {ident}: {ty} = crate::host::argument(&args, {parameter:?})?;\n"
                    ));
                }
                parameters.push(ident);
                index += 1;
            }
            assert_eq!(
                index,
                wire.len(),
                "Command shape drift in {name}; regenerate contracts"
            );
            let await_ = if function.sig.asyncness.is_some() {
                ".await"
            } else {
                ""
            };
            let fallible = match &function.sig.output {
                ReturnType::Type(_, ty) => {
                    matches!(ty.as_ref(), Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "AppResult" || s.ident == "Result"))
                }
                _ => false,
            };
            let try_ = if fallible { "?" } else { "" };
            let result_type = match &function.sig.output {
                ReturnType::Default => None,
                ReturnType::Type(_, ty) if fallible => {
                    let Type::Path(path) = ty.as_ref() else {
                        unreachable!()
                    };
                    let PathArguments::AngleBracketed(args) =
                        &path.path.segments.last().unwrap().arguments
                    else {
                        panic!("Missing result type in {name}")
                    };
                    let Some(GenericArgument::Type(ty)) = args.args.first() else {
                        panic!("Missing result type in {name}")
                    };
                    Some(ty)
                }
                ReturnType::Type(_, ty) => Some(ty.as_ref()),
            };
            let unit = result_type
                .is_none_or(|ty| matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty()));
            let call = format!(
                "crate::commands::{module}::{name}({}){await_}{try_}",
                parameters.join(", ")
            );
            let result = if unit {
                format!("{call}; Ok(serde_json::Value::Null)")
            } else {
                format!(
                    "let result = {call}; serde_json::to_value(result).map_err(|_| crate::error::AppError::internal(\"Could not encode command result\"))"
                )
            };
            dispatch.push_str(&format!("{name:?} => {{ #[allow(unused_imports)] use crate::commands::{module}::*; #[allow(unused_imports)] use crate::dto::*; #[allow(unused_imports)] use crate::host::ipc::Channel; {declarations} {result} }},\n"));
        }
    }
    dispatch.push_str("_ => Err(error::AppError::invalid(\"command\", \"This command is unavailable on this host\")), } }\n");
    fs::write(out.join("command_modules.rs"), modules).unwrap();
    fs::write(out.join("dispatch.rs"), dispatch).unwrap();
    supported.sort();
    fs::write(
        out.join("supported.rs"),
        format!("pub const SUPPORTED_COMMANDS: &[&str] = &{:?};", supported),
    )
    .unwrap();
    println!("cargo:rerun-if-changed={}", source.join("dto.rs").display());
    let dto = syn::parse_file(&fs::read_to_string(source.join("dto.rs")).unwrap()).unwrap();
    let mut events = String::new();
    for item in dto.items {
        let Item::Struct(item) = item else {
            continue;
        };
        for attr in item.attrs {
            let text = attr.to_token_stream().to_string();
            if !text.contains("event_name") {
                continue;
            }
            let name = text
                .split("event_name")
                .nth(1)
                .unwrap()
                .split('"')
                .nth(1)
                .unwrap();
            events.push_str(&format!(
                "impl host::Event for dto::{} {{ const NAME: &'static str = {name:?}; }}\n",
                item.ident
            ));
        }
    }
    fs::write(out.join("events.rs"), events).unwrap();
    // Reuse the exact Worker produced for the upstream desktop build.
    let worker = source.join("../gen/worker/bundle.json");
    println!("cargo:rerun-if-changed={}", worker.display());
    let bundle = match fs::read(worker) {
        Ok(bundle) => bundle,
        Err(error) if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("ohos") => {
            panic!(
                "Build the shared Worker bundle with pnpm worker:bundle before the OHOS runtime: {error}"
            );
        }
        Err(_) => Vec::new(),
    };
    fs::write(out.join("worker-bundle.json"), bundle).unwrap();
}
