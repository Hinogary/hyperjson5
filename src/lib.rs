use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

mod error;
use error::*;

use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::{
    types::{PyAny, PyDict, PyFloat, PyList, PyTuple},
    wrap_pyfunction,
};

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{self, Serialize, SerializeMap, SerializeSeq, Serializer};

#[pyfunction]
pub fn load(py: Python, fp: PyObject, kwargs: Option<&PyDict>) -> PyResult<PyObject> {
    // Reset file pointer to beginning See
    // https://github.com/PyO3/pyo3/issues/143 Note that we ignore the return
    // value, because `seek` does not strictly need to exist on the object
    let _success = fp.call_method(py, "seek", (0,), None);

    let s_obj = fp.call_method0(py, "read")?;
    loads(
        py,
        s_obj.to_object(py),
        None,
        None,
        None,
        None,
        None,
        kwargs,
    )
}

// This function is a poor man's implementation of
// impl From<&str> for PyResult<PyObject>, which is not possible,
// because we have none of these types under our control.
// Note: Encoding param is deprecated and ignored.
#[pyfunction]
pub fn loads(
    py: Python,
    s: PyObject,
    encoding: Option<PyObject>,
    cls: Option<PyObject>,
    object_hook: Option<PyObject>,
    parse_float: Option<PyObject>,
    parse_int: Option<PyObject>,
    kwargs: Option<&PyDict>,
) -> PyResult<PyObject> {
    // if let Some(kwargs) = kwargs {
    //     for (key, val) in kwargs.iter() {
    //         println!("{} = {}", key, val);
    //     }
    // }

    // if args.len() == 0 {
    //     // TODO: This is the wrong error message.
    //     return Err(PyLookupError::py_err("oh no"));
    // }
    // if args.len() >= 2 {
    //     // return Err(PyTypeError::py_err(format!(
    //     //     "Unknown encoding: {}",
    //     //     args.get_item(1).to_string()
    //     // )));
    //     return Err(PyLookupError::py_err(
    //         "loads() takes exactly 1 argument (2 given)",
    //     ));
    // }
    // let s = args.get_item(0).to_string();

    // This was moved out of the Python module code to enable benchmarking.
    loads_impl(
        py,
        s,
        encoding,
        cls,
        object_hook,
        parse_float,
        parse_int,
        kwargs,
    )
}

#[pyfunction]
// ensure_ascii, check_circular, allow_nan, cls, indent, separators, default, sort_keys, kwargs = "**")]
#[allow(unused_variables)]
pub fn dumps(
    py: Python,
    obj: PyObject,
    _skipkeys: Option<PyObject>,
    ensure_ascii: Option<PyObject>,
    _check_circular: Option<PyObject>,
    _allow_nan: Option<PyObject>,
    _cls: Option<PyObject>,
    indent: Option<PyObject>,
    _separators: Option<PyObject>,
    _default: Option<PyObject>,
    sort_keys: Option<PyObject>,
    _kwargs: Option<&PyDict>,
) -> PyResult<PyObject> {
    let indent_data: Option<Vec<u8>> = if let Some(indent_py) = indent {
        let indent_number = indent_py.extract(py)?;
        Some(vec![b' '; indent_number])
    } else {
        None
    };

    let v = SerializePyObject {
        py,
        obj: obj.extract(py)?,
        sort_keys: match sort_keys {
            Some(sort_keys) => sort_keys.is_true(py)?,
            None => false,
        },
    };

    let s: Result<String, HyperJsonError> = if let Some(indent) = indent_data {
        let buf = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent);
        let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
        v.serialize(&mut ser)
            .map_err(|error| HyperJsonError::InvalidConversion { error })?;
        String::from_utf8(ser.into_inner()).map_err(|error| HyperJsonError::Utf8Error { error })
    } else {
        json5::to_string(&v).map_err(|error| HyperJsonError::InvalidConversion5 { error })
    };

    Ok(s?.to_object(py))
}

