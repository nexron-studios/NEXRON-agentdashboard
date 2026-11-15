//! `agentd connect`: speaks the JSON-lines protocol on stdin/stdout. The
//! dashboard starts it locally as a child process and remotely over `ssh -T`.

use std::io;

fn main() {
    if std::env::args().nth(1).as_deref() != Some("connect") {
        eprintln!("usage: agentd connect");
        std::process::exit(2);
    }
    agentd::daemon::Daemon::new(io::stdout()).serve(io::stdin().lock());
}
