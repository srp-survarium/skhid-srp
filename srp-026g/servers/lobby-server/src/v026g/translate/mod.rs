//! Chat translation (`docs/notes/chat-translation.md`): a player's line is
//! given to each reader in the reader's game language.
//!
//! - **Off by default** (`SRP_CHAT_TRANSLATE`); off, chat is as it was.
//! - **The backend** is pluggable ([`Backend`]): Claude Haiku over HTTPS
//!   ([`claude::Claude`], the one shipped) or a fake one for the tests.
//! - **Never in the lobby's way.** The backend runs on worker threads; the
//!   lobby's thread only queues a job ([`Translator::request`]) and picks up
//!   what is done ([`Translator::poll`]). A line waits for its translation
//!   at most [`Config::timeout`]; then, or on any error, the original goes.
//! - **A cache** of each (text, language) translated, so "gg wp all" or a
//!   line said to ten readers of one language is asked for once.
//! - **A failing backend is left alone a while** ([`BACKOFF`]): every line
//!   goes as it is, at once, instead of each waiting for the timeout.

pub mod claude;
pub mod detect;
pub mod glossary;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::LockExt;

use glossary::Glossary;

/// The game's five languages, as the client names them in `4F` (0x26C36E:
/// english 0, russian 1, ukrainian 2, german 3, polish 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lang {
    En,
    Ru,
    Uk,
    De,
    Pl,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Ru, Lang::Uk, Lang::De, Lang::Pl];

    /// The language byte of `4F` (and `45`).
    pub fn from_game(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Lang::En),
            1 => Some(Lang::Ru),
            2 => Some(Lang::Uk),
            3 => Some(Lang::De),
            4 => Some(Lang::Pl),
            _ => None,
        }
    }

    /// ISO 639-1.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::Uk => "uk",
            Lang::De => "de",
            Lang::Pl => "pl",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.code() == code)
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Russian",
            Lang::Uk => "Ukrainian",
            Lang::De => "German",
            Lang::Pl => "Polish",
        }
    }
}

/// What a backend made of a line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Translated {
    /// The language the line is really in, if the backend says (`None`:
    /// as guessed, or none of the five).
    pub source: Option<Lang>,
    /// A text for some of the languages asked for. One left out, or one
    /// equal to the line, is given the original.
    pub lines: Vec<(Lang, String)>,
}

/// What a backend is asked.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    /// The language it is guessed to be in (`detect`).
    pub from: Option<Lang>,
    /// The languages wanted (never `from`).
    pub to: Vec<Lang>,
    /// The glossary's terms the line has, with their words in `to`.
    pub glossary: Vec<glossary::Entry>,
}

/// A translation service. Called on a worker thread, never the lobby's: it
/// may block (up to its own timeout).
pub trait Backend: Send + Sync {
    fn translate(&self, line: &Line) -> Result<Translated, String>;
}

/// The settings (`SRP_CHAT_TRANSLATE*`, read once at start). The key is not
/// here: the backend reads it from the environment itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `SRP_CHAT_TRANSLATE_MODEL`, default [`claude::DEFAULT_MODEL`].
    pub model: String,
    /// `SRP_CHAT_TRANSLATE_TIMEOUT_MS` (default 2000, 200 to 10000): the
    /// longest a line waits for its translation.
    pub timeout: Duration,
}

/// The default wait for a translation.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(2000);
/// How long a backend that failed is not asked.
pub const BACKOFF: Duration = Duration::from_secs(15);
/// Jobs waiting for a worker; past them a line goes as it is.
const QUEUE: usize = 64;
/// The worker threads (the backend mostly waits on the network).
const WORKERS: usize = 4;
/// Translations kept.
const CACHE: usize = 10_000;

