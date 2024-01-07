use std::io;

fn main() -> io::Result<()> {
    ircat::ircat(io::stdin().lock(), &mut io::stdout())?;
    Ok(())
}
