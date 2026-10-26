//! Bounded test-only host that runs the module graph embedded in a packed
//! `.spkg` (an `sf.substreams.v1.Package`). Each module's inputs and block
//! filter are read from the package, so a rewired manifest changes what runs.
//! Same limits and host imports as `legacy_wasm.rs`: no stores, WASI, network
//! or filesystem; 64 MiB of memory, 4 MiB per input and output, bounded fuel.
//!
//! Shared by the `committed_` tests of `svm-transfers` and `svm-dex`.
#![allow(dead_code)]
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use substreams::buffa::Message;
use substreams::pb::sf::substreams::index::v1::Keys;
use substreams::pb::sf::substreams::{self as modules, module::block_filter::Query, module::input::Input};
use substreams_database_change::pb::sf::substreams::sink::database::v1::{table_change::PrimaryKey, DatabaseChanges};
use wasmi::{Caller, Config, Engine, Error, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const FUEL: u64 = 4_000_000_000;

struct Host {
    limits: StoreLimits,
    output: Option<Vec<u8>>,
    panicked: bool,
}

/// Fields of one protobuf message: (number, length-delimited bytes, varint).
fn fields(mut bytes: &[u8]) -> Vec<(u64, &[u8], u64)> {
    fn varint(bytes: &mut &[u8]) -> u64 {
        let (mut value, mut shift) = (0u64, 0);
        loop {
            let byte = bytes[0];
            *bytes = &bytes[1..];
            value |= u64::from(byte & 0x7f) << shift;
            shift += 7;
            if byte < 0x80 {
                return value;
            }
        }
    }
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let key = varint(&mut bytes);
        match key & 7 {
            0 => out.push((key >> 3, &[][..], varint(&mut bytes))),
            1 => bytes = &bytes[8..],
            2 => {
                let length = varint(&mut bytes) as usize;
                out.push((key >> 3, &bytes[..length], 0));
                bytes = &bytes[length..];
            }
            5 => bytes = &bytes[4..],
            other => panic!("unsupported wire type {other}"),
        }
    }
    out
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub struct Package {
    pub path: PathBuf,
    pub sha256: String,
    /// (name, version) of each `package_meta` entry; the first is the package
    /// itself, the others its imports.
    pub meta: Vec<(String, String)>,
    engine: Engine,
    binaries: Vec<Module>,
    binary_sha256: Vec<String>,
    modules: HashMap<String, modules::Module>,
}

/// One block's inputs: the raw `sf.solana.type.v1.Block` and its `Clock`.
pub struct Sources<'a> {
    pub block: &'a [u8],
    pub clock: &'a [u8],
    /// Replaces a module's packed `params` input.
    pub params: &'a [(&'a str, &'a str)],
}

