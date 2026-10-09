//! Project automation: `cargo xtask <command>`. See `docs/PLAN.md`.

mod cpp;
mod cpu_probe;
mod gen_test_fonts;
mod inventory;
mod oracle;
mod publish;
mod rp_diff;
mod skia;
mod sksl;
mod sksl_lexer;
mod verify;
mod verify_benches;
mod verify_gms;
mod verify_sksl;

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "skia-rust project automation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the pinned Skia checkout in `third_party/skia`.
    Skia {
        #[command(subcommand)]
        command: SkiaCommand,
    },
    /// Manage the test inventory (`inventory/manifest.toml`).
    Inventory {
        #[command(subcommand)]
        command: InventoryCommand,
    },
    /// Maintain the `SkSL` port's embedded data (`docs/design/sksl.md`).
    Sksl {
        #[command(subcommand)]
        command: SkslCommand,
    },
    /// Build and run the Skia oracle (see `oracle/README.md`).
    Oracle {
        #[command(subcommand)]
        command: OracleCommand,
    },
    /// Generate the portable test font data (`tests/tools/src/fonts/test_font_data`) from
    /// Skia's `tools/fonts/test_font_*.inc` (needs `cargo xtask skia fetch`).
    GenTestFonts,
    /// Print this host's CPU tiers and `rcp`/`rsqrt` estimate fingerprints
    /// (`docs/design/raster-pipeline.md` §1.4, §4.6).
    CpuProbe {
        /// Also write the host's `rcpps`/`rsqrtps` tables (`rcpps.bin`, `rsqrtps.bin`,
        /// `estimates.txt`) into this directory.
        #[arg(long, value_name = "DIR")]
        dump_tables: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum OracleCommand {
    /// Sync Skia's third-party deps (Dawn, codecs, ...) plus gn and ninja.
    Deps,
    /// Apply the oracle patches to `third_party/skia` (idempotent).
    Patch,
    /// List the CPU tiers derived from `oracle/tiers.toml`.
    Tiers,
    /// Configure and build DM for one build in `oracle/tiers.toml`.
    Build {
        /// Build name, e.g. `x64-sse2`.
        name: String,
    },
    /// Render goldens for one tier into `goldens/<commit>/<tier>/` and hash them.
    Run {
        /// Tier name, e.g. `cpu-x64-sse2-rt-ml3` (see `cargo xtask oracle tiers`).
        tier: String,
        /// DM configs (color types) for CPU tiers, e.g. `8888 f16` (default `8888`).
        #[arg(long, num_args = 1..)]
        config: Vec<String>,
        /// DM sources, e.g. `gm image`.
        #[arg(long, num_args = 1.., default_value = "gm")]
        src: Vec<String>,
        /// DM `--match` patterns (`~` excludes, `^`/`$` anchor).
        #[arg(long, num_args = 1..)]
        r#match: Vec<String>,
        /// DM worker threads.
        #[arg(long)]
        threads: Option<usize>,
        /// Replace the tier's existing goldens instead of merging into them.
        #[arg(long)]
        fresh: bool,
    },
    /// Write one golden's raw bytes to a file (for diffing).
    Extract {
        /// Tier name.
        tier: String,
        /// Result id, e.g. `8888/gm/aarectmodes`.
        id: String,
        /// Output file.
        dest: PathBuf,
    },
    /// Bundle the pin's goldens, upload them to the `goldens-<mNNN>` GitHub release,
    /// and write `inventory/goldens.lock`.
    Publish {
        /// Build the bundle and print the lock file without uploading.
        #[arg(long)]
        dry_run: bool,
    },
    /// Run DM for one GM with `SKIA_ORACLE_RP_DUMP` and print its raster-pipeline stage lists.
    RpDump {
        /// CPU tier name.
        tier: String,
        /// GM name, e.g. `aarectmodes`.
        gm: String,
        /// DM config (color type).
        #[arg(long, default_value = "8888")]
        config: String,
        /// Where to keep the raw dump (default `target/oracle-rp-dump/<tier>/<config>/<gm>.txt`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Also print the `#` lines with context values (colors, matrices, ...).
        #[arg(long)]
        ctx: bool,
    },
    /// Check that the `[[class]]` tier groups in `oracle/tiers.toml` still match the goldens.
    CheckClasses,
    /// Run the per-stage raster pipeline cases (`oracle/rp-diff`) through Skia and skia-rust
    /// and compare them byte for byte (design §4.2).
    RpDiff {
        /// Code paths to run: `scalar`, `sse2`, `sse41`, `ml3`, `ml4` (repeat or comma-separate;
        /// default all).
        #[arg(long)]
        tier: Vec<String>,
        /// Only cases whose names match this glob (`*` matches anything), e.g. `srcover/*`.
        #[arg(long)]
        case_glob: Option<String>,
        /// Store Skia's results in `oracle/rp-diff/expected/<tier>.txt`.
        #[arg(long)]
        update: bool,
        /// Only check skia-rust against the stored results (no C++; any host).
        #[arg(long)]
        replay: bool,
        /// Oracle build whose static libraries the driver links against.
        #[arg(long, default_value = "x64-sse2")]
        build: String,
        /// Rebuild the C++ driver.
        #[arg(long)]
        rebuild: bool,
        /// Mismatches to describe in detail per tier.
        #[arg(long, default_value_t = 10)]
        details: usize,
    },
    /// Compare a directory of skia-rust outputs (golden layout) against a tier's hashes.
    Compare {
        /// Tier name.
        tier: String,
        /// Directory holding `<config>/<src>/[<options>/]<name>.raw` files.
        dir: PathBuf,
    },
}

#[derive(Subcommand)]
enum SkiaCommand {
    /// Clone Skia at the pinned commit (shallow) into `third_party/skia`.
    Fetch,
}

#[derive(Subcommand)]
enum SkslCommand {
    /// Copy the built-in `SkSL` modules (original and minified) from `third_party/skia` into
    /// `crates/skia-rust-sksl/src/modules`.
    SyncModules {
        /// Compare the crate's copies with the pinned tree instead of writing them.
        #[arg(long)]
        check: bool,
    },
    /// Transcribe the lexer's DFA tables and token kinds from `SkSLLexer.{h,cpp}` into
    /// `crates/skia-rust-sksl/src/lexer`.
    GenLexer,
}

#[derive(Subcommand)]
enum InventoryCommand {
    /// Re-scan the pinned Skia tree and update the manifest, preserving hand-edited fields.
    Sync {
        /// Skip the check that `third_party/skia` is at the pinned commit.
        #[arg(long)]
        allow_pin_mismatch: bool,
    },
    /// Print pass-rate statistics from the manifest.
    Stats,
    /// Print the Rust test path a unit-test id must be ported to.
    ModulePath {
        /// Manifest id, e.g. `tests/PointTest.cpp::Point`.
        id: String,
    },
    /// Run the ported unit tests and GMs and check them against the manifest.
    Verify {
        /// Mark newly passing entries `passing` (and failing ported ones `failing`).
        #[arg(long)]
        update: bool,
    },
}

fn main() -> Result<()> {
    let root = workspace_root();
    match Cli::parse().command {
        Command::Skia {
            command: SkiaCommand::Fetch,
        } => skia::fetch(&root),
        Command::Sksl { command } => match command {
            SkslCommand::SyncModules { check } => sksl::sync_modules(&root, check),
            SkslCommand::GenLexer => sksl_lexer::gen_lexer(&root),
        },
        Command::Inventory { command } => match command {
            InventoryCommand::Sync { allow_pin_mismatch } => {
                inventory::sync(&root, allow_pin_mismatch)
            }
            InventoryCommand::Stats => inventory::stats(&root),
            InventoryCommand::ModulePath { id } => inventory::module_path(&root, &id),
            InventoryCommand::Verify { update } => inventory::verify(&root, update),
        },
        Command::Oracle { command } => match command {
            OracleCommand::Deps => oracle::deps(&root),
            OracleCommand::Patch => oracle::patch(&root),
            OracleCommand::Tiers => oracle::list_tiers(&root),
            OracleCommand::Build { name } => oracle::build(&root, &name),
            OracleCommand::Run {
                tier,
                config,
                src,
                r#match,
                threads,
                fresh,
            } => oracle::run_tier(
                &root,
                &oracle::RunOptions {
                    tier,
                    configs: config,
                    srcs: src,
                    matches: r#match,
                    threads,
                    fresh,
                },
            ),
            OracleCommand::Extract { tier, id, dest } => oracle::extract(&root, &tier, &id, &dest),
            OracleCommand::RpDump {
                tier,
                gm,
                config,
                out,
                ctx,
            } => oracle::rp_dump(&root, &tier, &gm, &config, out.as_deref(), ctx),
            OracleCommand::CheckClasses => oracle::check_classes(&root),
            OracleCommand::RpDiff {
                tier,
                case_glob,
                update,
                replay,
                build,
                rebuild,
                details,
            } => rp_diff::run(
                &root,
                &rp_diff::Options {
                    tiers: tier,
                    case_glob,
                    update,
                    replay,
                    build,
                    rebuild,
                    details,
                },
            ),
            OracleCommand::Compare { tier, dir } => oracle::compare(&root, &tier, &dir),
            OracleCommand::Publish { dry_run } => publish::publish(&root, dry_run),
        },
        Command::GenTestFonts => gen_test_fonts::run(&root),
        Command::CpuProbe { dump_tables } => cpu_probe::probe(dump_tables.as_deref()),
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}
