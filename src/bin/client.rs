#![feature(conservative_impl_trait)]

extern crate feel_it_still;
extern crate futures;
extern crate rand;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_timer;

extern crate structopt;
#[macro_use]
extern crate structopt_derive;

#[macro_use]
extern crate log;
extern crate stderrlog;

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures::Future;
use futures::future;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::Framed;
use tokio_core::reactor::{Core, Handle};
use tokio_proto::pipeline::ClientProto;
use tokio_proto::TcpClient;
use tokio_service::Service;

use rand::Rng;

use structopt::StructOpt;

use feel_it_still::UintCodec;

#[derive(StructOpt, Debug)]
#[structopt(name = "feel-it-steel client", about = "Test client.")]
struct Opt {
    #[structopt(long = "addr", help = "Addr", default_value = "127.0.0.1:12345")]
    addr: String,

    #[structopt(short = "s", long = "sync", help = "Make sync requests")]
    sync: bool,

    #[structopt(short = "q", long = "quite", help = "Dont print to stdout")]
    quite: bool,

    #[structopt(short = "v", long = "verbose", help = "Verbosity", default_value = "2")]
    verbosity: usize,

    #[structopt(short = "t", long = "timeout", help = "Timeout in ms", default_value = "5000")]
    timeout: usize,

    #[structopt(long = "no-random", help = "Don't generate random values")]
    no_random: bool,

    #[structopt(short = "n", long = "number", help = "Number or requests", default_value = "10")]
    number: usize,

    #[structopt(long = "min", help = "Min number for generation", default_value = "100000")]
    min: usize,

    #[structopt(long = "max", help = "Max number for generation", default_value = "1000000")]
    max: usize,
}

pub struct UintProto;

impl<T: AsyncRead + AsyncWrite + 'static> ClientProto<T> for UintProto {
    // For this protocol style, `Request` matches the `Item` type of the codec's `Decoder`
    type Request = u64;

    // For this protocol style, `Response` matches the `Item` type of the codec's `Encoder`
    type Response = u64;

    // A bit of boilerplate to hook in the codec:
    type Transport = Framed<T, UintCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(UintCodec::new()))
    }
}

struct Range {
    min: u64,
    max: u64,
    n: usize,
}

impl Range {
    fn gen_random(&self) -> Vec<u64> {
        let mut rng = rand::thread_rng();
        (0..self.n)
            .into_iter()
            .map(|_| rng.gen_range(self.min, self.max))
            .collect()
    }

    fn gen_sequential(&self) -> Vec<u64> {
        (self.min..self.max).into_iter().take(self.n).collect()
    }
}

struct Client {
    addr: SocketAddr,
    requests: Vec<u64>,
    timeout: Duration,
}

impl Client {
    fn run_async(&self) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let clients = self.requests
            .iter()
            .map(|&req| self.make_tcp_client(&handle, req));
        let future = future::join_all(clients);

        core.run(future).unwrap();
    }

    fn run_sync(&self) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        for &req in &self.requests {
            let client = self.make_tcp_client(&handle, req);
            core.run(client).unwrap();
        }
    }

    fn make_tcp_client(
        &self,
        handle: &Handle,
        req: u64,
    ) -> impl Future<Item = u64, Error = io::Error> {
        // see https://github.com/tokio-rs/tokio-proto/blob/master/src/streaming/pipeline/client.rs#L75
        let connection = TcpClient::new(UintProto).connect(&self.addr, handle);

        tokio_timer::wheel()
            .tick_duration(Duration::from_millis(10))
            .build()
            .timeout(connection, self.timeout)
            .map_err(move |err| {
                info!("Client timeout {}", req);
                err
            })
            .and_then(move |client_service| client_service.call(req))
            .inspect(|res| info!("Got {}", res))
            .or_else(|_| Ok(0))
    }
}

fn main() {
    let opt = Opt::from_args();
    stderrlog::new()
        .verbosity(opt.verbosity)
        .quiet(opt.quite)
        .init()
        .unwrap();
    debug!("{:?}", opt);

    let addr = opt.addr.parse().expect("Invalid addr");
    let range = Range {
        min: opt.min as u64,
        max: opt.max as u64,
        n: opt.number,
    };

    let requests = if opt.no_random {
        range.gen_sequential()
    } else {
        range.gen_random()
    };

    let client = Client {
        addr,
        requests,
        timeout: Duration::from_millis(opt.timeout as u64),
    };

    if opt.sync {
        client.run_sync();
    } else {
        client.run_async();
    }
}
