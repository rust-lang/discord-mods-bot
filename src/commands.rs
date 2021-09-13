use crate::{
    //api,
    state_machine::{CharacterSet, StateMachine},
    Error,
};
use indexmap::IndexMap;
use reqwest::Client as HttpClient;
use serenity::{model::channel::Message, prelude::Context};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

pub const PREFIX: &str = "?";

type ResultFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>>>>;

pub trait Auth: 'static {
    fn call(&self, args: Arc<Args>) -> ResultFuture<bool, Error>;
}

impl<F, G> Auth for F
where
    F: Fn(Arc<Args>) -> G + 'static,
    G: Future<Output = Result<bool, Error>> + 'static,
{
    fn call(&self, args: Arc<Args>) -> ResultFuture<bool, Error> {
        let fut = (self)(args);
        Box::pin(async move { fut.await })
    }
}

pub trait CommandEndpoint: 'static {
    fn call(&self, args: Arc<Args>) -> ResultFuture<(), Error>;
}

impl<F, G> CommandEndpoint for F
where
    F: Fn(Arc<Args>) -> G + 'static,
    G: Future<Output = Result<(), Error>> + 'static,
{
    fn call(&self, args: Arc<Args>) -> ResultFuture<(), Error> {
        let fut = (self)(args);
        Box::pin(async move { fut.await })
    }
}

pub struct Command {
    auth: Box<dyn Auth>,
    inner: Box<dyn CommandEndpoint>,
}

impl Command {
    pub fn new(inner: impl CommandEndpoint) -> Self {
        Self {
            auth: Box::new(|_| async { Ok(true) }),
            inner: Box::new(inner),
        }
    }

    pub fn new_with_auth(inner: impl CommandEndpoint, auth: impl Auth) -> Self {
        Self {
            auth: Box::new(auth),
            inner: Box::new(inner),
        }
    }

    async fn call(&self, args: Arc<Args>) -> Result<(), Error> {
        info!("Checking permissions");
        match self.auth.call(args.clone()).await {
            Ok(true) => {
                info!("Executing command");
                self.inner.call(args.clone()).await?;
            }
            Ok(false) => {
                info!("Not executing command, unauthorized");
            }
            Err(e) => (),
        };

        Ok(())
    }
}

type CommandMap = HashMap<usize, Arc<Command>>;

pub struct Args {
    pub http: Arc<HttpClient>,
    pub cx: Context,
    pub msg: Message,
    pub params: HashMap<&'static str, String>,
}

pub struct Commands {
    http_client: Arc<HttpClient>,
    state_machine: StateMachine,
    command_map: CommandMap,
}

impl Commands {
    pub fn new() -> Self {
        Self {
            http_client: Arc::new(HttpClient::new()),
            state_machine: StateMachine::new(),
            command_map: CommandMap::new(),
        }
    }

