//! Contains the different types of values understood by XML-RPC.

use utils::{escape_xml, format_datetime};

use base64::encode;
use iso8601::DateTime;

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::borrow::{Cow, ToOwned};

/// The possible XML-RPC values.
///
/// This enum allows both borrowed data (of lifetime `'a`) and owned data
#[derive(Debug, PartialEq, Clone)]
pub enum Value<'a> {
    /// `<i4>` or `<int>`, 32-bit signed integer.
    Int(i32),

    /// `<i8>`, 64-bit signed integer.
    ///
    /// This is a non-standard feature that may not be supported on all servers or clients.
    Int64(i64),

    /// `<boolean>`, 0 == `false`, 1 == `true`.
    Bool(bool),

    /// `<string>`, a string of bytes.
    ///
    /// According the the [specification][spec], "A string can be used to encode binary data", so
    /// there is no guarantee that the contents are valid UTF-8, which is required for Rust strings.
    ///
    /// For the common case where the value is indeed valid UTF-8, the `Value::as_str` accessor can
    /// be used. Since success of that method depends on the remote machine, proper error handling
    /// is necessary.
    ///
    /// [spec]: https://web.archive.org/web/20050913062502/http://www.xmlrpc.com/spec
    String(Cow<'a, [u8]>),

    /// `<double>`
    Double(f64),

    /// `<dateTime.iso8601>`, an ISO 8601 formatted date/time value.
    DateTime(DateTime),

    /// `<base64>`, base64-encoded binary data.
    Base64(Cow<'a, [u8]>),

    /// `<struct>`, a mapping of named values.
    ///
    /// Note that XML-RPC [doesn't require][dup] the keys inside a `<struct>` to be
    /// unique. However, most implementations will all but one of the duplicate entries.
    ///
    /// To allow non-copy operation and since XML-RPC allows it, this just stores a list of
    /// key-value pairs.
    ///
    /// You most likely don't need the non-copy capabilities and want to make sure that no duplicate
    /// keys exist, so you're encouraged to use a `BTreeMap` or a `HashMap` and convert to a `Value`
    /// by using `Into` or `From`.
    ///
    /// [dup]: http://xml-rpc.yahoogroups.narkive.com/Br9xMUtQ/duplicate-struct-member-names-allowed
    Struct(Cow<'a, Slice<(Cow<'a, str>, Value<'a>)>>),

    /// `<array>`, a list of arbitrary (heterogeneous) values.
    Array(Cow<'a, Slice<Value<'a>>>),

    /// `<nil/>`
    ///
    /// This is a non-standard feature that may not be supported on all servers or clients.
    ///
    /// Refer to the [specification of `<nil>`][nil] for more information.
    ///
    /// [nil]: https://web.archive.org/web/20050911054235/http://ontosys.com/xml-rpc/extensions.php
    Nil,
}

struct Slice<T>([T]);

impl<T> ToOwned for Slice<T> {
    type Owned = Vec<T>;

    fn to_owned(self) -> Self::Owned {
        Vec::from(&self.0)
    }
}

impl<'a> Value<'a> {
    /// Writes this `Value` as XML.
    pub fn format<W: Write>(&self, fmt: &mut W) -> io::Result<()> {
        try!(writeln!(fmt, "<value>"));

        match *self {
            Value::Int(i) => {
                try!(writeln!(fmt, "<i4>{}</i4>", i));
            }
            Value::Int64(i) => {
                try!(writeln!(fmt, "<i8>{}</i8>", i));
            }
            Value::Bool(b) => {
                try!(writeln!(fmt, "<boolean>{}</boolean>", if b { "1" } else { "0" }));
            }
            Value::String(ref s) => {
                try!(writeln!(fmt, "<string>{}</string>", escape_xml(s)));
            }
            Value::Double(d) => {
                try!(writeln!(fmt, "<double>{}</double>", d));
            }
            Value::DateTime(date_time) => {
                try!(writeln!(fmt, "<dateTime.iso8601>{}</dateTime.iso8601>", format_datetime(&date_time)));
            }
            Value::Base64(ref data) => {
                try!(writeln!(fmt, "<base64>{}</base64>", encode(data)));
            }
            Value::Struct(ref map) => {
                try!(writeln!(fmt, "<struct>"));
                for (ref name, ref value) in map {
                    try!(writeln!(fmt, "<member>"));
                    try!(writeln!(fmt, "<name>{}</name>", escape_xml(name)));
                    try!(value.format(fmt));
                    try!(writeln!(fmt, "</member>"));
                }
                try!(writeln!(fmt, "</struct>"));
            }
            Value::Array(ref array) => {
                try!(writeln!(fmt, "<array>"));
                try!(writeln!(fmt, "<data>"));
                for value in array {
                    try!(value.format(fmt));
                }
                try!(writeln!(fmt, "</data>"));
                try!(writeln!(fmt, "</array>"));
            }
            Value::Nil => {
                try!(writeln!(fmt, "<nil/>"));
            }
        }

        try!(writeln!(fmt, "</value>"));
        Ok(())
    }
}

impl<'a> From<i32> for Value<'a> {
    fn from(other: i32) -> Self {
        Value::Int(other)
    }
}

impl<'a> From<bool> for Value<'a> {
    fn from(other: bool) -> Self {
        Value::Bool(other)
    }
}

impl<'a> From<String> for Value<'a> {
    fn from(other: String) -> Self {
        Value::String(other)
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(other: &'a str) -> Self {
        Value::String(Cow::from(other.as_slice()))
    }
}

impl<'a> From<f64> for Value<'a> {
    fn from(other: f64) -> Self {
        Value::Double(other)
    }
}

impl<'a> From<DateTime> for Value<'a> {
    fn from(other: DateTime) -> Self {
        Value::DateTime(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str;
    use std::collections::BTreeMap;

    #[test]
    fn escapes_strings() {
        let mut output: Vec<u8> = Vec::new();

        Value::from("<xml>&nbsp;string").format(&mut output).unwrap();
        assert_eq!(str::from_utf8(&output).unwrap(), "<value>\n<string>&lt;xml>&amp;nbsp;string</string>\n</value>\n");
    }

    #[test]
    fn escapes_struct_member_names() {
        let mut output: Vec<u8> = Vec::new();
        let mut map: BTreeMap<String, Value> = BTreeMap::new();
        map.insert("x&<x".to_string(), Value::from(true));

        Value::Struct(map).format(&mut output).unwrap();
        assert_eq!(str::from_utf8(&output).unwrap(), "<value>\n<struct>\n<member>\n<name>x&amp;&lt;x</name>\n<value>\n<boolean>1</boolean>\n</value>\n</member>\n</struct>\n</value>\n");
    }
}
