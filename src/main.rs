pub mod workout;

use rodio::{source::{SineWave, Source, Zero}, OutputStream, queue::queue};
use workout::{load_workout, BeepLevel, do_workout};
use std::{time::Duration, env};

// TODO: better errors

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(file) = env::args().nth(1) else {
        return Err("No file provided".into());
    };
    let source = std::fs::read_to_string(file)?;
    let workout = load_workout(&source)?;

    // FIXME: ALSA lib pcm.c:8570:(snd_pcm_recover) underrun occurred
    let (queue_in, queue_out) = queue(true);
    let (_stream, stream_handle) = OutputStream::try_default()?;
    stream_handle.play_raw(queue_out)?;

    let beep_len = Duration::from_secs_f64(0.5);
    let beep_sample = |level: BeepLevel| {
        SineWave::new(level.get_frequency())
            .take_duration(beep_len)
            .fade_in(Duration::from_secs_f64(0.1))
            .take_crossfade_with(
                Zero::<i16>::new(1, 1)
                    .take_duration(Duration::from_secs_f64(0.1)),
                beep_len
            )
    };

    let presampled = [
        beep_sample(BeepLevel::Low).buffered(),
        beep_sample(BeepLevel::Mid).buffered(),
        beep_sample(BeepLevel::High).buffered(),
    ];

    // TODO: handle pausing somehow
    // thread::scope(|s| {
    //  s.spawn(|| {
    //          ...
    //     });
    // });

    do_workout(&workout, |level| 
        queue_in.append(presampled[level as usize].clone())
    )?;

    Ok(())
}