impl Package {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let spkg = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let package = fields(&spkg);
        let (_, encoded, _) = *package.iter().find(|(number, _, _)| *number == 6).expect("Package.modules");
        let decoded = modules::Modules::decode_from_slice(encoded).unwrap();
        let meta = package
            .iter()
            .filter(|(number, _, _)| *number == 8)
            .map(|(_, value, _)| {
                let meta = fields(value);
                let text = |n: u64| {
                    meta.iter()
                        .find(|f| f.0 == n)
                        .map(|f| String::from_utf8(f.1.to_vec()).unwrap())
                        .unwrap_or_default()
                };
                (text(3), text(1))
            })
            .collect();
        Self {
            sha256: sha256(&spkg),
            meta,
            binaries: decoded
                .binaries
                .iter()
                .map(|binary| Module::new(&engine, &binary.content[..]).unwrap())
                .collect(),
            binary_sha256: decoded.binaries.iter().map(|binary| sha256(&binary.content)).collect(),
            modules: decoded.modules.into_iter().map(|module| (module.name.clone(), module)).collect(),
            engine,
            path,
        }
    }

    pub fn module_names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.modules.keys().map(String::as_str).collect();
        names.sort();
        names
    }

    pub fn has_module(&self, name: &str) -> bool {
        self.modules.contains_key(name)
    }

    fn module(&self, name: &str) -> &modules::Module {
        self.modules.get(name).unwrap_or_else(|| panic!("module {name} not in {}", self.path.display()))
    }

    /// SHA-256 of the binary a module runs.
    pub fn binary_of(&self, name: &str) -> &str {
        &self.binary_sha256[self.module(name).binary_index as usize]
    }

    /// A module's wiring as text: its entrypoint, inputs and block filter.
    /// `strip` removes an import prefix from referenced module names.
    pub fn wiring(&self, name: &str, strip: &str) -> String {
        let module = self.module(name);
        let local = |name: &str| name.strip_prefix(strip).unwrap_or(name).to_string();
        let inputs: Vec<String> = module
            .inputs
            .iter()
            .map(|input| match input.input.as_ref().expect("module input") {
                Input::Source(source) => format!("source {}", source.r#type),
                Input::Map(map) => format!("map {}", local(&map.module_name)),
                Input::Params(params) => format!("params {:?}", params.value),
                other => format!("{other:?}"),
            })
            .collect();
        let filter = module
            .block_filter
            .as_option()
            .map(|filter| format!(" filter {} {:?}", local(&filter.module), filter.query));
        format!("{} <- [{}]{}", module.binary_entrypoint, inputs.join(", "), filter.unwrap_or_default())
    }

    /// Runs one map or index module on explicit inputs; `None` is a
    /// registered package panic (a handler error).
    pub fn run(&self, name: &str, inputs: &[&[u8]]) -> Option<Vec<u8>> {
        let definition = self.module(name);
        let module = &self.binaries[definition.binary_index as usize];
        let mut store = Store::new(
            &self.engine,
            Host {
                limits: StoreLimitsBuilder::new().memory_size(64 * 1024 * 1024).instances(1).memories(1).build(),
                output: None,
                panicked: false,
            },
        );
        store.limiter(|host| &mut host.limits);
        store.set_fuel(FUEL).unwrap();
        let mut linker = Linker::<Host>::new(&self.engine);
        linker.func_wrap("logger", "println", |_pointer: i32, _length: i32| {}).unwrap();
        linker.func_wrap("env", "skip_empty_output", || {}).unwrap();
        linker
            .func_wrap(
                "env",
                "register_panic",
                |mut caller: Caller<'_, Host>, _: i32, _: i32, _: i32, _: i32, _: i32, _: i32| {
                    caller.data_mut().panicked = true;
                },
            )
            .unwrap();
        linker
            .func_wrap(
                "env",
                "output",
                |mut caller: Caller<'_, Host>, pointer: i32, length: i32| -> Result<(), Error> {
                    let pointer = usize::try_from(pointer).map_err(|_| Error::new("negative output pointer"))?;
                    let length = usize::try_from(length).map_err(|_| Error::new("negative output length"))?;
                    if length > MAX_BYTES || caller.data().output.is_some() {
                        return Err(Error::new("oversized or duplicate output"));
                    }
                    let memory = caller
                        .get_export("memory")
                        .and_then(|value| value.into_memory())
                        .ok_or_else(|| Error::new("missing memory"))?;
                    let mut bytes = vec![0; length];
                    memory.read(&caller, pointer, &mut bytes).map_err(|_| Error::new("invalid output range"))?;
                    caller.data_mut().output = Some(bytes);
                    Ok(())
                },
            )
            .unwrap();
        // `+wasm-bindgen-shims` binaries link these imports; the handlers
        // under test must never call them.
        for import in module.imports().filter(|import| import.module().starts_with("__wbindgen_")) {
            linker
                .func_new(import.module(), import.name(), import.ty().func().unwrap().clone(), |_, _, _| {
                    Err(Error::new("unexpected JavaScript host invocation"))
                })
                .unwrap();
        }
        let instance = linker.instantiate(&mut store, module).unwrap().start(&mut store).unwrap();
        let allocate = instance.get_typed_func::<i32, i32>(&store, "alloc").unwrap();
        let memory = instance.get_memory(&store, "memory").unwrap();
        let mut arguments = Vec::with_capacity(inputs.len() * 2);
        for bytes in inputs {
            assert!(bytes.len() <= MAX_BYTES, "{name}: input of {} bytes", bytes.len());
            let length = i32::try_from(bytes.len()).unwrap();
            let pointer = allocate.call(&mut store, length).unwrap();
            memory.write(&mut store, usize::try_from(pointer).unwrap(), bytes).unwrap();
            arguments.extend([Val::I32(pointer), Val::I32(length)]);
        }
        let result = instance
            .get_func(&store, &definition.binary_entrypoint)
            .unwrap_or_else(|| panic!("{name}: no export {}", definition.binary_entrypoint))
            .call(&mut store, &arguments, &mut []);
        if store.data().panicked {
            assert!(result.is_err(), "a registered panic must trap");
            None
        } else {
            result.unwrap_or_else(|error| panic!("{name}: unexpected WASM host/trap/fuel failure: {error}"));
            Some(store.data_mut().output.take().unwrap_or_default())
        }
    }

    /// Evaluates `name` and the modules it depends on, as wired in the
    /// package. A module whose block filter does not match the block's index
    /// keys is skipped and gives an empty output, as in the Substreams engine.
    /// `Err` names the module that registered a panic.
    pub fn evaluate(&self, name: &str, sources: &Sources) -> Result<Vec<u8>, String> {
        self.evaluate_memo(name, sources, &mut HashMap::new())
    }

    fn evaluate_memo(&self, name: &str, sources: &Sources, memo: &mut HashMap<String, Result<Vec<u8>, String>>) -> Result<Vec<u8>, String> {
        if let Some(result) = memo.get(name) {
            return result.clone();
        }
        let module = self.module(name);
        let result = (|| {
            if let Some(filter) = module.block_filter.as_option() {
                let keys = Keys::decode_from_slice(&self.evaluate_memo(&filter.module, sources, memo)?).unwrap();
                let Some(Query::QueryString(query)) = &filter.query else {
                    panic!("{name}: unsupported block filter query {:?}", filter.query);
                };
                if !substreams::matches_keys_in_parsed_expr(&keys.keys, query).unwrap() {
                    return Ok(Vec::new());
                }
            }
            let mut inputs = Vec::with_capacity(module.inputs.len());
            for input in &module.inputs {
                inputs.push(match input.input.as_ref().expect("module input") {
                    Input::Source(source) => match source.r#type.as_str() {
                        "sf.solana.type.v1.Block" => sources.block.to_vec(),
                        "sf.substreams.v1.Clock" => sources.clock.to_vec(),
                        other => panic!("{name}: unsupported source {other}"),
                    },
                    Input::Map(map) => self.evaluate_memo(&map.module_name, sources, memo)?,
                    Input::Params(params) => match sources.params.iter().find(|(module, _)| *module == name) {
                        Some((_, value)) => value.as_bytes().to_vec(),
                        None => params.value.as_bytes().to_vec(),
                    },
                    other => panic!("{name}: unsupported input {other:?}"),
                });
            }
            let inputs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
            self.run(name, &inputs).ok_or_else(|| format!("{name} registered a panic"))
        })();
        memo.insert(name.to_string(), result.clone());
        result
    }
}