impl Config {
    /// `SRP_CHAT_TRANSLATE`: `claude` turns it on; unset or `off`, off.
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
        };
        match var("SRP_CHAT_TRANSLATE").as_deref() {
            None | Some("" | "off" | "0" | "no" | "false") => return None,
            Some("claude" | "on") => {}
            Some(other) => {
                log::error!("SRP_CHAT_TRANSLATE is `{other}`; it must be `claude` or off. Off");
                return None;
            }
        }
        let timeout = match var("SRP_CHAT_TRANSLATE_TIMEOUT_MS") {
            None => DEFAULT_TIMEOUT,
            Some(value) => match value.parse::<u64>() {
                Ok(ms @ 200..=10_000) => Duration::from_millis(ms),
                _ => {
                    log::error!(
                        "SRP_CHAT_TRANSLATE_TIMEOUT_MS is `{value}`; it must be 200 to 10000. \
                         Using {}",
                        DEFAULT_TIMEOUT.as_millis()
                    );
                    DEFAULT_TIMEOUT
                }
            },
        };
        let model = var("SRP_CHAT_TRANSLATE_MODEL")
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| claude::DEFAULT_MODEL.to_owned());
        Some(Self { model, timeout })
    }

    /// The translator: off (`None`, said loudly) when the backend can not
    /// be made, as with no key.
    pub fn build(&self) -> Option<Translator> {
        match claude::Claude::from_env(&self.model, self.timeout * 2) {
            Ok(backend) => {
                log::info!(
                    "Chat translation on: {} (a line waits at most {} ms)",
                    self.model,
                    self.timeout.as_millis()
                );
                Some(Translator::threaded(
                    Arc::new(backend),
                    WORKERS,
                    self.timeout,
                ))
            }
            Err(error) => {
                log::error!("SRP_CHAT_TRANSLATE is on but {error}: chat is not translated");
                None
            }
        }
    }
}

/// A job for a worker.
type Job = Line;

/// A job's result.
struct Done {
    text: String,
    to: Vec<Lang>,
    result: Result<Translated, String>,
}

enum Run {
    /// On worker threads.
    Threads {
        jobs: SyncSender<Job>,
        done: Receiver<Done>,
    },
    /// In the caller's thread, at once (the tests' clock is their own).
    Inline(Vec<Done>),
}

/// What the lobby gives a reader of a language.
#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    /// This text.
    Text(String),
    /// The line as it was said (same language, no answer, an error).
    Original,
    /// Not yet: asked and not answered.
    Waiting,
}

/// The lobby's side: the cache, the jobs under way and the workers' answers.
pub struct Translator {
    backend: Arc<dyn Backend>,
    run: Run,
    /// The longest a line waits.
    pub timeout: Duration,
    /// (text, language) -> the translation, `None` for "the original".
    cache: HashMap<(String, Lang), Option<String>>,
    order: VecDeque<(String, Lang)>,
    asked: HashSet<(String, Lang)>,
    backoff_until: Option<Instant>,
    /// The terms sent with a line (`glossary`): srp's own by default.
    pub glossary: Glossary,
}

impl Translator {
    /// With `workers` threads calling `backend`.
    pub fn threaded(backend: Arc<dyn Backend>, workers: usize, timeout: Duration) -> Self {
        let (jobs, queue) = mpsc::sync_channel::<Job>(QUEUE);
        let (finished, done) = mpsc::channel();
        let queue = Arc::new(Mutex::new(queue));
        for index in 0..workers.max(1) {
            let (backend, queue, finished) = (backend.clone(), queue.clone(), finished.clone());
            let spawned = std::thread::Builder::new()
                .name(format!("chat-translate-{index}"))
                .spawn(move || {
                    loop {
                        // The lock is held only while waiting for a job.
                        let job = queue.locked().recv();
                        let Ok(job) = job else { return };
                        let result = backend.translate(&job);
                        let done = Done {
                            text: job.text,
                            to: job.to,
                            result,
                        };
                        if finished.send(done).is_err() {
                            return;
                        }
                    }
                });
            if let Err(error) = spawned {
                log::error!("A chat translation worker did not start: {error}");
            }
        }
        Self::with(backend, Run::Threads { jobs, done }, timeout)
    }

    /// Calling `backend` in [`request`](Self::request) itself.
    pub fn inline(backend: Arc<dyn Backend>, timeout: Duration) -> Self {
        Self::with(backend, Run::Inline(Vec::new()), timeout)
    }

    fn with(backend: Arc<dyn Backend>, run: Run, timeout: Duration) -> Self {
        Self {
            backend,
            run,
            timeout,
            cache: HashMap::new(),
            order: VecDeque::new(),
            asked: HashSet::new(),
            backoff_until: None,
            glossary: Glossary::shared().clone(),
        }
    }

    /// What a reader of `lang` is given for `text` now.
    pub fn lookup(&self, text: &str, lang: Lang) -> Lookup {
        let key = (text.to_owned(), lang);
        match self.cache.get(&key) {
            Some(Some(translated)) => Lookup::Text(translated.clone()),
            Some(None) => Lookup::Original,
            None if self.asked.contains(&key) => Lookup::Waiting,
            None => Lookup::Original,
        }
    }

