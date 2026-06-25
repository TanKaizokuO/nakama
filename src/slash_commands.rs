pub trait SlashCommandHandler {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn execute(&self, args: &str) -> String;
}

pub struct SlashCommandRegistry {
    handlers: std::collections::HashMap<String, Box<dyn SlashCommandHandler>>,
}

impl Default for SlashCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SlashCommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, handler: Box<dyn SlashCommandHandler>) {
        self.handlers.insert(handler.name().to_string(), handler);
    }

    pub fn dispatch(&self, input: &str) -> Option<String> {
        if !input.starts_with('/') {
            return None;
        }

        let parts: Vec<&str> = input.trim().splitn(2, ' ').collect();
        let cmd_name = &parts[0][1..]; // trim the '/'
        let args = if parts.len() > 1 { parts[1] } else { "" };

        if let Some(handler) = self.handlers.get(cmd_name) {
            Some(handler.execute(args))
        } else {
            // Unrecognized slash command => treat as model prompt as per spec
            None
        }
    }
}