#[pyfunction]
pub fn dump(
    py: Python,
    obj: PyObject,
    fp: PyObject,
    skipkeys: Option<PyObject>,
    ensure_ascii: Option<PyObject>,
    check_circular: Option<PyObject>,
    allow_nan: Option<PyObject>,
    cls: Option<PyObject>,
    indent: Option<PyObject>,
    separators: Option<PyObject>,
    default: Option<PyObject>,
    sort_keys: Option<PyObject>,
    kwargs: Option<&PyDict>,
) -> PyResult<PyObject> {
    let s = dumps(
        py,
        obj,
        skipkeys,
        ensure_ascii,
        check_circular,
        allow_nan,
        cls,
        indent,
        separators,
        default,
        sort_keys,
        kwargs,
    )?;
    let fp_ref: &PyAny = fp.extract(py)?;
    fp_ref.call_method1("write", (s,))?;
    // TODO: Will this always return None?
    Ok(pyo3::Python::None(py))
}

/// A hyper-fast JSON encoder/decoder written in Rust
#[pymodule]
fn hyperjson5(_py: Python, m: &PyModule) -> PyResult<()> {
    // See https://github.com/PyO3/pyo3/issues/171
    // Use JSONDecodeError from stdlib until issue is resolved.
    // py_exception!(_hyperjson, JSONDecodeError);
    // m.add("JSONDecodeError", py.get_type::<JSONDecodeError>());

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    m.add_wrapped(wrap_pyfunction!(load))?;
    m.add_wrapped(wrap_pyfunction!(loads))?;
    m.add_wrapped(wrap_pyfunction!(dump))?;
    m.add_wrapped(wrap_pyfunction!(dumps))?;

    Ok(())
}

pub fn loads_impl(
    py: Python,
    s: PyObject,
    _encoding: Option<PyObject>,
    _cls: Option<PyObject>,
    _object_hook: Option<PyObject>,
    parse_float: Option<PyObject>,
    parse_int: Option<PyObject>,
    _kwargs: Option<&PyDict>,
) -> PyResult<PyObject> {
    let string_result: Result<String, _> = s.extract(py);
    match string_result {
        Ok(string) => {
            let mut deserializer = json5::Deserializer::from_str(&string).map_err(|e|PyValueError::new_err(format!("Error with parser: {:?}", e)))?;
            let seed = HyperJsonValue::new(py, &parse_float, &parse_int);
            match seed.deserialize(&mut deserializer) {
                Ok(py_object) => {
                    Ok(py_object)
                }
                Err(e) => {
                    return convert_special_floats(py, &string, &parse_int).or_else(|_| {
                        return Err(PyValueError::new_err(format!(
                            "Value: {:?}, Error: {:?}",
                            s, e
                        )));
                    })
                }
            }
        }
        _ => {Err(PyTypeError::new_err("Couldn't convert string"))}
    }
}

struct SerializePyObject<'p, 'a> {
    py: Python<'p>,
    obj: &'a PyAny,
    sort_keys: bool,
}

impl<'p, 'a> Serialize for SerializePyObject<'p, 'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        macro_rules! cast {
            ($f:expr) => {
                if let Ok(val) = PyTryFrom::try_from(self.obj) {
                    return $f(val);
                }
            };
        }

        macro_rules! extract {
            ($t:ty) => {
                if let Ok(val) = <$t as FromPyObject>::extract(self.obj) {
                    return val.serialize(serializer);
                }
            };
        }

        cast!(|x: &PyDict| {
            if self.sort_keys {
                // TODO: this could be implemented more efficiently by building
                // a `Vec<Cow<str>, &PyAny>` of the map entries, sorting
                // by key, and serializing as in the `else` branch. That avoids
                // buffering every map value into a serde_json::Value.
                let no_sort_keys = SerializePyObject {
                    py: self.py,
                    obj: self.obj,
                    sort_keys: false,
                };
                let jv = serde_json::to_value(no_sort_keys).map_err(ser::Error::custom)?;
                jv.serialize(serializer)
            } else {
                let mut map = serializer.serialize_map(Some(x.len()))?;
                for (key, value) in x {
                    if key.is_none() {
                        map.serialize_key("null")?;
                    } else if let Ok(key) = key.extract::<bool>() {
                        map.serialize_key(if key { "true" } else { "false" })?;
                    } else if let Ok(key) = key.str() {
                        let key = key.to_string();
                        map.serialize_key(&key)?;
                    } else {
                        return Err(ser::Error::custom(format_args!(
                            "Dictionary key is not a string: {:?}",
                            key
                        )));
                    }
                    map.serialize_value(&SerializePyObject {
                        py: self.py,
                        obj: value,
                        sort_keys: self.sort_keys,
                    })?;
                }
                map.end()
            }
        });

        cast!(|x: &PyList| {
            let mut seq = serializer.serialize_seq(Some(x.len()))?;
            for element in x {
                seq.serialize_element(&SerializePyObject {
                    py: self.py,
                    obj: element,
                    sort_keys: self.sort_keys,
                })?
            }
            seq.end()
        });
        cast!(|x: &PyTuple| {
            let mut seq = serializer.serialize_seq(Some(x.len()))?;
            for element in x {
                seq.serialize_element(&SerializePyObject {
                    py: self.py,
                    obj: element,
                    sort_keys: self.sort_keys,
                })?
            }
            seq.end()
        });

        extract!(String);
        extract!(bool);

        cast!(|x: &PyFloat| x.value().serialize(serializer));
        extract!(u64);
        extract!(i64);

        if self.obj.is_none() {
            return serializer.serialize_unit();
        }

        match self.obj.repr() {
            Ok(repr) => Err(ser::Error::custom(format_args!(
                "Value is not JSON serializable: {}",
                repr,
            ))),
            Err(_) => Err(ser::Error::custom(format_args!(
                "Type is not JSON serializable: {}",
                self.obj.get_type().name().unwrap().to_string(),
            ))),
        }
    }
}

