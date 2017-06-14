// Copyright (c) 2017-present PyO3 Project and Contributors

use syn;
use quote::Tokens;

use args;
use method;
use py_method;
use utils;


pub fn build_py3_module_init(ast: &mut syn::Item, attr: String) -> Tokens {
    let modname = &attr.to_string()[1..attr.to_string().len()-1].to_string();

    match ast.node {
        syn::ItemKind::Fn(_, _, _, _, _, ref mut block) => {
            let mut stmts = Vec::new();
            for stmt in block.stmts.iter_mut() {
                match stmt {
                    &mut syn::Stmt::Item(ref mut item) => {
                        if let Some(block) = wrap_fn(item) {
                            for stmt in block.stmts.iter() {
                                stmts.push(stmt.clone());
                            }
                            continue
                        }
                    }
                    _ => (),
                }
                stmts.push(stmt.clone());
            }
            block.stmts = stmts;

            py3_init(&ast.ident, &modname)
        },
        _ => panic!("#[modinit] can only be used with fn block"),
    }
}

pub fn py3_init(fnname: &syn::Ident, name: &String) -> Tokens {
    let cb_name = syn::Ident::from(format!("PyInit_{}", name.trim()).as_ref());
    quote! {
        #[no_mangle]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn #cb_name() -> *mut ::pyo3::ffi::PyObject {
            use std;
            extern crate pyo3 as _pyo3;
            use pyo3::IntoPyPointer;

            static mut MODULE_DEF: _pyo3::ffi::PyModuleDef = _pyo3::ffi::PyModuleDef_INIT;
            // We can't convert &'static str to *const c_char within a static initializer,
            // so we'll do it here in the module initialization:
            MODULE_DEF.m_name = concat!(stringify!(#cb_name), "\0").as_ptr() as *const _;

            let guard = _pyo3::callback::AbortOnDrop("py_module_init");
            let py = _pyo3::Python::assume_gil_acquired();
            _pyo3::ffi::PyEval_InitThreads();
            let module = _pyo3::ffi::PyModule_Create(&mut MODULE_DEF);
            if module.is_null() {
                std::mem::forget(guard);
                return module;
            }

            let module = match _pyo3::PyObject::from_owned_ptr(
                py, module).cast_into::<PyModule>(py)
            {
                Ok(m) => m,
                Err(e) => {
                    _pyo3::PyErr::from(e).restore(py);
                    std::mem::forget(guard);
                    return std::ptr::null_mut();
                }
            };
            let ret = match #fnname(py, &module) {
                Ok(_) => module.into_ptr(),
                Err(e) => {
                    e.restore(py);
                    std::ptr::null_mut()
                }
            };
            std::mem::forget(guard);
            ret
        }
    }
}

pub fn build_py2_module_init(ast: &mut syn::Item, attr: String) -> Tokens {
    let modname = &attr.to_string()[1..attr.to_string().len()-1].to_string();

    match ast.node {
        syn::ItemKind::Fn(_, _, _, _, _, ref mut block) => {
            let mut stmts = Vec::new();
            for stmt in block.stmts.iter_mut() {
                match stmt {
                    &mut syn::Stmt::Item(ref mut item) => {
                        if let Some(block) = wrap_fn(item) {
                            for stmt in block.stmts.iter() {
                                stmts.push(stmt.clone());
                            }
                            continue
                        }
                    }
                    _ => (),
                }
                stmts.push(stmt.clone());
            }
            block.stmts = stmts;

            py2_init(&ast.ident, &modname)
        },
        _ => panic!("#[modinit] can only be used with fn block"),
    }
}

pub fn py2_init(fnname: &syn::Ident, name: &String) -> Tokens {
    let cb_name = syn::Ident::from(format!("init{}", name.trim()).as_ref());

    quote! {
        #[no_mangle]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn #cb_name() {
            extern crate pyo3 as _pyo3;
            use std;

            let name = concat!(stringify!(#cb_name), "\0").as_ptr() as *const _;
            let guard = _pyo3::callback::AbortOnDrop("py_module_initializer");
            let py = pyo3::Python::assume_gil_acquired();
            pyo3::ffi::PyEval_InitThreads();
            let module = pyo3::ffi::Py_InitModule(name, std::ptr::null_mut());
            if module.is_null() {
                std::mem::forget(guard);
                return
            }

            let module = match pyo3::PyObject::from_borrowed_ptr(
                py, module).cast_into::<pyo3::PyModule>(py)
            {
                Ok(m) => m,
                Err(e) => {
                    _pyo3::PyErr::from(e).restore(py);
                    std::mem::forget(guard);
                    return
                }
            };
            let ret = match #fnname(py, &module) {
                Ok(()) => (),
                Err(e) => e.restore(py)
            };
            std::mem::forget(guard);
            ret
        }
    }
}

