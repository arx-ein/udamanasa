use crate::ai::GptModel;
use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::application::{CommandOptionType, ResolvedValue},
};
use std::time::Duration;

use crate::{commands::ManamiSlashCommand, Bot};
use serenity::model::application::ResolvedOption;

const COMMAND_NAME: &str = "auto";

pub const SLASH_AUTO_COMMAND: ManamiSlashCommand = ManamiSlashCommand {
    name: COMMAND_NAME,
    usage: "/auto [model] [sec]",
    description: "呼びかけられなくてもお返事するよ",
    register,
    run: |option, ctx| {
        let opts = parse_options(option, ctx.bot);
        Box::pin(async move { run_body(opts, ctx.bot).await })
    },
    is_local_command: true,
};

pub fn register() -> CreateCommand {
    CreateCommand::new(COMMAND_NAME)
        .description("[sec]秒以内の連続した会話に対して、[model]を使って必ず返信するよ")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "model", "モデル")
                .required(false)
                .add_string_choice("GPT-5", "gpt-5")
                .add_string_choice("GPT-5 mini", "gpt-5-mini")
                .add_string_choice("GPT-5 nano", "gpt-5-nano"),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::Integer, "sec", "秒数")
                .required(false)
                .min_int_value(0)
                .max_int_value(600),
        )
}

pub async fn run(option: Vec<ResolvedOption<'_>>, bot: &Bot) -> String {
    run_body(parse_options(option, bot), bot).await
}

fn parse_options(option: Vec<ResolvedOption<'_>>, bot: &Bot) -> (GptModel, Duration) {
    let model = option
        .iter()
        .fold(None, |model, option| match (option.name, &option.value) {
            ("model", ResolvedValue::String(s)) => Some(GptModel::from(*s)),
            _ => model,
        })
        .unwrap_or_else(|| bot.gpt.get_model());

    let sec = Duration::from_secs(
        option
            .iter()
            .fold(None, |sec, option| match (option.name, &option.value) {
                ("sec", ResolvedValue::Integer(s)) => Some(*s as u64),
                _ => sec,
            })
            .unwrap_or(300),
    );

    (model, sec)
}

async fn run_body((model, sec): (GptModel, Duration), bot: &Bot) -> String {
    bot.reply_to_all_mode
        .lock()
        .unwrap()
        .set(model.clone(), sec);

    let msg = format!(
        "（全レスモード：{}秒以内の連続した会話に対して、{}で返信するよ）",
        sec.as_secs(),
        model
    );
    msg
}
