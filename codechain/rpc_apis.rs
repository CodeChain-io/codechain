use codechain_rpc::{MetaIoHandler, Params, Value};

pub fn setup_rpc(mut handler: MetaIoHandler<()>) -> MetaIoHandler<()> {
    handler.add_method("ping", |_params: Params| {
        Ok(Value::String("pong".to_string()))
    });
    handler
}

