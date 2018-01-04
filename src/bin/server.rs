extern crate bytes;
extern crate elapsed;
extern crate feel_it_still;
extern crate futures;
extern crate futures_cpupool;
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
use std::time::Duration;

use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::Framed;
use tokio_proto::pipeline::ServerProto;
use tokio_service::Service;
use futures::Future;
use futures_cpupool::CpuPool;
use tokio_proto::TcpServer;
use elapsed::measure_time;

use feel_it_still::UintCodec;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "feel-it-steel client", about = "Test client.")]
struct Opt {
    #[structopt(long = "addr", help = "Addr", default_value = "0.0.0.0:12345")]
    addr: String,


    #[structopt(short = "q", long = "quite", help = "Dont print to stdout")]
    quite: bool,

    #[structopt(short = "v", long = "verbose", help = "Verbosity", default_value = "2")]
    verbosity: usize,

    #[structopt(short = "t", long = "timeout", help = "Timeout in ms", default_value = "50")]
    timeout: usize,

    #[structopt(short = "n", long = "threads", help = "Number of cpu pool thread. Defaults to num of host cpus.")]
    threads_num: Option<usize>,
}

pub struct UintProto;

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for UintProto {
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

pub struct Server {
    thread_pool: CpuPool,
    timeout: Duration,
}

impl Service for Server {
    // These types must match the corresponding protocol types:
    type Request = u64;
    type Response = u64;

    // For non-streaming protocols, service errors are always io::Error
    type Error = io::Error;

    // The future for computing the response; box it for simplicity.
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    // Produce a future for computing a response from a request.
    fn call(&self, req: Self::Request) -> Self::Future {
        info!("Got {}", req);

        let count = self.thread_pool.spawn_fn(move || {
            let (elapsed, res) = measure_time(|| (0..req).count());
            info!("Finished {} after {}ms", res, elapsed.millis());
            Ok(elapsed.millis())
        });

        let timeout = tokio_timer::wheel()
            .tick_duration(Duration::from_millis(10)) // hardcoded for great good
            .build()
            .timeout(count, self.timeout)
            .map_err(move |err| {
                info!("Timed out {}", req);
                err
            });

        return Box::new(timeout);
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

    let pool = match opt.threads_num {
        Some(num) => CpuPool::new(num),
        None => CpuPool::new_num_cpus(),
    };

    let addr = opt.addr.parse().expect("Invalid addr");
    let server = TcpServer::new(UintProto, addr);

    server.serve(move || {
        Ok(Server {
            thread_pool: pool.clone(),
            timeout: Duration::from_millis(opt.timeout as u64),
        })
    });
}
