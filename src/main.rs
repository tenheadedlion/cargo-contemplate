use clap::{Parser, Subcommand};
use fs_extra::dir::CopyOptions;
use git2::build::{CheckoutBuilder, RepoBuilder};
use git2::{FetchOptions, Progress, RemoteCallbacks};
use phf::phf_map;
use rand::{distributions::Alphanumeric, Rng};
use std::cell::RefCell;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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

static URLS: phf::Map<&'static str, &'static str> = phf_map! {
    "phat-contract" => "https://github.com/tenheadedlion/phat-contract-starter.git",
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

#[derive(Parser)]
struct Args {
    #[clap(name = "class")]
    class: String,
}

struct Context {
    url: String,
    tmp_path: PathBuf,
    path: String,
    current_dir: PathBuf,
}

impl TryFrom<Args> for Context {
    type Error = Error;
    fn try_from(args: Args) -> Result<Self, Self::Error> {
        match URLS.get(&args.class) {
            Some(url) => Ok(Context {
                url: url.to_string(),
                tmp_path: random_path(),
                path: base_name(url)?,
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
        .clone(&ctx.url, &ctx.tmp_path)
        .map_err(|_| Error::GitFault)?;

    println!("{} ->  {}", &ctx.tmp_path.display(), &ctx.path);
    let options = CopyOptions::new();
    fs_extra::dir::copy(&ctx.tmp_path, &ctx.current_dir, &options).map_err(|e| {
        println!("{}", e);
        Error::FileSystemFault
    })?;

    std::fs::rename(Path::new(&ctx.tmp_path).file_name().unwrap(), &ctx.path).map_err(|e| {
        println!("{}", e);
        Error::FileSystemRename
    })?;
    
    std::fs::remove_dir_all(Path::join(Path::new(&ctx.path), ".git")).map_err(|e| {
        println!("{}", e);
        Error::FileSystemRemoveDir
    })?;

    println!();

    Ok(())
}

fn main() {
    let args = Args::parse();
    let context = Context::try_from(args).unwrap();
    run(&context).unwrap();
}
