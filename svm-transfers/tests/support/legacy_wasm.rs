//! Minimal, bounded host for the four immutable historical map binaries.
//! No stores, RPC, WASI, network, filesystem or production runtime integration.
use sha2::{Digest, Sha256};
use substreams::buffa::Message;
use wasmi::{Caller, Config, Engine, Error, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val};

const MAX_BYTES: usize = 4 * 1024 * 1024;

struct Host {
    limits: StoreLimits,
    output: Option<Vec<u8>>,
    panicked: bool,
}

pub struct Oracle {
    engine: Engine,
    filter: Module,
    spl: Module,
    system: Module,
    database: Module,
}
impl Oracle {
    pub fn new() -> Self {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = |bytes: &[u8], digest: &str| {
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), digest);
            Module::new(&engine, bytes).unwrap()
        };
        let database = module(
            include_bytes!("../fixtures/legacy-v0.3.0/db_out.wasm"),
            "a838643adbb8fef0ecbf21f130782f2232afd03a79955806e5119aec9c80b07c",
        );
        let spl = module(
            include_bytes!("../fixtures/legacy-v0.3.0/spl_token.wasm"),
            "d372129b9f2c901b2f1f7fb10659904eefa15c89a775a91fedff20ddcadd21f9",
        );
        let filter = module(
            include_bytes!("../fixtures/legacy-v0.3.0/blocks_without_votes.wasm"),
            "4547dba2b5a4a9d7ed8ef3e5f09b3e9e8cc9f6f2775f47ec3db4f9aa6e800d89",
        );
        let system = module(
            include_bytes!("../fixtures/legacy-v0.3.0/native_token.wasm"),
            "84e057e6e1eee295b57dc3deb623107f1ffe572dde36609a913a1d3f16390df2",
        );
        Self {
            engine,
            filter,
            spl,
            system,
            database,
        }
    }
    pub fn database(&self, clock: &[u8], block: &[u8]) -> Option<Vec<u8>> {
        let block = self.run(&self.filter, "blocks_without_votes", &[block])?;
        let keys = self.run(&self.filter, "program_ids_without_votes", &[&block])?;
        let keys = substreams::pb::substreams::index::v1::Keys::decode_from_slice(&keys).unwrap();
        let has_program = |program: &str| keys.keys.iter().any(|key| key == &format!("program:{program}"));
        let spl = if [
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
            "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo",
            "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
        ]
        .into_iter()
        .any(has_program)
        {
            self.run(&self.spl, "map_events", &[&block])?
        } else {
            Vec::new()
        };
        let system = if has_program("11111111111111111111111111111111") {
            self.run(&self.system, "map_events", &[&block])?
        } else {
            Vec::new()
        };
        self.run(&self.database, "db_out", &[clock, &spl, &system])
    }

    fn run(&self, module: &Module, function: &str, inputs: &[&[u8]]) -> Option<Vec<u8>> {
        let mut store = Store::new(
            &self.engine,
            Host {
                limits: StoreLimitsBuilder::new().memory_size(64 * 1024 * 1024).instances(1).memories(1).build(),
                output: None,
                panicked: false,
            },
        );
        store.limiter(|host| &mut host.limits);
        store.set_fuel(1_000_000_000).unwrap();
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
        // The pinned Solana SDK carries unused wasm-bindgen JS declarations.
        // Resolve them only for instantiation; any invocation is a host failure,
        // never a fabricated JavaScript value or an accepted legacy outcome.
        for import in module.imports().filter(|import| import.module().starts_with("__wbindgen_")) {
            linker
                .func_new(import.module(), import.name(), import.ty().func().unwrap().clone(), |_, _, _| {
                    Err(Error::new("unexpected JavaScript host invocation"))
                })
                .unwrap();
        }
        // Every other import must be one of the four explicit Substreams calls.
        let instance = linker.instantiate(&mut store, module).unwrap().start(&mut store).unwrap();
        let allocate = instance.get_typed_func::<i32, i32>(&store, "alloc").unwrap();
        let memory = instance.get_memory(&store, "memory").unwrap();
        let mut arguments = Vec::with_capacity(inputs.len() * 2);
        for bytes in inputs {
            assert!(bytes.len() <= MAX_BYTES);
            let length = i32::try_from(bytes.len()).unwrap();
            let pointer = allocate.call(&mut store, length).unwrap();
            memory.write(&mut store, usize::try_from(pointer).unwrap(), bytes).unwrap();
            arguments.extend([Val::I32(pointer), Val::I32(length)]);
        }
        let result = instance.get_func(&store, function).unwrap().call(&mut store, &arguments, &mut []);
        if store.data().panicked {
            assert!(result.is_err(), "a registered panic must trap");
            assert!(store.data().output.is_none(), "a panic must not publish output");
            None
        } else {
            result.expect("unexpected WASM host/trap/fuel failure");
            Some(store.data_mut().output.take().unwrap_or_default())
        }
        // Dropping this fresh Store releases all allocations after every module.
    }
}