    pub fn add(&mut self, input: &'static str, command: Command) {
        info!("Adding command {}", &input);
        let mut state = 0;

        let mut reused_space_state = None;
        let mut opt_final_states = vec![];

        let handler = Arc::new(command);

        input
            .split(' ')
            .filter(|segment| segment.len() > 0)
            .enumerate()
            .for_each(|(i, segment)| {
                if let Some(name) = key_value_pair(segment) {
                    if let Some(lambda) = reused_space_state {
                        state = self.add_key_value(name, lambda);
                        self.state_machine.add_next_state(state, lambda);
                        opt_final_states.push(state);

                        state = self.add_quoted_key_value(name, lambda);
                        self.state_machine.add_next_state(state, lambda);
                        opt_final_states.push(state);
                    } else {
                        opt_final_states.push(state);
                        state = self.add_space(state, i);
                        reused_space_state = Some(state);

                        state = self.add_key_value(name, state);
                        self.state_machine
                            .add_next_state(state, reused_space_state.unwrap());
                        opt_final_states.push(state);

                        state = self.add_quoted_key_value(name, reused_space_state.unwrap());
                        self.state_machine
                            .add_next_state(state, reused_space_state.unwrap());
                        opt_final_states.push(state);
                    }
                } else {
                    reused_space_state = None;
                    opt_final_states.truncate(0);
                    let last_state = state;
                    state = self.add_space(state, i);

                    if segment.starts_with("```\n") && segment.ends_with("```") {
                        state = self.add_code_segment_multi_line(state, segment);
                    } else if segment.starts_with("```") && segment.ends_with("```") {
                        state = self.add_code_segment_single_line(state, segment, 3);
                    } else if segment.starts_with('`') && segment.ends_with('`') {
                        state = self.add_code_segment_single_line(state, segment, 1);
                    } else if segment.starts_with('{') && segment.ends_with('}') {
                        state = self.add_dynamic_segment(state, segment);
                    } else if segment.ends_with("...") {
                        if segment == "..." {
                            self.state_machine.set_final_state(last_state);
                            self.command_map.insert(last_state, handler.clone());
                            state = self.add_unnamed_remaining_segment(last_state);
                        } else {
                            state = self.add_remaining_segment(state, segment);
                        }
                    } else {
                        segment.chars().for_each(|ch| {
                            state = self.state_machine.add(state, CharacterSet::from_char(ch))
                        });
                    }
                }
            });

        if reused_space_state.is_some() {
            opt_final_states.iter().for_each(|state| {
                self.state_machine.set_final_state(*state);
                self.command_map.insert(*state, handler.clone());
            });
        } else {
            self.state_machine.set_final_state(state);
            self.command_map.insert(state, handler.clone());
        }
    }

    pub fn help(&mut self, cmd: &'static str, desc: &'static str, command: Command) {
        let base_cmd = &cmd[1..];
        info!("Adding command ?help {}", &base_cmd);
        let mut state = 0;

        //        self.menu.as_mut().map(|menu| {
        //            menu.insert(cmd, (desc, guard));
        //            menu
        //        });

        state = self.add_help_menu(base_cmd, state);
        self.state_machine.set_final_state(state);
        self.command_map.insert(state, Arc::new(command));
    }

    //pub fn menu(&mut self) -> Option<IndexMap<&'static str, (&'static str, GuardFn)>> {
    //    self.menu.take()
    //}

    pub async fn execute(&self, cx: Context, msg: Message) {
        let message = &msg.content;
        if !msg.is_own(&cx).await && message.starts_with(PREFIX) {
            if let Some(matched) = self.state_machine.process(message) {
                info!("Processing command: {}", message);
                let args = Arc::new(Args {
                    http: self.http_client.clone(),
                    cx: cx,
                    msg: msg,
                    params: matched.params,
                });

                self.command_map
                    .get(&matched.state)
                    .unwrap()
                    .call(args.clone())
                    .await;

                //                 match matched.handler.authorize(args.clone()) {
                //                     Ok(true) => {
                //                         info!("Executing command");
                //                         if let Err(e) = matched.handler.call(args).await {
                //                             error!("{}", e);
                //                         }
                //                     }
                //                     Ok(false) => {
                //                         info!("Not executing command, unauthorized");
                //                         if let Err(e) = api::send_reply(
                //                             args.clone(),
                //                             "You do not have permission to run this command",
                //                         ) {
                //                             error!("{}", e);
                //                         }
                //                     }
                //                     Err(e) => error!("{}", e),
                //                 }
            }
        }
    }

    fn add_space(&mut self, mut state: usize, i: usize) -> usize {
        if i > 0 {
            let char_set = CharacterSet::from_chars(&[' ', '\n']);

            state = self.state_machine.add(state, char_set);
            self.state_machine.add_next_state(state, state);
        }
        state
    }

    fn add_help_menu(&mut self, cmd: &'static str, mut state: usize) -> usize {
        "?help".chars().for_each(|ch| {
            state = self.state_machine.add(state, CharacterSet::from_char(ch));
        });
        state = self.add_space(state, 1);
        cmd.chars().for_each(|ch| {
            state = self.state_machine.add(state, CharacterSet::from_char(ch));
        });

        state
    }

