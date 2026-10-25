//! Minimal, bounded host for the three immutable historical map binaries.
//! No stores, RPC, WASI, network, filesystem or production runtime integration.
use sha2::{Digest, Sha256};
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
    swaps: Module,
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
            include_bytes!("../fixtures/legacy-v0.5.1/db_out.wasm"),
            "c7e8e781484b615115c55761db49a165c25575a78a31b1bc85a3b978c674a5b4",
        );
        let swaps = module(
            include_bytes!("../fixtures/legacy-v0.5.1/dex_swaps.wasm"),
            "5f00c0e2dc5161fede0d8b25226a8c0ab8405823b4796e2b29692209aca4e087",
        );
        let filter = module(
            include_bytes!("../fixtures/legacy-v0.5.1/blocks_without_votes.wasm"),
            "3507399c9f8091276b72d750632c6979e78e42a0137de8cee1be5e61b8cdb392",
        );
        Self {
            engine,
            filter,
            swaps,
            database,
        }
    }
    pub fn events(&self, block: &[u8]) -> Option<Vec<u8>> {
        let filtered = self.run(&self.filter, "blocks_without_votes", &[block])?;
        self.run(&self.swaps, "map_events", &[&filtered])
    }
    pub fn format(&self, clock: &[u8], events: &[u8]) -> Option<Vec<u8>> {
        self.run(&self.database, "db_out", &[clock, events])
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
        // The 3 MiB complete-block oracle exceeds one billion interpreter fuel
        // units. Small synthetic maps retain that bound; whole-block modules
        // get a finite five-billion budget without widening memory or IO.
        let fuel = if inputs.iter().map(|input| input.len()).sum::<usize>() > 1024 * 1024 {
            5_000_000_000
        } else {
            1_000_000_000
        };
        store.set_fuel(fuel).unwrap();
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
            result.unwrap_or_else(|error| panic!("unexpected {function} WASM host/trap/fuel failure: {error}"));
            Some(store.data_mut().output.take().unwrap_or_default())
        }
        // Dropping this fresh Store releases all allocations after every module.
    }
}
