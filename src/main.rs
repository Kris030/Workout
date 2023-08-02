pub mod workout;

use anyhow::Result;
use rodio::{
    queue::queue,
    source::{SineWave, Source, Zero},
    OutputStream,
};
use std::{env, time::Duration};
use workout::{do_workout, load_workout, BeepLevel};

// TODO: better errors

fn parse_from(s: &str) -> Result<(u16, u16, u16)> {
    let Some((mut set, excercise)) = s.split_once('.') else {
        return Err(anyhow::Error::msg("Starting position format: SET[/SET_REP].EXCERCISE"));
    };

    let set_rep;
    if let Some((seti, srep)) = set.split_once('/') {
        set = seti;
        set_rep = srep.parse::<u16>()?.saturating_sub(1);
    } else {
        set_rep = 0;
    }

    let set = set.parse::<u16>()?.saturating_sub(1);
    let excercise = excercise.parse::<u16>()?.saturating_sub(1);
    Ok((set, set_rep, excercise))
}

fn main() -> Result<()> {
    let Some(file) = env::args().nth(1) else {
        return Err(anyhow::Error::msg("No file provided"));
    };
    let from = if let Some(a) = env::args().nth(2) {
        parse_from(&a)?
    } else {
        (0, 0, 0)
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
                Zero::<i16>::new(1, 1).take_duration(Duration::from_secs_f64(0.1)),
                beep_len,
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

    do_workout(workout, from, |level| {
        queue_in.append(presampled[level as usize].clone())
    })?;

    Ok(())
}
