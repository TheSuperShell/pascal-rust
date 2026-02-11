use crate::{
    error::Error,
    interpreter::{BuiltinCtx, Value},
    semantic_analyzer::SemanticMetadata,
    symbols::{
        BuiltinInput, CallableSymbol, CallableSymbolRef, CallableType, LValue, ParamInputMode,
        ParamMode, SymbolTable, TypeSymbol, TypeSymbolRef, VarSymbol, VarSymbolRef,
    },
    utils::NodePool,
};

fn write(
    ctx: &mut dyn BuiltinCtx<Value = Value>,
    semantic_metadata: &SemanticMetadata,
    args: BuiltinInput,
) -> Result<Option<Value>, Error> {
    args.iter()
        .map(|(v, t)| match v {
            LValue::Value(v) => {
                write!(ctx.output(), "{}", t.to_string(Some(v), semantic_metadata))?;
                Ok(())
            }
            _ => Err(Error::BuiltinFunctionError {
                function_name: "writeln",
                msg: format!("expected literal value, got {:?}", v),
            }),
        })
        .collect::<Result<(), Error>>()?;
    Ok(None)
}
fn writeln(
    ctx: &mut dyn BuiltinCtx<Value = Value>,
    semantic_metadata: &SemanticMetadata,
    args: BuiltinInput,
) -> Result<Option<Value>, Error> {
    write(ctx, semantic_metadata, args)?;
    write!(ctx.output(), "\n")?;
    Ok(None)
}

fn readln(
    ctx: &mut dyn BuiltinCtx<Value = Value>,
    _: &SemanticMetadata,
    args: BuiltinInput,
) -> Result<Option<Value>, Error> {
    args.iter()
        .map(|(v, _)| {
            let mut s = String::new();
            ctx.input()
                .read_line(&mut s)
                .map_err(|e| Error::BuiltinFunctionError {
                    function_name: "readln",
                    msg: format!("read line error: {e}"),
                })?;
            if s.ends_with('\n') {
                s.pop();
            }
            if s.ends_with('\r') {
                s.pop();
            }
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
        let write = callables.alloc(CallableSymbol {
            name: "write".into(),
            params: vec![],
            param_input_mode: ParamInputMode::Repeat,
            body: CallableType::Builtin { func: write },
            return_type: None,
        });
        st.define_callable("write", write);
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
