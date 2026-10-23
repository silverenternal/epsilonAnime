use crate::bench::run_bench;
use crate::cli::Cli;
use crate::generated::{palette_names, variant_specs};
use crate::glyphs::Charset;
use anyhow::Result;

pub fn print_requested(cli: &Cli) -> Result<bool> {
    if cli.list_animations {
        for (name, kernel, palette, weight) in variant_specs() {
            println!("{name:<14} kernel={kernel:<14} palette={palette:<8} weight={weight:.1}");
        }
        return Ok(true);
    }
    if cli.list_palettes {
        for name in palette_names() {
            println!("{name}");
        }
        return Ok(true);
    }
    if cli.list_charsets {
        for charset in Charset::ALL {
            println!("{}", charset.name());
        }
        return Ok(true);
    }
    if let Some(frames) = cli.bench {
        run_bench(frames)?;
        return Ok(true);
    }
    Ok(false)
}