/// `name` and `version` of a manifest's `package` section, and its local
/// `.spkg` imports as (alias, path relative to the manifest).
pub struct Manifest {
    pub dir: PathBuf,
    pub name: String,
    pub version: String,
    pub imports: Vec<(String, String)>,
}

impl Manifest {
    pub fn read(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        let text = std::fs::read_to_string(dir.join("substreams.yaml")).unwrap();
        let (mut section, mut name, mut version, mut imports) = (String::new(), None, None, Vec::new());
        for line in text.lines() {
            if !line.starts_with(' ') && line.ends_with(':') {
                section = line.trim_end_matches(':').to_string();
                continue;
            }
            let Some((key, value)) = line.trim().split_once(": ") else { continue };
            match section.as_str() {
                "package" if line.starts_with("  ") && !line.starts_with("   ") => match key {
                    "name" => name = Some(value.to_string()),
                    "version" => version = Some(value.to_string()),
                    _ => {}
                },
                "imports" if value.ends_with(".spkg") => imports.push((key.to_string(), value.to_string())),
                _ => {}
            }
        }
        Self {
            name: name.expect("package.name"),
            version: version.expect("package.version"),
            imports,
            dir,
        }
    }

    /// `spkg/{name}-{version}.spkg`, the file `substreams pack -o
    /// spkg/{spkgDefaultName}` writes.
    pub fn default_spkg_name(&self) -> String {
        format!("{}-{}.spkg", self.name.replace('_', "-"), self.version)
    }
}

