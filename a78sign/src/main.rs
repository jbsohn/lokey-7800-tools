use a78sign::{Cartridge, Signature, SignatureStatus, SIGNATURE_LEN};
use clap::Parser;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "a78sign",
    version,
    about = "Atari 7800 cartridge digital signature tool",
    long_about = "Generates and verifies the digital signature that the Atari 7800 BIOS checks \
                  on cartridges.\n\n\
                  A Rust port of 7800sign (sign7800.c) by Bruce Tomlin, from the 7800basic project.\n\n\
                  The image must be a raw ROM binary whose size is a multiple of 4 KB \
                  with $FF at $FFF8 and the hash start page in the high nibble of $FFF9. The 120-byte \
                  signature lives at $FF80 and is written over the existing bytes there."
)]
struct Cli {
    /// Only test the existing signature; exits 0 if valid, 1 otherwise
    #[arg(short = 't', long = "test", conflicts_with = "write")]
    test: bool,

    /// Write the newly generated signature back into the image
    #[arg(short = 'w', long = "write")]
    write: bool,

    /// Force writing signature even if the image already has a valid signature
    #[arg(short = 'f', long = "force", requires = "write")]
    force: bool,

    /// Raw cartridge ROM binary (.bin, .rom)
    image: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    run_sign(&cli).unwrap_or_else(|err| {
        eprintln!("Error: {err}");
        ExitCode::FAILURE
    })
}

fn run_sign(cli: &Cli) -> Result<ExitCode, String> {
    let name = cli.image.display();

    let image = fs::read(&cli.image).map_err(|_| format!("Can't open '{name}'!"))?;
    let cart = Cartridge::load(&image).map_err(|e| e.to_string())?;
    println!("Read ${:04X} bytes of cartridge data.", cart.loaded_len());

    cart.validate().map_err(|e| e.to_string())?;
    println!(
        "Cartridge hash area is from ${:04X} to $FFFF.",
        cart.hash_start()
    );

    let status = cart.status();
    match status {
        SignatureStatus::Valid => println!("Cartridge signature for '{name}' is valid!"),
        SignatureStatus::Empty => println!("Cartridge signature for '{name}' appears to be empty."),
        SignatureStatus::Invalid => println!("Cartridge signature for '{name}' is not valid."),
    }

    if cli.test {
        return Ok(if status == SignatureStatus::Valid {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }

    print!("Encrypting...");
    let result = cart.sign(|attempt| {
        print!(" {attempt:02X}");
        let _ = io::stdout().flush();
    });
    let signature = match result {
        Ok(sig) => {
            println!(" success!");
            sig
        }
        Err(e) => {
            println!(" failed!");
            return Err(e.to_string());
        }
    };

    println!("\nA valid cartridge signature is:");
    print!("{signature}");

    if cli.write {
        if status == SignatureStatus::Valid && !cli.force {
            println!("Cartridge already has a valid signature, not re-writing file (use --force to overwrite).");
        } else {
            write_signature(&cli.image, &signature)?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// Overwrite the signature area of the image at `path` in place.
fn write_signature(path: &Path, signature: &Signature) -> Result<(), String> {
    let name = path.display();
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| format!("Can't open '{name}' for writing!"))?;
    signature
        .write_to(&mut file)
        .map_err(|e| format!("Cannot write '{name}': {e}"))?;
    println!("Wrote back {SIGNATURE_LEN} bytes to '{name}'.");
    Ok(())
}
