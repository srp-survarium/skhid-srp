//! Chat translation (`docs/notes/chat-translation.md`): a player's line is
//! given to each reader in the reader's game language.
//!
//! - **Off by default** (`SRP_CHAT_TRANSLATE`); off, chat is as it was.
//! - **The backend** is pluggable ([`Backend`]): xAI Grok over HTTPS
//!   ([`grok::Grok`], the default), Anthropic Claude ([`claude::Claude`]),
//!   or a fake one for the tests.
//! - **A gate first, failing closed:** srp's word list (`swear`, on the
//!   lobby's thread, free), then a tiny local classifier (`moderate`, on a
//!   worker) say whether the line may go at all; "no", a timeout or an
//!   error, and the readers get the original. A player whose lines are
//!   refused often, and the whole server when refusals spike, are stopped a
//!   while (`guard`).
//! - **Never in the lobby's way.** The gate's call and the backend run on
//!   worker threads; the lobby's thread only queues a job
//!   ([`Translator::request`]) and picks up what is done
//!   ([`Translator::poll`]). A line waits for its translation at most
//!   [`Config::timeout`]; then, or on any error, the original goes.
//! - **A cache** of each (text, language) translated, so "gg wp all" or a
//!   line said to ten readers of one language is asked for once.
//! - **A failing backend is left alone a while** ([`BACKOFF`]): every line
//!   goes as it is, at once, instead of each waiting for the timeout.

pub mod claude;
pub mod detect;
pub mod glossary;
pub mod grok;
pub mod guard;
pub mod moderate;
pub mod swear;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::LockExt;

use glossary::Glossary;
use guard::{Allow, Event, Guard};
use moderate::{Gate, Moderator};
use swear::{SwearFilter, Verdict};

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
    /// The backend declined the line (a refusal, or an answer that is no
    /// translation): every reader gets the original, and the backend is
    /// not rested for it.
    pub refused: bool,
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

/// The translation service (`SRP_CHAT_TRANSLATE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// xAI Grok (`grok`; `on` too): the default.
    Grok,
    /// Anthropic Claude (`claude`).
    Claude,
}

impl Provider {
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Grok => grok::DEFAULT_MODEL,
            Provider::Claude => claude::DEFAULT_MODEL,
        }
    }

    /// The backend, with its key from the environment.
    pub fn backend(self, model: &str, timeout: Duration) -> Result<Arc<dyn Backend>, String> {
        Ok(match self {
            Provider::Grok => Arc::new(grok::Grok::from_env(model, timeout)?),
            Provider::Claude => Arc::new(claude::Claude::from_env(model, timeout)?),
        })
    }
}

