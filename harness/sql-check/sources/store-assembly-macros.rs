pub fn assemble(out: &mut String, name: &str) -> core::fmt::Result {
    let joined = concat!("id", ", name");
    write!(out, "{joined}")?;
    writeln!(out, "{name}")?;
    out.write_fmt(format_args!("{name}"))
}
