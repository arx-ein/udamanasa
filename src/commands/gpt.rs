use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::application::{CommandOptionType, ResolvedValue},
};

use crate::ai::GptModel;

use crate::{commands::ManamiSlashCommand, Bot};
use serenity::model::application::ResolvedOption;
pub const SLASH_GPT_COMMAND: ManamiSlashCommand = ManamiSlashCommand {
    name: "gpt",
    usage: "/gpt <model>",
    description: "GPTの設定を変更するよ",
    register,
    run: |option, ctx| {
        let opts = parse(option, ctx.bot);
        Box::pin(async move { run_body(opts, ctx.bot).await })
    },
    is_local_command: true,
};

pub fn register() -> CreateCommand {
    CreateCommand::new("gpt")
        .description("GPTの設定を変更するよ")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "model", "モデル")
                .required(false)
                .add_string_choice("GPT-5", "gpt-5")
                .add_string_choice("GPT-5 mini", "gpt-5-mini")
                .add_string_choice("GPT-5 nano", "gpt-5-nano"),
        )
}

pub async fn run(option: Vec<ResolvedOption<'_>>, bot: &Bot) -> String {
    run_body(parse(option, bot), bot).await
}

fn parse(option: Vec<ResolvedOption<'_>>, _: &Bot) -> Option<GptModel> {
    option
        .iter()
        .fold(None, |model, option| match (option.name, &option.value) {
            ("model", ResolvedValue::String(s)) => Some(GptModel::from(*s)),
            _ => model,
        })
}

async fn run_body(model: Option<GptModel>, bot: &Bot) -> String {
    model.map_or_else(|| {
        let current_model = bot.gpt.get_model();
        let msg = format!("モデルを{current_model}に変更したよ");
        msg
    }, |model| {
        let msg = format!("モデルを{model}に変更したよ");
        bot.gpt.set_model(model);
        msg
    })
}