/// The settings (`SRP_CHAT_TRANSLATE*`, `SRP_CHAT_MODERATION*`, read once at
/// start). The key is not here: the backend reads it from the environment
/// itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub provider: Provider,
    /// `SRP_CHAT_TRANSLATE_MODEL`, default the provider's
    /// ([`grok::DEFAULT_MODEL`], [`claude::DEFAULT_MODEL`]).
    pub model: String,
    /// `SRP_CHAT_TRANSLATE_TIMEOUT_MS` (default 2000, 200 to 10000): the
    /// longest a line waits for its translation.
    pub timeout: Duration,
    /// The gate (`moderate`); `Err`: translation stays off, why.
    pub gate: Result<Gate, String>,
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
        let provider = match var("SRP_CHAT_TRANSLATE").as_deref() {
            None | Some("" | "off" | "0" | "no" | "false") => return None,
            Some("grok" | "on") => Provider::Grok,
            Some("claude") => Provider::Claude,
            Some(other) => {
                log::error!(
                    "SRP_CHAT_TRANSLATE is `{other}`; it must be `grok`, `claude` or off. Off"
                );
                return None;
            }
        };
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
            .unwrap_or_else(|| provider.default_model().to_owned());
        Some(Self {
            provider,
            model,
            timeout,
            gate: Gate::from_env(),
        })
    }

    /// The translator: off (`None`, said loudly) when the gate or the
    /// backend can not be made, as with no key.
    pub fn build(&self) -> Option<Translator> {
        // Nothing goes out without its gate.
        let gate = match &self.gate {
            Ok(gate) => gate.clone(),
            Err(error) => {
                log::error!("SRP_CHAT_TRANSLATE is on but {error}: chat is not translated");
                return None;
            }
        };
        match self.provider.backend(&self.model, self.timeout * 2) {
            Ok(backend) => {
                log::info!(
                    "Chat translation on: {} (a line waits at most {} ms), gated by {}",
                    self.model,
                    self.timeout.as_millis(),
                    gate.url.as_deref().unwrap_or("the word list alone")
                );
                let moderator = gate
                    .moderator()
                    .map(|moderator| Arc::new(moderator) as Arc<dyn Moderator>);
                Some(Translator::threaded(
                    backend,
                    moderator,
                    gate,
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
struct Job {
    line: Line,
    /// Whose line it is.
    player: u32,
}

/// What became of a job.
enum Outcome {
    Translated(Translated),
    /// The gate said no.
    No,
    /// The gate did not answer (a timeout, an error): not sent.
    Unchecked(String),
    /// The backend failed.
    Failed(String),
}

/// A job's result.
struct Done {
    key: String,
    to: Vec<Lang>,
    player: u32,
    outcome: Outcome,
}

/// What never reached the backend, or came back as no translation: counts
/// only, never a text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Withheld {
    /// Lines the word list refused (blocked, or mostly obscene).
    pub listed: u64,
    /// Lines the gate's classifier refused.
    pub flagged: u64,
    /// Lines whose gate did not answer: not sent (fail closed).
    pub unchecked: u64,
    /// Lines of a throttled player, or while the breaker was open.
    pub stopped: u64,
    /// Lines the backend refused, or answered with no translation.
    pub refused: u64,
}

/// The gate's classifier, then the backend: on a worker, or inline.
fn run(
    backend: &dyn Backend,
    moderator: Option<&dyn Moderator>,
    gate: &Gate,
    job: &Job,
) -> Outcome {
    if let Some(moderator) = moderator {
        match moderator.score(&job.line.text, job.line.from) {
            Ok(score) if score < gate.threshold(job.line.from) => {}
            Ok(_) => return Outcome::No,
            Err(error) => return Outcome::Unchecked(error),
        }
    }
    match backend.translate(&job.line) {
        Ok(translated) => Outcome::Translated(translated),
        Err(error) => Outcome::Failed(error),
    }
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
    moderator: Option<Arc<dyn Moderator>>,
    gate: Gate,
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
    /// The word list (`swear`): srp's own.
    pub swear: SwearFilter,
    pub guard: Guard,
    pub withheld: Withheld,
}

impl Translator {
    /// With `workers` threads calling the gate's `moderator` (if any) and
    /// `backend`.
    pub fn threaded(
        backend: Arc<dyn Backend>,
        moderator: Option<Arc<dyn Moderator>>,
        gate: Gate,
        workers: usize,
        timeout: Duration,
    ) -> Self {
        let (jobs, queue) = mpsc::sync_channel::<Job>(QUEUE);
        let (finished, done) = mpsc::channel();
        let queue = Arc::new(Mutex::new(queue));
        for index in 0..workers.max(1) {
            let (backend, moderator, gate, queue, finished) = (
                backend.clone(),
                moderator.clone(),
                gate.clone(),
                queue.clone(),
                finished.clone(),
            );
            let spawned = std::thread::Builder::new()
                .name(format!("chat-translate-{index}"))
                .spawn(move || {
                    loop {
                        // The lock is held only while waiting for a job.
                        let job = queue.locked().recv();
                        let Ok(job) = job else { return };
                        let outcome = run(&*backend, moderator.as_deref(), &gate, &job);
                        let done = Done {
                            key: job.line.text,
                            to: job.line.to,
                            player: job.player,
                            outcome,
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
        Self::with(
            backend,
            moderator,
            gate,
            Run::Threads { jobs, done },
            timeout,
        )
    }

    /// Calling the gate and `backend` in [`request`](Self::request) itself.
    pub fn inline(
        backend: Arc<dyn Backend>,
        moderator: Option<Arc<dyn Moderator>>,
        timeout: Duration,
    ) -> Self {
        let gate = Gate {
            url: None,
            thresholds: Vec::new(),
            timeout: moderate::DEFAULT_TIMEOUT,
        };
        Self::with(backend, moderator, gate, Run::Inline(Vec::new()), timeout)
    }

    fn with(
        backend: Arc<dyn Backend>,
        moderator: Option<Arc<dyn Moderator>>,
        gate: Gate,
        run: Run,
        timeout: Duration,
    ) -> Self {
        Self {
            backend,
            moderator,
            gate,
            run,
            timeout,
            cache: HashMap::new(),
            order: VecDeque::new(),
            asked: HashSet::new(),
            backoff_until: None,
            glossary: Glossary::shared().clone(),
            swear: SwearFilter::built_in(),
            guard: Guard::default(),
            withheld: Withheld::default(),
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

    fn log_withheld(&self) {
        log::info!(
            "Chat translation, lines not translated: {:?}",
            self.withheld
        );
    }

    /// Ask for `player`'s `text` in the languages of `to` that are neither
    /// cached nor asked already. Those that can not be asked now (the word
    /// list says no, the player or the server is stopped, the backend is
    /// backing off, the queue is full) are left out: their readers get the
    /// original.
    pub fn request(
        &mut self,
        text: &str,
        from: Option<Lang>,
        to: &[Lang],
        player: u32,
        now: Instant,
    ) {
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
        if self.guard.allows(player, now) != Allow::Yes {
            self.withheld.stopped += 1;
            return;
        }
        if self.swear.judge(text) != Verdict::Pass {
            for lang in to {
                self.remember((text.to_owned(), lang), None);
            }
            self.withheld.listed += 1;
            self.guard.record(player, Event::No, now);
            self.log_withheld();
            return;
        }
        let job = Job {
            line: Line {
                glossary: self.glossary.entries(text, from, &to),
                text: text.to_owned(),
                from,
                to,
            },
            player,
        };
        let asked: Vec<Lang> = job.line.to.clone();
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
                let outcome = run(&*self.backend, self.moderator.as_deref(), &self.gate, &job);
                done.push(Done {
                    key: job.line.text,
                    to: job.line.to,
                    player,
                    outcome,
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
                self.asked.remove(&(done.key.clone(), *lang));
            }
            let original = |translator: &mut Self| {
                for lang in &done.to {
                    translator.remember((done.key.clone(), *lang), None);
                }
            };
            match done.outcome {
                Outcome::No => {
                    original(self);
                    self.withheld.flagged += 1;
                    self.guard.record(done.player, Event::No, now);
                    self.log_withheld();
                }
                Outcome::Unchecked(error) => {
                    // Not cached: asked again later, when the gate answers.
                    self.withheld.unchecked += 1;
                    log::warn!("The chat gate did not answer ({error}): a line was not sent");
                }
                Outcome::Translated(translated) if translated.refused => {
                    original(self);
                    self.withheld.refused += 1;
                    self.guard.record(done.player, Event::Refused, now);
                    self.log_withheld();
                }
                Outcome::Translated(translated) => {
                    self.guard.record(done.player, Event::Sent, now);
                    for lang in &done.to {
                        let text = translated
                            .lines
                            .iter()
                            .find(|(to, _)| to == lang)
                            .map(|(_, line)| clean(line))
                            .filter(|line| {
                                !line.is_empty()
                                    && *line != done.key
                                    && translated.source != Some(*lang)
                            });
                        self.remember((done.key.clone(), *lang), text);
                    }
                }
                Outcome::Failed(error) => {
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
                refused: false,
            })
        }
    }

    /// Declines every line (a refusal).
    pub struct Refuses;

    impl Backend for Refuses {
        fn translate(&self, _: &Line) -> Result<Translated, String> {
            Ok(Translated {
                refused: true,
                ..Translated::default()
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

    /// A gate that scores a line 0.9 when it holds `bad`, else 0.1; or that
    /// never answers.
    pub enum Classifier {
        Flags(&'static str),
        Down,
    }

    impl Moderator for Classifier {
        fn score(&self, text: &str, _: Option<Lang>) -> Result<f32, String> {
            match self {
                Classifier::Flags(bad) => Ok(if text.contains(bad) { 0.9 } else { 0.1 }),
                Classifier::Down => Err("timed out".to_owned()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Broken, Classifier, Echo, Refuses, Stuck};
    use super::*;
    use std::sync::atomic::Ordering;

    const PLAYER: u32 = 1;

    fn inline(backend: Arc<dyn Backend>) -> Translator {
        Translator::inline(backend, None, DEFAULT_TIMEOUT)
    }

    #[test]
    fn a_translation_is_asked_once_and_cached() {
        let echo = Arc::new(Echo::default());
        let mut translator = inline(echo.clone());
        let now = Instant::now();
        translator.request("привет", Some(Lang::Ru), &[Lang::En, Lang::De], PLAYER, now);
        assert_eq!(translator.lookup("привет", Lang::En), Lookup::Waiting);
        translator.poll(now);
        assert_eq!(
            translator.lookup("привет", Lang::En),
            Lookup::Text("en: привет".into())
        );
        translator.request("привет", Some(Lang::Ru), &[Lang::En, Lang::De], PLAYER, now);
        translator.request("привет", Some(Lang::Ru), &[Lang::Pl], PLAYER, now);
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
        let mut translator = inline(echo.clone());
        translator.glossary = Glossary::parse(
            "map\tChemical Plant\tХимзавод\tХімзавод\tChemiewerk\tZakłady chemiczne\t",
        )
        .unwrap();
        translator.request(
            "го на химзавод",
            Some(Lang::Ru),
            &[Lang::De],
            PLAYER,
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
    fn swearing_goes_as_it_is_and_the_no_cases_never_go() {
        let echo = Arc::new(Echo::default());
        let mut translator = inline(echo.clone());
        let now = Instant::now();
        // The owner's: swearing is translated, not masked.
        translator.request("бля, промазал", Some(Lang::Ru), &[Lang::De], PLAYER, now);
        translator.poll(now);
        assert_eq!(echo.asked.locked()[0].text, "бля, промазал");
        // Blocked and mostly obscene lines: never sent.
        for (player, line) in [(2, "сука блять нахуй"), (3, "kill yourself noob")] {
            translator.request(line, None, &[Lang::En, Lang::De], player, now);
            translator.poll(now);
            assert_eq!(translator.lookup(line, Lang::En), Lookup::Original);
        }
        assert_eq!(echo.calls.load(Ordering::Relaxed), 1);
        assert_eq!(translator.withheld.listed, 2);
    }

    #[test]
    fn the_gate_decides_and_fails_closed() {
        let echo = Arc::new(Echo::default());
        let now = Instant::now();
        let gate = Arc::new(Classifier::Flags("bad"));
        let mut translator = Translator::inline(echo.clone(), Some(gate), DEFAULT_TIMEOUT);
        translator.request("a bad line", Some(Lang::En), &[Lang::Ru], PLAYER, now);
        translator.request("a good line", Some(Lang::En), &[Lang::Ru], PLAYER, now);
        translator.poll(now);
        assert_eq!(translator.lookup("a bad line", Lang::Ru), Lookup::Original);
        assert_eq!(
            translator.lookup("a good line", Lang::Ru),
            Lookup::Text("ru: a good line".into())
        );
        assert_eq!(translator.withheld.flagged, 1);
        assert_eq!(echo.calls.load(Ordering::Relaxed), 1);

        // A gate that does not answer: nothing is sent.
        let echo = Arc::new(Echo::default());
        let mut translator = Translator::inline(
            echo.clone(),
            Some(Arc::new(Classifier::Down)),
            DEFAULT_TIMEOUT,
        );
        translator.request("a good line", Some(Lang::En), &[Lang::Ru], PLAYER, now);
        translator.poll(now);
        assert_eq!(translator.lookup("a good line", Lang::Ru), Lookup::Original);
        assert_eq!(echo.calls.load(Ordering::Relaxed), 0);
        assert_eq!(translator.withheld.unchecked, 1);
    }

    #[test]
    fn sexual_content_with_minors_is_never_sent() {
        let echo = Arc::new(Echo::default());
        let mut translator = inline(echo.clone());
        let now = Instant::now();
        for (player, line) in [(2, "anyone got child porn"), (3, "есть детское порно?")]
        {
            translator.request(line, None, &[Lang::De], player, now);
            translator.poll(now);
        }
        assert_eq!(echo.calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_player_refused_three_times_is_not_sent_for_a_while() {
        let echo = Arc::new(Echo::default());
        let mut translator = inline(echo.clone());
        let now = Instant::now();
        for line in ["сука блять нахуй", "fuck fuck fuck", "бля бля бля сука"]
        {
            translator.request(line, None, &[Lang::De], PLAYER, now);
        }
        translator.request("hello all", Some(Lang::En), &[Lang::De], PLAYER, now);
        translator.poll(now);
        assert_eq!(translator.lookup("hello all", Lang::De), Lookup::Original);
        assert_eq!(echo.calls.load(Ordering::Relaxed), 0);
        assert_eq!(translator.withheld.stopped, 1);
        // Another player is not.
        translator.request("hello all", Some(Lang::En), &[Lang::De], 2, now);
        translator.poll(now);
        assert_eq!(echo.calls.load(Ordering::Relaxed), 1);
        // The first, after its cooldown.
        let later = now + guard::THROTTLE_COOLDOWN;
        translator.request("good game", Some(Lang::En), &[Lang::De], PLAYER, later);
        translator.poll(later);
        assert_eq!(echo.calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn refusals_across_the_server_open_the_breaker() {
        let mut translator = inline(Arc::new(Refuses));
        let now = Instant::now();
        for (player, line) in ["one line", "two lines", "three lines"].iter().enumerate() {
            translator.request(line, Some(Lang::En), &[Lang::Ru], player as u32, now);
            translator.poll(now);
        }
        assert_eq!(translator.withheld.refused, 3);
        // Open: no line of anybody goes.
        translator.request("four lines", Some(Lang::En), &[Lang::Ru], 9, now);
        assert_eq!(translator.lookup("four lines", Lang::Ru), Lookup::Original);
        assert_eq!(translator.withheld.stopped, 1);
        assert_eq!(translator.guard.trips, 1);
        let later = now + guard::BREAKER_OFF;
        translator.request("four lines", Some(Lang::En), &[Lang::Ru], 9, later);
        assert_eq!(translator.lookup("four lines", Lang::Ru), Lookup::Waiting);
    }

    #[test]
    fn a_failure_gives_the_original_and_the_backend_a_rest() {
        let mut translator = inline(Arc::new(Broken));
        let now = Instant::now();
        translator.request("hallo", Some(Lang::De), &[Lang::En], PLAYER, now);
        translator.poll(now);
        assert_eq!(translator.lookup("hallo", Lang::En), Lookup::Original);
        // Backing off: not even asked.
        translator.request(
            "hallo",
            Some(Lang::De),
            &[Lang::En],
            PLAYER,
            now + Duration::from_secs(1),
        );
        assert_eq!(translator.lookup("hallo", Lang::En), Lookup::Original);
        assert_eq!(translator.poll(now), 0);
        translator.request("hallo", Some(Lang::De), &[Lang::En], PLAYER, now + BACKOFF);
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
                    refused: false,
                })
            }
        }
        let mut translator = inline(Arc::new(Says));
        let now = Instant::now();
        translator.request(
            "camper",
            Some(Lang::De),
            &[Lang::En, Lang::Pl, Lang::Ru],
            PLAYER,
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
        let gate = Gate {
            url: None,
            thresholds: Vec::new(),
            timeout: moderate::DEFAULT_TIMEOUT,
        };
        let mut translator = Translator::threaded(stuck, None, gate, 2, DEFAULT_TIMEOUT);
        let now = Instant::now();
        // The request returns at once although the backend is stuck.
        translator.request("hello", Some(Lang::En), &[Lang::Ru], PLAYER, now);
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