    fn add_dynamic_segment(&mut self, mut state: usize, s: &'static str) -> usize {
        let name = &s[1..s.len() - 1];

        let mut char_set = CharacterSet::any();
        char_set.remove(&[' ']);
        state = self.state_machine.add(state, char_set);
        self.state_machine.add_next_state(state, state);
        self.state_machine.start_parse(state, name);
        self.state_machine.end_parse(state);

        state
    }

    fn add_remaining_segment(&mut self, mut state: usize, s: &'static str) -> usize {
        let name = &s[..s.len() - 3];

        let char_set = CharacterSet::any();
        state = self.state_machine.add(state, char_set);
        self.state_machine.add_next_state(state, state);
        self.state_machine.start_parse(state, name);
        self.state_machine.end_parse(state);

        state
    }

    fn add_unnamed_remaining_segment(&mut self, mut state: usize) -> usize {
        let char_set = CharacterSet::any();
        state = self.state_machine.add(state, char_set);
        self.state_machine.add_next_state(state, state);

        state
    }

    fn add_code_segment_multi_line(&mut self, mut state: usize, s: &'static str) -> usize {
        let name = &s[4..s.len() - 3];

        "```".chars().for_each(|ch| {
            state = self.state_machine.add(state, CharacterSet::from_char(ch));
        });

        let lambda = state;

        let mut char_set = CharacterSet::any();
        char_set.remove(&['`', ' ', '\n']);
        state = self.state_machine.add(state, char_set);
        self.state_machine.add_next_state(state, state);

        state = self.state_machine.add(state, CharacterSet::from_char('\n'));

        self.state_machine.add_next_state(lambda, state);

        state = self.state_machine.add(state, CharacterSet::any());
        self.state_machine.add_next_state(state, state);
        self.state_machine.start_parse(state, name);
        self.state_machine.end_parse(state);

        "```".chars().for_each(|ch| {
            state = self.state_machine.add(state, CharacterSet::from_char(ch));
        });

        state
    }

    fn add_code_segment_single_line(
        &mut self,
        mut state: usize,
        s: &'static str,
        n_backticks: usize,
    ) -> usize {
        use std::iter::repeat;

        let name = &s[n_backticks..s.len() - n_backticks];

        repeat('`').take(n_backticks).for_each(|ch| {
            state = self.state_machine.add(state, CharacterSet::from_char(ch));
        });
        state = self.state_machine.add(state, CharacterSet::any());
        self.state_machine.add_next_state(state, state);
        self.state_machine.start_parse(state, name);
        self.state_machine.end_parse(state);
        repeat('`').take(n_backticks).for_each(|ch| {
            state = self.state_machine.add(state, CharacterSet::from_char(ch));
        });

        state
    }

    fn add_key_value(&mut self, name: &'static str, mut state: usize) -> usize {
        name.chars().for_each(|c| {
            state = self.state_machine.add(state, CharacterSet::from_char(c));
        });
        state = self.state_machine.add(state, CharacterSet::from_char('='));

        let mut char_set = CharacterSet::any();
        char_set.remove(&[' ', '\n', '"']);
        state = self.state_machine.add(state, char_set);
        self.state_machine.add_next_state(state, state);
        self.state_machine.start_parse(state, name);
        self.state_machine.end_parse(state);

        state
    }

    fn add_quoted_key_value(&mut self, name: &'static str, mut state: usize) -> usize {
        name.chars().for_each(|c| {
            state = self.state_machine.add(state, CharacterSet::from_char(c));
        });
        state = self.state_machine.add(state, CharacterSet::from_char('='));
        state = self.state_machine.add(state, CharacterSet::from_char('"'));

        let mut char_set = CharacterSet::any();
        char_set.remove(&['"']);
        state = self.state_machine.add(state, char_set);
        self.state_machine.add_next_state(state, state);
        self.state_machine.start_parse(state, name);
        self.state_machine.end_parse(state);

        state = self.state_machine.add(state, CharacterSet::from_char('"'));

        state
    }
}

fn key_value_pair(s: &'static str) -> Option<&'static str> {
    s.match_indices("={}")
        .next()
        .map(|pair| {
            let name = &s[0..pair.0];
            if name.len() > 0 {
                Some(name)
            } else {
                None
            }
        })
        .flatten()
}
