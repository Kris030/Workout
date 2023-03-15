use std::{time::Duration, thread, fmt::Display, io::{stdin, stdout, Write}, env};
use rodio::{source::{SineWave, Source, Zero}, OutputStream, queue::queue};
pub struct Workout {
    sections: Vec<WorkoutSet>,
    name: String,
}
impl Workout {
    pub fn length(&self) -> Duration {
        self.sections.iter().map(|s| {
            let reps = s.reps as u32;
            let rests = s.set_rest.unwrap_or_default();
            let parts: Duration = s.parts.iter().map(|p| match p {
                WorkoutSetElement::Excercise { amount, .. } => match amount {
                    ExcerciseAmout::Time { duration, .. } => *duration,
                    ExcerciseAmout::Reps(_) => Duration::default(),
                },
                WorkoutSetElement::Rest { duration } => *duration,
            }).sum();

            rests * (reps - 1) + parts * reps
        }).sum()
    }
}
impl Display for Workout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [~{:.1} mins]", self.name, self.length().as_secs_f64() / 60.)?;
        Ok(())
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub enum BeepLevel {
    High = 0,
    Mid  = 1,
    Low  = 2,
}
impl BeepLevel {
    pub fn get_frequency(&self) -> f32 {
        use BeepLevel::*;
        match self {
            High => 750.,
            Mid => 600.,
            Low => 450.,
        }
    }
}

pub struct WorkoutSet {
    name: Option<String>,
    parts: Vec<WorkoutSetElement>,
    reps: u16,
    set_rest: Option<Duration>,
}
impl Display for WorkoutSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{name}")?;
        } else {
            write!(f, "[UNKNOWN]")?;
        }
        if self.reps > 1 {
            write!(f, " x{}", self.reps)?;
        }

        Ok(())
    }
}

pub enum ExcerciseAmout {
    Time {
        duration: Duration,
        midbeep: bool,
    },
    Reps(u16),
}
impl Display for ExcerciseAmout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExcerciseAmout::Time { duration, .. } => write!(f, "{duration:?}"),
            ExcerciseAmout::Reps(r) => write!(f, "x{r}"),
        }
    }
}


pub enum WorkoutSetElement {
    Excercise {
        name: String,
        amount: ExcerciseAmout,
    },
    Rest {
        duration: Duration,
    }
}
impl Display for WorkoutSetElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkoutSetElement::Excercise { name, amount, .. } =>
                write!(f, "[EXCERCISE]: {name} {amount}"),
            WorkoutSetElement::Rest { duration } =>
                write!(f, "[REST]: {duration:?}"),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(file) = env::args().nth(1) else {
        return Err("No file provided".into());
    };
    let workout = get_workout(&std::fs::read_to_string(file)?)?;

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

    play_workout(&workout, |level| 
        queue_in.append(presampled[level as usize].clone())
    )?;

    Ok(())
}

fn play_workout(workout: &Workout, beep: impl Fn(BeepLevel)) -> Result<(), Box<dyn std::error::Error>> {
    println!("Beginning {workout}");

    const PRE_SECTION_WAIT: Duration = Duration::from_secs(2);

    for s in &workout.sections {
        println!("\nSection {s}");

        for section_repetition in 0..s.reps {
            if section_repetition > 0 {
                println!("\nRepeating section ({} / {})", section_repetition + 1, s.reps);
            }

            beep(BeepLevel::Mid);
            beep(BeepLevel::Mid);

            thread::sleep(PRE_SECTION_WAIT);

            for p in &s.parts {
                println!("  {}", p);

                use WorkoutSetElement::*;
                use ExcerciseAmout::*;
                match &p {
                    Excercise { amount, .. } => {
                        beep(BeepLevel::High);

                        match amount {
                            Time { duration, midbeep } => {
                                if *midbeep {
                                    let dur_half = duration.div_f64(2.);

                                    thread::sleep(dur_half);
                                    println!("    Reached midpoint");
                                    beep(BeepLevel::Mid);
                                    thread::sleep(dur_half);
                                } else {
                                    thread::sleep(*duration);
                                }

                                beep(BeepLevel::Low);
                            },

                            Reps(_) => {
                                print!("    Press enter to continue! ");
                                stdout().flush()?;
                                let mut s = String::new();
                                stdin().read_line(&mut s)?;
                            },
                        }
                    },

                    Rest { duration, } => {
                        const LAST_5: Duration = Duration::from_secs(5);
                        if let Some(dur_first) = duration.checked_sub(LAST_5) {
                            thread::sleep(dur_first);
                            println!("    5s left");
                            beep(BeepLevel::Mid);
                            thread::sleep(LAST_5);
                        } else {
                            thread::sleep(*duration)
                        }
                    },
                }
            }

            if section_repetition < s.reps - 1 {
                if let Some(dur) = s.set_rest {
                    println!("[REST]: {dur:?}");
                    thread::sleep(dur.saturating_sub(PRE_SECTION_WAIT));
                }
            }
        }
    }

    println!("Readched the end. Good job!");

    thread::sleep(Duration::from_secs(1));

    Ok(())
}

