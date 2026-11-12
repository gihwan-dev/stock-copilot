pub(crate) const STRING_TYPES: [&str; 9] = [
    "String", "str", "char", "OsString", "OsStr", "CString", "CStr", "PathBuf", "Path",
];

pub(crate) const INTEGER_TYPES: [&str; 12] = [
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
];

pub(crate) const NON_ZERO_TYPES: [&str; 13] = [
    "NonZero",
    "NonZeroI8",
    "NonZeroI16",
    "NonZeroI32",
    "NonZeroI64",
    "NonZeroI128",
    "NonZeroIsize",
    "NonZeroU8",
    "NonZeroU16",
    "NonZeroU32",
    "NonZeroU64",
    "NonZeroU128",
    "NonZeroUsize",
];

pub(crate) const FLOAT_TYPES: [&str; 2] = ["f32", "f64"];

pub(crate) const DECIMAL_TYPE: &str = "Decimal";

pub(crate) const BOOL_TYPE: &str = "bool";

pub(crate) const BYTE_VECTOR: (&str, &str) = ("Vec", "u8");

pub(crate) const CONSTRUCTOR_NAMES: [&str; 2] = ["new", "try_new"];

pub(crate) const CONSTRUCTOR_WRAPPERS: [&str; 2] = ["Result", "Option"];

pub(crate) const ACCESSOR_PREFIX: &str = "as_";

pub(crate) const STANDARD_ROOTS: [&str; 2] = ["std", "core"];

pub(crate) const TRAIT_BOOL_METHODS: [(&str, &str, &str); 6] = [
    ("cmp", "PartialEq", "eq"),
    ("cmp", "PartialEq", "ne"),
    ("cmp", "PartialOrd", "lt"),
    ("cmp", "PartialOrd", "le"),
    ("cmp", "PartialOrd", "gt"),
    ("cmp", "PartialOrd", "ge"),
];

pub(crate) fn raw_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = STRING_TYPES.to_vec();
    names.extend(INTEGER_TYPES);
    names.extend(NON_ZERO_TYPES);
    names.extend(FLOAT_TYPES);
    names.push(DECIMAL_TYPE);
    names.push(BOOL_TYPE);
    names
}
