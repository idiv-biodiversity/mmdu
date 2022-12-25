use clap_complete::Shell;

#[allow(dead_code)]
#[path = "src/cli.rs"]
mod cli;

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=src/cli.rs");

    let out_dir = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?,
    );

    let man = clap_mangen::Man::new(cli::build());
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    std::fs::write(out_dir.join("mmdu.1"), buffer)?;

    clap_complete::generate_to(
        Shell::Bash,
        &mut cli::build(),
        "mmdu",
        &out_dir,
    )?;
    clap_complete::generate_to(
        Shell::Fish,
        &mut cli::build(),
        "mmdu",
        &out_dir,
    )?;
    clap_complete::generate_to(
        Shell::Elvish,
        &mut cli::build(),
        "mmdu",
        &out_dir,
    )?;
    clap_complete::generate_to(
        Shell::Zsh,
        &mut cli::build(),
        "mmdu",
        &out_dir,
    )?;

    Ok(())
}
