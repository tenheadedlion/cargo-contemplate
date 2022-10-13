use clap::{Parser, Subcommand};
use fs_extra::dir::CopyOptions;
use git2::build::{CheckoutBuilder, RepoBuilder};
use git2::{FetchOptions, Progress, RemoteCallbacks};
use phf::phf_map;
use rand::{distributions::Alphanumeric, Rng};
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::{env, string};
use url::{Host, Position, Url};

fn base_name(url: &str) -> Result<String, Error> {
    let parsed_url = Url::parse(url).map_err(|_| Error::CannotParseUrl)?;
    let base = Path::new(parsed_url.path())
        .file_stem()
        .map(Ok)
        .unwrap_or_else(|| Err(Error::CannotParseUrl))?;
    Ok(base.to_str().unwrap().to_string())
}

fn random_path() -> PathBuf {
    let rnd_path: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();
    Path::join(Path::new("/tmp"), rnd_path)
}

static URLS: phf::Map<&'static str, (&'static str, &'static str, &'static str)> = phf_map! {
    "phat-contract-with-sideprog" => ("https://github.com/tenheadedlion/phat-contract-starter.git", "master", "log_server-a00c26e4ff2173713db9afca5a82aee3"),
    "phat-contract" => ("https://github.com/tenheadedlion/phat-contract-starter.git", "plain-phat-contract", "erc20-497c0f607b393edb86f8da1bf053fb06"),
};

#[derive(Debug)]
enum Error {
    NoSuchClass,
    CannotParseUrl,
    FileSystemFault,
    FileSystemRename,
    FileSystemRemoveDir,
    GitFault,
}

#[derive(Debug)]
struct Args {
    class: String,
    dest: String,
}

#[derive(Debug)]
struct Context {
    url: String,
    tmp_path: PathBuf,
    path: String,
    branch: String,
    package: String,
    current_dir: PathBuf,
}

impl TryFrom<Args> for Context {
    type Error = Error;
    fn try_from(args: Args) -> Result<Self, Self::Error> {
        match URLS.get(&args.class) {
            Some(url) => Ok(Context {
                url: url.0.to_string(),
                branch: url.1.to_string(),
                package: url.2.to_string(),
                tmp_path: random_path(),
                path: args.dest,
                current_dir: env::current_dir().map_err(|_| Error::FileSystemFault)?,
            }),
            None => Err(Error::NoSuchClass),
        }
    }
}

struct State {
    progress: Option<Progress<'static>>,
    total: usize,
    current: usize,
    path: Option<PathBuf>,
    newline: bool,
}

fn print(state: &mut State) {
    let stats = state.progress.as_ref().unwrap();
    let network_pct = (100 * stats.received_objects()) / stats.total_objects();
    let index_pct = (100 * stats.indexed_objects()) / stats.total_objects();
    let co_pct = if state.total > 0 {
        (100 * state.current) / state.total
    } else {
        0
    };
    let kbytes = stats.received_bytes() / 1024;
    if stats.received_objects() == stats.total_objects() {
        if !state.newline {
            println!();
            state.newline = true;
        }
        print!(
            "Resolving deltas {}/{}\r",
            stats.indexed_deltas(),
            stats.total_deltas()
        );
    } else {
        print!(
            "net {:3}% ({:4} kb, {:5}/{:5})  /  idx {:3}% ({:5}/{:5})  \
             /  chk {:3}% ({:4}/{:4}) {}\r",
            network_pct,
            kbytes,
            stats.received_objects(),
            stats.total_objects(),
            index_pct,
            stats.indexed_objects(),
            stats.total_objects(),
            co_pct,
            state.current,
            state.total,
            state
                .path
                .as_ref()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        )
    }
    io::stdout().flush().unwrap();
}

fn run(ctx: &Context) -> Result<(), Error> {
    let state = RefCell::new(State {
        progress: None,
        total: 0,
        current: 0,
        path: None,
        newline: false,
    });
    let mut cb = RemoteCallbacks::new();
    cb.transfer_progress(|stats| {
        let mut state = state.borrow_mut();
        state.progress = Some(stats.to_owned());
        print(&mut *state);
        true
    });

    let mut co = CheckoutBuilder::new();
    co.progress(|path, cur, total| {
        let mut state = state.borrow_mut();
        state.path = path.map(|p| p.to_path_buf());
        state.current = cur;
        state.total = total;
        print(&mut *state);
    });

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(cb);
    println!("{} -> {}", &ctx.url, &ctx.tmp_path.display());
    RepoBuilder::new()
        .fetch_options(fo)
        .with_checkout(co)
        .branch(&ctx.branch)
        .clone(&ctx.url, &ctx.tmp_path)
        .map_err(|_| Error::GitFault)?;

    println!("{} ->  {}", &ctx.tmp_path.display(), &ctx.path);
    let options = CopyOptions::new();
    fs_extra::dir::copy(
        Path::new(&ctx.tmp_path).join(&ctx.package),
        &ctx.current_dir,
        &options,
    )
    .map_err(|e| {
        println!("{}", e);
        Error::FileSystemFault
    })?;

    std::fs::rename(&ctx.package, &ctx.path).map_err(|e| {
        println!("{}", e);
        Error::FileSystemRename
    })?;

    //std::fs::remove_dir_all(Path::join(Path::new(&ctx.path), ".git")).map_err(|e| {
    //    println!("{}", e);
    //    Error::FileSystemRemoveDir
    //})?;

    Ok(())
}

fn main() {
    let cmd = clap::Command::new("cargo")
        .bin_name("cargo")
        .subcommand_required(true)
        .subcommand(
            clap::command!("contemplate")
                .arg(clap::arg!(<CLASS>).value_parser(clap::value_parser!(std::string::String)))
                .arg(clap::arg!(<DEST>).value_parser(clap::value_parser!(std::string::String))),
        );
    let matches = cmd.get_matches();
    let matches = match matches.subcommand() {
        Some(("contemplate", matches)) => matches,
        _ => unreachable!("clap should ensure we don't get here"),
    };

    let class = matches
        .get_one::<String>("CLASS")
        .map(|s| s.as_str())
        .unwrap()
        .to_string();
    let dest = matches
        .get_one::<String>("DEST")
        .map(|s| s.as_str())
        .unwrap()
        .to_string();

    let args = Args { class, dest };
    let context = Context::try_from(args).unwrap();
    run(&context).unwrap();
}
