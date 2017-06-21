// Copyright (c) 2017-present PyO3 Project and Contributors
//
// based on Daniel Grunwald's https://github.com/dgrunwald/rust-cpython

use std;
use ffi;
use std::os::raw::c_char;
use std::ffi::{CStr, CString};

use conversion::{ToPyObject, IntoPyTuple};
use pointers::PyPtr;
use python::{Python, ToPyPointer};
use objects::{PyObject, PyDict, PyType, exc};
use objectprotocol2::ObjectProtocol2;
use token::PyObjectWithToken;
use err::{PyResult, PyErr, ToPyErr};


/// Represents a Python module object.
pub struct PyModule(PyPtr);

pyobject_nativetype2!(PyModule, PyModule_Type, PyModule_Check);


impl PyModule {
    /// Create a new module object with the `__name__` attribute set to name.
    pub fn new<'p>(py: Python<'p>, name: &str) -> PyResult<&'p PyModule> {
        let name = CString::new(name).map_err(|e| e.to_pyerr(py))?;
        unsafe {
            py.unchecked_cast_from_ptr_or_err(
                ffi::PyModule_New(name.as_ptr()))
        }
    }

    /// Import the Python module with the specified name.
    pub fn import<'p>(py: Python<'p>, name: &str) -> PyResult<&'p PyModule> {
        let name = CString::new(name).map_err(|e| e.to_pyerr(py))?;
        unsafe {
            py.unchecked_cast_from_ptr_or_err(
                ffi::PyImport_ImportModule(name.as_ptr()))
        }
    }

    /// Return the dictionary object that implements module's namespace;
    /// this object is the same as the `__dict__` attribute of the module object.
    pub fn dict(&self) -> &PyDict {
        unsafe {
            self.token().unchecked_cast_from_ptr::<PyDict>(
                ffi::PyModule_GetDict(self.as_ptr()))
        }
    }

    unsafe fn str_from_ptr<'a>(&'a self, ptr: *const c_char) -> PyResult<&'a str> {
        if ptr.is_null() {
            Err(PyErr::fetch(self.token()))
        } else {
            let slice = CStr::from_ptr(ptr).to_bytes();
            match std::str::from_utf8(slice) {
                Ok(s) => Ok(s),
                Err(e) => Err(PyErr::from_instance(
                    self.token(),
                    try!(exc::UnicodeDecodeError::new_utf8(self.token(), slice, e))))
            }
        }
    }

    /// Gets the module name.
    ///
    /// May fail if the module does not have a `__name__` attribute.
    pub fn name<'a>(&'a self) -> PyResult<&'a str> {
        unsafe { self.str_from_ptr(ffi::PyModule_GetName(self.as_ptr())) }
    }

    /// Gets the module filename.
    ///
    /// May fail if the module does not have a `__file__` attribute.
    pub fn filename<'a>(&'a self) -> PyResult<&'a str> {
        unsafe { self.str_from_ptr(ffi::PyModule_GetFilename(self.as_ptr())) }
    }

    /// Calls a function in the module.
    /// This is equivalent to the Python expression: `getattr(module, name)(*args, **kwargs)`
    pub fn call<A>(&self, name: &str, args: A, kwargs: Option<&PyDict>) -> PyResult<PyObject>
        where A: IntoPyTuple
    {
        use objectprotocol::ObjectProtocol;

        ObjectProtocol2::getattr(&self, name)?.call(self.token(), args, kwargs)
    }

    /// Gets a member from the module.
    /// This is equivalent to the Python expression: `getattr(module, name)`
    pub fn get(&self, name: &str) -> PyResult<PyObject>
    {
        self.getattr(name)
    }

    /// Adds a member to the module.
    ///
    /// This is a convenience function which can be used from the module's initialization function.
    pub fn add<V>(&self, name: &str, value: V) -> PyResult<()> where V: ToPyObject {
        self.setattr(name, value)
    }

    /// Adds a new extension type to the module.
    ///
    /// This is a convenience function that initializes the `class`,
    /// sets `new_type.__module__` to this module's name,
    /// and adds the type to this module.
    pub fn add_class<T>(&self) -> PyResult<()>
        where T: ::typeob::PyTypeInfo
    {
        let mut ty = <T as ::typeob::PyTypeInfo>::type_object();
        let type_name = <T as ::typeob::PyTypeInfo>::type_name();

        let ty = if (ty.tp_flags & ffi::Py_TPFLAGS_READY) != 0 {
            unsafe { PyType::from_type_ptr(self.token(), ty) }
        } else {
            // automatically initialize the class
            let name = self.name()?;
            let type_description = <T as ::typeob::PyTypeInfo>::type_description();

            let to = ::typeob::initialize_type::<T>(
                self.token(), Some(name), type_name, type_description, ty)
                .expect(format!("An error occurred while initializing class {}",
                                <T as ::typeob::PyTypeInfo>::type_name()).as_ref());
            self.token().release(to);
            unsafe { PyType::from_type_ptr(self.token(), ty) }
        };

        self.setattr(type_name, &ty)?;

        self.token().release(ty);
        Ok(())
    }
}