fn parse_dur(s: &str) -> Result<Duration, Box<dyn std::error::Error>> {
    let (mins, secs)  = s[..5].split_at(2);
    let secs = &secs[1..];
    Ok(Duration::from_secs(mins.parse::<u64>()? * 60 + secs.parse::<u64>()?))
}

fn get_workout(source: &str) -> Result<Workout, Box<dyn std::error::Error>> {
    let lines: Vec<&str> = source.lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    let Some(workout_name) = lines[0]
        .trim_start()
        .strip_prefix("Workout ")
        .map(ToString::to_string) else {
        return Err("Didn't provide workout name".into());
    };

    let mut l = 1;
    let mut sections = vec![];
    while l < lines.len() {
        let Some(set) = lines[l]
            .trim_start()
            .strip_prefix("Set") else {
            return Err("Expected start of set".into());
        };

        let get_name_reps = || {
            let set = set.trim_start();
            if set.is_empty() {
                return (None, 1);
            }

            if let Some((name, reps)) = set.rsplit_once(' ') {
                if let Some(Ok(r)) = reps.strip_prefix('x')
                    .map(|v| v.parse::<u16>()) {
                    (Some(name), r)
                } else {
                    (Some(set), 1)
                }
            } else {
                (Some(set), 1)
            }
        };
        let set_name_reps = get_name_reps();
        l += 1;

        let mut set_parts = vec![];
        while l < lines.len() {
            let line = lines[l].trim_start();
            let Some((t, rest)) = line.split_once(' ') else {
                break;
            };
            let p = match t {
                "Excercise" => {
                    let Some((name, amount)) = rest.rsplit_once(' ') else {
                        return Err("No amount provided for excercise".into());
                    };

                    let amount = {
                        if let Some(reps) = amount.strip_prefix('x') {
                            ExcerciseAmout::Reps(reps.parse().map_err(|_| "Coudln't parse excercise reps")?)
                        } else {
                            let midbeep = amount.ends_with('"');
                            ExcerciseAmout::Time {
                                duration: parse_dur(amount)
                                    .map_err(|_| "Couldn't parse excercise duration")?,
                                midbeep,
                            }
                        }
                    };

                    WorkoutSetElement::Excercise { name: name.to_string(), amount }
                },
                "Rest" =>
                    WorkoutSetElement::Rest {
                        duration: parse_dur(rest)
                            .map_err(|_| "Couldn't parse rest duration")?
                    },
                _ => break
            };
            set_parts.push(p);
            l += 1;
        }
        
        let set_rest = if l < lines.len() {
            lines[l]
            .trim_start()
            .strip_prefix("Set rest ")
            .and_then(|r| parse_dur(r).ok())
        } else {
            None
        };
        if set_rest.is_some() {
            l += 1;
        }

        sections.push(WorkoutSet {
            name: set_name_reps.0.map(ToString::to_string),
            reps: set_name_reps.1,
            parts: set_parts,
            set_rest,
        });
    }

    Ok(Workout {
        name: workout_name,
        sections,
    })
}
