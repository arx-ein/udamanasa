use super::ManamiSlashCommand;
use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::model::application::{CommandOptionType, ResolvedOption, ResolvedValue};

pub const SLASH_ECHO_COMMAND: ManamiSlashCommand = ManamiSlashCommand {
    name: "echo",
    usage: "/echo <text>",
    description: "もらったメッセージをオウム返しするよ！",
    register,
    run: |opt, _| {
        let result = run(opt);
        Box::pin(async move { result })
    },
    is_local_command: false,
};

pub fn run(opt: Vec<ResolvedOption<'_>>) -> String {
    opt.iter()
        .find_map(|opt| match (opt.name, &opt.value) {
            ("text", ResolvedValue::String(s)) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

pub fn register() -> CreateCommand {
    CreateCommand::new("echo")
        .description("もらったメッセージをそのまま返すよ！")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "text", "返すテキスト")
                .required(true),
        )
}