fn wrap_fn(item: &mut syn::Item) -> Option<Box<syn::Block>> {
    let name = item.ident.clone();
    let mut new_attrs = Vec::new();
    let mut fnname = None;
    let mut modname = None;
    let mut fn_attrs = Vec::new();

    for attr in item.attrs.iter() {
        match attr.value {
            syn::MetaItem::List(ref name, ref meta) => {
                match name.as_ref() {
                    "pyfn" => {
                        if meta.len() >= 2 {
                            match meta[0] {
                                syn::NestedMetaItem::MetaItem(syn::MetaItem::Word(ref ident)) => {
                                    modname = Some(ident.clone());
                                }
                                _ => modname = None
                            }
                            match meta[1] {
                                syn::NestedMetaItem::Literal(syn::Lit::Str(ref s, _)) => {
                                    fnname = Some(syn::Ident::from(s.as_str()));
                                }
                                _ => fnname = None
                            }
                        } else {
                            println!("can not parse 'pyfn' params {:?}", attr);
                            modname = None
                        }
                        if meta.len() >= 3 {
                            match meta[2] {
                                syn::NestedMetaItem::Literal(syn::Lit::Str(ref s, _)) => {
                                    fn_attrs = args::parse_arguments(s.as_ref());
                                },
                                _ => modname = None
                            }
                        }
                        continue;
                    }
                    _ => (),
                }
            }
            _ => (),
        };
        new_attrs.push(attr.clone())
    }
    item.attrs.clear();
    item.attrs.extend(new_attrs);

    if let None = fnname {
        return None
    }
    if let None = modname {
        return None
    }

    match item.node {
        syn::ItemKind::Fn(ref decl, _, _, _, _, _) => {
            let mut py = false;
            let mut arguments = Vec::new();

            for input in decl.inputs.iter() {
                match input {
                    &syn::FnArg::SelfRef(_, _) | &syn::FnArg::SelfValue(_) => (),
                    &syn::FnArg::Captured(ref pat, ref ty) => {
                        let (mode, ident) = match pat {
                            &syn::Pat::Ident(ref mode, ref ident, _) =>
                                (mode, ident),
                            _ =>
                                panic!("unsupported argument: {:?}", pat),
                        };

                        if !py {
                            match ty {
                                &syn::Ty::Path(_, ref path) =>
                                    if let Some(segment) = path.segments.last() {
                                        if segment.ident.as_ref() == "Python" {
                                            py = true;
                                            continue;
                                        }
                                    },
                                _ => (),
                            }
                        }

                        let opt = method::check_arg_ty_and_optional(&name, ty);
                        arguments.push(method::FnArg {name: ident,
                                                      mode: mode,
                                                      ty: ty,
                                                      optional: opt});
                    }
                    &syn::FnArg::Ignored(_) =>
                        panic!("ignored argument: {:?}", name),
                }
            }

            let ty = match decl.output {
                syn::FunctionRetTy::Default => syn::Ty::Infer,
                syn::FunctionRetTy::Ty(ref ty) => ty.clone()
            };

            let spec = method::FnSpec {
                tp: method::FnType::Fn,
                attrs: fn_attrs,
                args: arguments,
                output: ty,
            };

            let m = modname.unwrap();
            let fnname = fnname.unwrap();
            let wrapper = impl_wrap(&name, &spec);
            let item2 = item.clone();
            let doc = utils::get_doc(&item.attrs);

            let tokens = quote! {
                fn test() {
                    #item2

                    {
                        use std;
                        use pyo3 as _pyo3;

                        #wrapper

                        let def = pyo3::class::PyMethodDef {
                            ml_name: stringify!(#fnname),
                            ml_meth: pyo3::class::PyMethodType::PyCFunctionWithKeywords(wrap),
                            ml_flags: pyo3::ffi::METH_VARARGS | pyo3::ffi::METH_KEYWORDS,
                            ml_doc: #doc,
                        }.as_method_def();

                        unsafe {
                            let func = PyObject::from_owned_ptr_or_panic(
                                py, pyo3::ffi::PyCFunction_New(
                                    &def as *const _ as *mut _, std::ptr::null_mut()));
                            std::mem::forget(def);

                            #m.add(py, stringify!(#fnname), func)?
                        }
                    }
                }
            }.to_string();

            let item = syn::parse_item(tokens.as_str()).unwrap();
            match item.node {
                syn::ItemKind::Fn(_, _, _, _, _, ref block) => {
                    return Some(block.clone())
                },
                _ => ()
            }
        },
        _ => (),
    }

    None
}


/// Generate static method wrapper (PyCFunction, PyCFunctionWithKeywords)
pub fn impl_wrap(name: &syn::Ident, spec: &method::FnSpec) -> Tokens {
    let names: Vec<&syn::Ident> = spec.args.iter().map(|item| item.name).collect();
    let cb = quote! {{
        #name(py, #(#names),*)
    }};

    let body = py_method::impl_arg_params(spec, cb);
    let output = &spec.output;

    quote! {
        #[allow(unused_mut)]
        unsafe extern "C" fn wrap(_slf: *mut _pyo3::ffi::PyObject,
                                  args: *mut _pyo3::ffi::PyObject,
                                  kwargs: *mut _pyo3::ffi::PyObject) -> *mut _pyo3::ffi::PyObject
        {
            const LOCATION: &'static str = concat!(stringify!(#name), "()");

            _pyo3::callback::cb_meth(LOCATION, |py| {
                let args = _pyo3::PyTuple::from_borrowed_ptr(py, args);
                let kwargs = _pyo3::argparse::get_kwargs(py, kwargs);

                let result: #output = {
                    #body
                };
                _pyo3::callback::cb_convert(
                    _pyo3::callback::PyObjectCallbackConverter, py, result)
            })
        }
    }
}