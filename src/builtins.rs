use std::io::stdin;

use crate::{
    error::Error,
    interpreter::{BuiltinCtx, Value},
    symbols::{
        BuiltinInput, CallableBody, CallableSymbol, CallableSymbolRef, ParamMode, SymbolTable,
        TypeSymbol, TypeSymbolRef, VarSymbol, VarSymbolRef,
    },
    utils::NodePool,
};

fn writeln(
    _: &mut dyn BuiltinCtx<Value = Value>,
    args: &[&BuiltinInput],
) -> Result<Option<Value>, Error> {
    args.iter()
        .map(|v| match v {
            BuiltinInput::Value(v) => {
                println!("{}", v.to_string());
                Ok(())
            }
            BuiltinInput::Ref { name: _ } => Err(Error::InterpreterError {
                msg: "unexpected".into(),
            }),
        })
        .collect::<Result<(), Error>>()?;
    Ok(None)
}

fn readln(
    ctx: &mut dyn BuiltinCtx<Value = Value>,
    args: &[&BuiltinInput],
) -> Result<Option<Value>, Error> {
    args.iter()
        .map(|v| {
            let mut s = String::new();
            stdin()
                .read_line(&mut s)
                .expect("did not enter correct string");
            ctx.write(v, Value::String(s))
        })
        .collect::<Result<(), Error>>()?;
    Ok(None)
}

impl SymbolTable {
    pub fn with_builtins(
        types: &mut NodePool<TypeSymbolRef, TypeSymbol>,
        vars: &mut NodePool<VarSymbolRef, VarSymbol>,
        callables: &mut NodePool<CallableSymbolRef, CallableSymbol>,
    ) -> Self {
        let mut st = Self::new(0, "builtin", None);
        let writeln = callables.alloc(CallableSymbol {
            name: "writeln".into(),
            return_type: None,
            params: vec![(
                vars.alloc(VarSymbol::Var {
                    name: "val".into(),
                    type_symbol: types.alloc(TypeSymbol::Any),
                }),
                ParamMode::Var,
            )],
            body: CallableBody::Func(writeln),
        });
        st.define_callable("writeln", writeln);
        let readln = callables.alloc(CallableSymbol {
            name: "readln".into(),
            return_type: None,
            params: vec![(
                vars.alloc(VarSymbol::Var {
                    name: "val".into(),
                    type_symbol: types.alloc(TypeSymbol::String),
                }),
                ParamMode::Ref,
            )],
            body: CallableBody::Func(readln),
        });
        st.define_callable("readln", readln);
        st
    }
}