    /// Ask for `text` in the languages of `to` that are neither cached nor
    /// asked already. Those that can not be asked now (the backend is
    /// backing off, the queue is full) are left out: their readers get the
    /// original.
    pub fn request(&mut self, text: &str, from: Option<Lang>, to: &[Lang], now: Instant) {
        let to: Vec<Lang> = to
            .iter()
            .copied()
            .filter(|lang| {
                let key = (text.to_owned(), *lang);
                !self.cache.contains_key(&key) && !self.asked.contains(&key)
            })
            .collect();
        if to.is_empty() || self.backoff_until.is_some_and(|until| now < until) {
            return;
        }
        let job = Job {
            text: text.to_owned(),
            from,
            glossary: self.glossary.entries(text, from, &to),
            to,
        };
        let asked: Vec<Lang> = job.to.clone();
        match &mut self.run {
            Run::Threads { jobs, .. } => match jobs.try_send(job) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    log::warn!("Chat translation is behind: a line goes untranslated");
                    return;
                }
                Err(TrySendError::Disconnected(_)) => return,
            },
            Run::Inline(done) => {
                let result = self.backend.translate(&job);
                done.push(Done {
                    text: job.text,
                    to: job.to,
                    result,
                });
            }
        }
        for lang in asked {
            self.asked.insert((text.to_owned(), lang));
        }
    }

    /// Take the answers that came: into the cache. Returns how many.
    pub fn poll(&mut self, now: Instant) -> usize {
        let finished: Vec<Done> = match &mut self.run {
            Run::Threads { done, .. } => done.try_iter().collect(),
            Run::Inline(done) => std::mem::take(done),
        };
        let count = finished.len();
        for done in finished {
            for lang in &done.to {
                self.asked.remove(&(done.text.clone(), *lang));
            }
            match done.result {
                Ok(translated) => {
                    for lang in &done.to {
                        let text = translated
                            .lines
                            .iter()
                            .find(|(to, _)| to == lang)
                            .map(|(_, line)| clean(line))
                            .filter(|line| {
                                !line.is_empty()
                                    && *line != done.text
                                    && translated.source != Some(*lang)
                            });
                        self.remember((done.text.clone(), *lang), text);
                    }
                }
                Err(error) => {
                    // Not cached: asked again after the backoff.
                    if self.backoff_until.is_none_or(|until| now >= until) {
                        log::error!(
                            "Chat translation failed ({error}); lines go untranslated for {} s",
                            BACKOFF.as_secs()
                        );
                    }
                    self.backoff_until = Some(now + BACKOFF);
                }
            }
        }
        count
    }

    fn remember(&mut self, key: (String, Lang), value: Option<String>) {
        if self.cache.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > CACHE {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            }
        }
    }
}

