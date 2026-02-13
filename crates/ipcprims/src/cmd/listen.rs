use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ipcprims_peer::PeerListener;

use crate::cmd::ListenArgs;
use crate::exit::{peer_error, CliError, CliResult, SUCCESS};
use crate::output::{print_frame, OutputFormat};

pub fn run(args: ListenArgs, format: OutputFormat) -> CliResult<i32> {
    let listener = PeerListener::bind(&args.path).map_err(|err| peer_error("bind failed", err))?;

    let running = Arc::new(AtomicBool::new(true));
    install_ctrlc_handler(running.clone())?;

    let mut printed = 0usize;

    while running.load(Ordering::SeqCst) {
        let mut peer = match listener.accept() {
            Ok(peer) => peer,
            Err(err) => return Err(peer_error("accept failed", err)),
        };

        while running.load(Ordering::SeqCst) {
            let frame = match peer.recv() {
                Ok(frame) => frame,
                Err(err) => match err {
                    ipcprims_peer::PeerError::Disconnected(_) => break,
                    _ => return Err(peer_error("receive failed", err)),
                },
            };

            if let Some(channels) = &args.channels {
                if !channels.contains(&frame.channel) {
                    continue;
                }
            }

            print_frame(&frame, peer.id(), format);
            printed = printed.saturating_add(1);

            if let Some(count) = args.count {
                if printed >= count {
                    return Ok(SUCCESS);
                }
            }
        }
    }

    Ok(SUCCESS)
}

fn install_ctrlc_handler(running: Arc<AtomicBool>) -> CliResult<()> {
    ctrlc::set_handler(move || {
        running.store(false, Ordering::SeqCst);
    })
    .map_err(|err| {
        CliError::new(
            crate::exit::INTERNAL,
            format!("signal handler setup failed: {err}"),
        )
    })
}