fn convert_special_floats(
    py: Python,
    s: &str,
    _parse_int: &Option<PyObject>,
) -> PyResult<PyObject> {
    match s {
        // TODO: If `allow_nan` is false (default: True), then this should be a ValueError
        // https://docs.python.org/3/library/json.html
        "NaN" => Ok(std::f64::NAN.to_object(py)),
        "Infinity" => Ok(std::f64::INFINITY.to_object(py)),
        "-Infinity" => Ok(std::f64::NEG_INFINITY.to_object(py)),
        _ => Err(PyValueError::new_err(format!("Value: {:?}", s))),
    }
}

#[derive(Copy, Clone)]
struct HyperJsonValue<'a> {
    py: Python<'a>,
    parse_float: &'a Option<PyObject>,
    parse_int: &'a Option<PyObject>,
}

impl<'a> HyperJsonValue<'a> {
    fn new(
        py: Python<'a>,
        parse_float: &'a Option<PyObject>,
        parse_int: &'a Option<PyObject>,
    ) -> HyperJsonValue<'a> {
        // We cannot borrow the runtime here,
        // because it wouldn't live long enough
        // let gil = Python::acquire_gil();
        // let py = gil.python();
        HyperJsonValue {
            py,
            parse_float,
            parse_int,
        }
    }
}

impl<'de, 'a> DeserializeSeed<'de> for HyperJsonValue<'a> {
    type Value = PyObject;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'a> HyperJsonValue<'a> {
    fn parse_primitive<E, T>(self, value: T, parser: &PyObject) -> Result<PyObject, E>
    where
        E: de::Error,
        T: ToString,
    {
        match parser.call1(self.py, (value.to_string(),)) {
            Ok(primitive) => Ok(primitive),
            Err(err) => Err(de::Error::custom(HyperJsonError::from(err))),
        }
    }
}

impl<'de, 'a> Visitor<'de> for HyperJsonValue<'a> {
    type Value = PyObject;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any valid JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value.to_object(self.py))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match self.parse_int {
            Some(parser) => self.parse_primitive(value, parser),
            None => Ok(value.to_object(self.py)),
        }
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match self.parse_int {
            Some(parser) => self.parse_primitive(value, parser),
            None => Ok(value.to_object(self.py)),
        }
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match self.parse_float {
            Some(parser) => self.parse_primitive(value, parser),
            None => Ok(value.to_object(self.py)),
        }
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value.to_object(self.py))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(self.py.None())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut elements = Vec::new();

        while let Some(elem) = seq.next_element_seed(self)? {
            elements.push(elem);
        }

        Ok(elements.to_object(self.py))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = BTreeMap::new();

        while let Some((key, value)) = map.next_entry_seed(PhantomData::<String>, self)? {
            entries.insert(key, value);
        }

        Ok(entries.to_object(self.py))
    }
}