/// One line of text, as the chat shows it: no line breaks or control
/// characters.
fn clean(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fakes for the tests.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// "<lang>: <text>" for each language asked; says nothing of the
    /// source. Counts its calls.
    #[derive(Default)]
    pub struct Echo {
        pub calls: AtomicUsize,
        /// Every line asked, in order.
        pub asked: Mutex<Vec<Line>>,
    }

    impl Backend for Echo {
        fn translate(&self, line: &Line) -> Result<Translated, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.asked.locked().push(line.clone());
            let (text, to) = (&line.text, &line.to);
            Ok(Translated {
                source: None,
                lines: to
                    .iter()
                    .map(|lang| (*lang, format!("{}: {text}", lang.code())))
                    .collect(),
            })
        }
    }

    /// Always fails.
    pub struct Broken;

    impl Backend for Broken {
        fn translate(&self, _: &Line) -> Result<Translated, String> {
            Err("the fake backend is broken".to_owned())
        }
    }

    /// Answers only when the test lets it.
    pub struct Stuck {
        pub release: Mutex<Receiver<()>>,
    }

    impl Backend for Stuck {
        fn translate(&self, line: &Line) -> Result<Translated, String> {
            let _ = self.release.locked().recv();
            Echo::default().translate(line)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Broken, Echo, Stuck};
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn a_translation_is_asked_once_and_cached() {
        let echo = Arc::new(Echo::default());
        let mut translator = Translator::inline(echo.clone(), DEFAULT_TIMEOUT);
        let now = Instant::now();
        translator.request("привет", Some(Lang::Ru), &[Lang::En, Lang::De], now);
        assert_eq!(translator.lookup("привет", Lang::En), Lookup::Waiting);
        translator.poll(now);
        assert_eq!(
            translator.lookup("привет", Lang::En),
            Lookup::Text("en: привет".into())
        );
        translator.request("привет", Some(Lang::Ru), &[Lang::En, Lang::De], now);
        translator.request("привет", Some(Lang::Ru), &[Lang::Pl], now);
        translator.poll(now);
        assert_eq!(echo.calls.load(Ordering::Relaxed), 2);
        assert_eq!(
            translator.lookup("привет", Lang::Pl),
            Lookup::Text("pl: привет".into())
        );
    }

    #[test]
    fn the_glossary_terms_of_a_line_go_with_it() {
        let echo = Arc::new(Echo::default());
        let mut translator = Translator::inline(echo.clone(), DEFAULT_TIMEOUT);
        translator.glossary = Glossary::parse(
            "map\tChemical Plant\tХимзавод\tХімзавод\tChemiewerk\tZakłady chemiczne\t",
        )
        .unwrap();
        translator.request(
            "го на химзавод",
            Some(Lang::Ru),
            &[Lang::De],
            Instant::now(),
        );
        let asked = echo.asked.locked();
        assert_eq!(asked[0].glossary.len(), 1);
        assert_eq!(asked[0].glossary[0].term, "химзавод");
        assert_eq!(
            asked[0].glossary[0].into,
            vec![(Lang::De, "Chemiewerk".to_owned())]
        );
    }

    #[test]
    fn a_failure_gives_the_original_and_the_backend_a_rest() {
        let mut translator = Translator::inline(Arc::new(Broken), DEFAULT_TIMEOUT);
        let now = Instant::now();
        translator.request("hallo", Some(Lang::De), &[Lang::En], now);
        translator.poll(now);
        assert_eq!(translator.lookup("hallo", Lang::En), Lookup::Original);
        // Backing off: not even asked.
        translator.request(
            "hallo",
            Some(Lang::De),
            &[Lang::En],
            now + Duration::from_secs(1),
        );
        assert_eq!(translator.lookup("hallo", Lang::En), Lookup::Original);
        assert_eq!(translator.poll(now), 0);
        translator.request("hallo", Some(Lang::De), &[Lang::En], now + BACKOFF);
        assert_eq!(translator.lookup("hallo", Lang::En), Lookup::Waiting);
    }

    #[test]
    fn the_source_language_or_an_echo_gives_the_original() {
        struct Says;
        impl Backend for Says {
            fn translate(&self, line: &Line) -> Result<Translated, String> {
                Ok(Translated {
                    source: Some(Lang::En),
                    lines: vec![(Lang::En, line.text.clone()), (Lang::Pl, line.text.clone())],
                })
            }
        }
        let mut translator = Translator::inline(Arc::new(Says), DEFAULT_TIMEOUT);
        let now = Instant::now();
        translator.request(
            "camper",
            Some(Lang::De),
            &[Lang::En, Lang::Pl, Lang::Ru],
            now,
        );
        translator.poll(now);
        for lang in [Lang::En, Lang::Pl, Lang::Ru] {
            assert_eq!(translator.lookup("camper", lang), Lookup::Original);
        }
    }

    #[test]
    fn a_line_break_in_an_answer_is_a_space() {
        assert_eq!(clean(" go\nto \r\n base\t "), "go to base");
    }

    #[test]
    fn workers_answer_off_the_callers_thread() {
        let (release, wait) = mpsc::channel();
        let stuck = Arc::new(Stuck {
            release: Mutex::new(wait),
        });
        let mut translator = Translator::threaded(stuck, 2, DEFAULT_TIMEOUT);
        let now = Instant::now();
        // The request returns at once although the backend is stuck.
        translator.request("hello", Some(Lang::En), &[Lang::Ru], now);
        assert_eq!(translator.poll(now), 0);
        assert_eq!(translator.lookup("hello", Lang::Ru), Lookup::Waiting);
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while translator.poll(now) == 0 {
            assert!(Instant::now() < deadline, "the worker never answered");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            translator.lookup("hello", Lang::Ru),
            Lookup::Text("ru: hello".into())
        );
    }
}
