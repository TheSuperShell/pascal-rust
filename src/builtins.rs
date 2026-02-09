use std::io::stdin;

use crate::{
    error::Error,
    interpreter::{BuiltinCtx, Value},
    symbols::{
        CallableSymbol, CallableSymbolRef, CallableType, LValue, ParamInputMode, ParamMode,
        SymbolTable, TypeSymbol, TypeSymbolRef, VarSymbol, VarSymbolRef,
    },
    utils::NodePool,
};

fn writeln(
    _: &mut dyn BuiltinCtx<Value = Value>,
    args: &[&LValue],
) -> Result<Option<Value>, Error> {
    args.iter()
        .map(|v| match v {
            LValue::Value(v) => {
                print!("{}", v.to_string());
                Ok(())
            }
            _ => Err(Error::BuiltinFunctionError {
                function_name: "writeln",
                msg: format!("expected literal value, got {:?}", v),
            }),
        })
        .collect::<Result<(), Error>>()?;
    print!("\n");
    Ok(None)
}

fn readln(
    ctx: &mut dyn BuiltinCtx<Value = Value>,
    args: &[&LValue],
) -> Result<Option<Value>, Error> {
    args.iter()
        .map(|v| {
            let mut s = String::new();
            stdin()
                .read_line(&mut s)
                .map_err(|e| Error::BuiltinFunctionError {
                    function_name: "readln",
                    msg: format!("read line error: {e}"),
                })?;
            ctx.write(v, Value::String(s));
            Ok(())
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
            param_input_mode: ParamInputMode::Repeat,
            body: CallableType::Builtin { func: writeln },
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
            param_input_mode: ParamInputMode::Seq,
            body: CallableType::Builtin { func: readln },
        });
        st.define_callable("readln", readln);
        st
    }
}