/// Checks the committed packages of release manifests (directories relative
/// to the repository root): each manifest's own `spkg/` file and every local
/// import exist with the name and version their file names state; an import
/// of another listed manifest uses that manifest's current version; an
/// aggregate embeds exactly the binaries of the committed packages it
/// imports; and each `released` file keeps its recorded SHA-256.
pub fn check_committed_packages(root: &Path, manifests: &[&str], released: &[(&str, &str)]) -> Vec<String> {
    let mut errors = Vec::new();
    let manifests: Vec<Manifest> = manifests.iter().map(|dir| Manifest::read(root.join(dir))).collect();
    let file_meta = |path: &Path| -> Option<(String, String)> {
        let file = path.file_name()?.to_str()?.strip_suffix(".spkg")?;
        let (name, version) = file.rsplit_once("-v")?;
        Some((name.replace('-', "_"), format!("v{version}")))
    };
    for manifest in &manifests {
        let own = root.join("spkg").join(manifest.default_spkg_name());
        if !own.exists() {
            errors.push(format!("{}: {} is not committed", manifest.dir.display(), own.display()));
            continue;
        }
        let package = Package::open(&own);
        if package.meta.first() != Some(&(manifest.name.clone(), manifest.version.clone())) {
            errors.push(format!("{}: package_meta {:?}", own.display(), package.meta.first()));
        }
        for (alias, import) in &manifest.imports {
            let path = manifest.dir.join(import);
            if !path.exists() {
                errors.push(format!("{}: import {alias} {} does not exist", manifest.dir.display(), path.display()));
                continue;
            }
            let imported = Package::open(&path);
            if imported.meta.first() != file_meta(&path).as_ref() {
                errors.push(format!(
                    "{}: package_meta {:?} does not match its file name",
                    path.display(),
                    imported.meta.first()
                ));
            }
            if let Some(current) = manifests.iter().find(|other| Some(&other.name) == imported.meta.first().map(|meta| &meta.0)) {
                if Some(&current.version) != imported.meta.first().map(|meta| &meta.1) {
                    errors.push(format!(
                        "{}: imports {alias} {:?}, but {} is at {}",
                        manifest.dir.display(),
                        imported.meta.first(),
                        current.dir.display(),
                        current.version
                    ));
                }
            }
            // A `use: alias:module` manifest takes the imported module's name.
            for module in imported.module_names() {
                let prefixed = format!("{alias}:{module}");
                let embedded = if package.has_module(&prefixed) {
                    prefixed
                } else if package.has_module(module) && imported.binary_of(module) == package.binary_of(module) {
                    module.to_string()
                } else {
                    continue;
                };
                if package.binary_of(&embedded) != imported.binary_of(module) || package.wiring(&embedded, &format!("{alias}:")) != imported.wiring(module, "")
                {
                    errors.push(format!("{}: {embedded} differs from {} {module}", own.display(), path.display()));
                }
            }
        }
    }
    for (file, expected) in released {
        let path = root.join("spkg").join(file);
        match std::fs::read(&path) {
            Ok(bytes) if sha256(&bytes) == *expected => {}
            Ok(bytes) => errors.push(format!(
                "released {file} changed: sha256 {} != {expected}; bump the version instead",
                sha256(&bytes)
            )),
            Err(error) => errors.push(format!("released {file}: {error}")),
        }
    }
    errors
}

/// A stable digest of database rows, in row order: table, primary key
/// (composite keys sorted by name), ordinal, operation and fields sorted by
/// name. Encoded bytes are not stable: field order and composite keys come
/// from hash maps.
pub fn database_digest(changes: &DatabaseChanges) -> String {
    use std::fmt::Write;
    let mut text = String::new();
    for row in &changes.table_changes {
        let key = match &row.primary_key {
            Some(PrimaryKey::Pk(key)) => key.clone(),
            Some(PrimaryKey::CompositePk(composite)) => {
                let keys: std::collections::BTreeMap<_, _> = composite.keys.iter().collect();
                format!("{keys:?}")
            }
            None => String::new(),
        };
        let fields: std::collections::BTreeMap<_, _> = row.fields.iter().map(|field| (&field.name, (&field.value, field.update_op))).collect();
        writeln!(text, "{}\t{key}\t{}\t{:?}\t{fields:?}", row.table, row.ordinal, row.operation).unwrap();
    }
    sha256(text.as_bytes())
}
