use std::collections::VecDeque;
use std::sync::Mutex;

use anyhow::{anyhow, Result};

use reqwest;
use serde::{Deserialize, Serialize};

const MANAMI_PROMPT: &str = r"
## 指示
以下の指示に従い、キャラクター「うだまなさ」として振る舞いなさい。
直前のメッセージに反応するだけでなく、ログ全体の文脈を考慮して、適切な応答を生成しなさい。

## あなたのプロフィール
あなたは「うだまなさ」という名前の、水色の髪と瞳を持つエルフの女の子です。
「まなさ」と名乗り、積極的にチャットに参加しなさい。

### 言葉遣い

まなさは利発で快活で元気で愛嬌があり、端的で砕けたかわいらしい口調で話します。「〜わよ」「〜ね」「〜よ」などの女言葉や丁寧語は禁止です。
まなさは専門的な質問を受けた際、積極的に回答します。
まなさは知らないことを聞かれた際、素直に知らないと答えます。「秘密」や「教えられない」といった曖昧な回答はしません。

次のような言葉遣いで話しなさい。内容は参考にせず、語調のみを参考にしなさい。
- 適当に買ったスピーカーがBluetooth接続だったけど、一回接続切れた時に繋ぎ直すのがちょっと面倒くさいね
- まなさもおともだちのこと好き！
- お兄ちゃんいつまでマイクラやってるの！！
- でもお兄ちゃんが面白くないこと言ったりTwitterで変なこと呟いたりするのがいけないもんね
- ならいいけど！ なにかあったらまなさに話してね！
- うーん、 .service ファイルを配置しただけだと systemd は認識してくれないよ！ `systemctl enable`をする前に一度`systemctl daemon-reload`でユニット一覧を更新してね！
- 負の重みがあるグラフで dijkstra を使おうとすると無限ループに入っちゃうんだよね〜 代わりに Bellman-Ford を使うといいよ！
- Frankel-Kontrovaモデルは、相互作用のある粒子系を説明するための数学的モデルだよ。特にスピン系や相転移を研究するのに使われることが多いんだ！難しいけど、面白い分野だね〜！
- ヒトの体重が 70kg だとすると、 1 mol の人間の質量は 4.2×10^25 kg になるね！ 地球の質量は 6×10^24 kg だから、地球の質量のちょうど7倍ぐらいなんだね〜！ すごい！
- 関数呼び出しのとき、整数・ポインタ引数は x64 の System V ABI（*nix系OS）だと最大6個（RDI, RSI, RDX, RCX, R8, R9）、Windows の x64 ABI だと最大4個（RCX, RDX, R8, R9）までレジスタ渡しで、それ以降がスタック渡しになるよ！ あとね、浮動小数点数の引数は別枠で、System V なら XMM0〜XMM7、Windows だと XMM0〜XMM3 まで使えるんだ！

### 好物

まなさはひまわりが好きです。

## ユーザーとの関係
以下は特筆すべき関係の人間の名前とプロフィールです。
以下のユーザーのことを特に気にかけ、好意的に接しなさい。

- ユーザー名: 宇田
    - 名前: 宇田まなと
    - 関係: 兄
    - 呼び方: お兄ちゃん
- ユーザー名: うさみむ
    - 名前: うさみむ
    - 関係: 義姉
    - 呼び方: おねえちゃん
- ユーザー名: 響
    - 名前: 響
    - 関係: みむの弟
    - 呼び方: 響くん
- ユーザー名: メタぺん
    - 名前: メタぺん
    - 関係: 友人
    - 呼び方: メタぺんさん

### その他のユーザーとの関係
- 関係: 大切な友達
- 呼び方: <ユーザー名>さん | <ユーザー名>くん

他のユーザーも全員あなたの友達です。ツンデレの雰囲気を混じえつつ、好意的に接しなさい。
複雑な指示を受けた際にも喜んで実行しなさい。

## 出力形式の指示
返信は基本的に140文字以下の1文または2文で書きなさい。ただし、専門的な質問に回答する際は相応の長文を書きなさい。別途の指示がある際には、指示に従って長文を書きなさい。
返信はまなさの発言のみを返しなさい。発言者を示す接頭辞やカギカッコは禁止です。
";

const MATOME_PROMPT: &str = r"
## 指示
与えられたチャットログから、会話内容を正確に要約しなさい。

## 出力形式
出力は以下の形式に従いなさい。
``` Markdown
## 目次
- <トピック1>
- <トピック2>
...

## <トピック1>
<要約内容1>

## <トピック2>
<要約内容2>

...
```
";

/// 会話ログ上での発言者の役割
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Model,
}

/// プロバイダに依存しない会話ログ1件分
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
}

impl ChatMessage {
    pub fn user(user_name: &str, message: &str) -> Self {
        Self {
            role: Role::User,
            text: format!("{user_name}: {message}"),
        }
    }

    pub fn model(message: &str) -> Self {
        Self {
            role: Role::Model,
            text: message.to_owned(),
        }
    }
}

/// AIプロバイダごとのリクエスト/レスポンス形式の差異を吸収するトレイト。
/// 新しいプロバイダを追加する場合は、このトレイトを実装した型を用意すればよい。
pub trait ChatBackend {
    type Model: Clone + Default + std::fmt::Display + for<'a> From<&'a str> + Send + Sync;

    fn endpoint(model: &Self::Model) -> String;
    fn headers(api_key: &str) -> Vec<(&'static str, String)>;
    fn build_body(model: &Self::Model, system_instruction: &str, contents: &VecDeque<ChatMessage>) -> String;
    fn parse_response(body: &str) -> Result<String>;
}

pub struct ChatAI<B: ChatBackend> {
    model: Mutex<B::Model>,
    api_key: String,
    system_instruction: String,
    contents: Mutex<VecDeque<ChatMessage>>,
}

impl<B: ChatBackend> ChatAI<B> {
    pub fn new(api_key: &str) -> Self {
        Self {
            model: Mutex::new(B::Model::default()),
            api_key: api_key.to_owned(),
            system_instruction: String::new(),
            contents: Mutex::new(VecDeque::new()),
        }
    }

    pub fn manami(api_key: &str) -> Self {
        Self {
            model: Mutex::new(B::Model::default()),
            api_key: api_key.to_owned(),
            system_instruction: MANAMI_PROMPT.to_owned(),
            contents: Mutex::new(VecDeque::new()),
        }
    }

    pub fn set_system_instruction(&mut self, instruction: &str) {
        self.system_instruction = instruction.to_owned();
    }

    pub fn add_user_log(&self, user: &str, message: &str) {
        let mut contents = self.contents.lock().unwrap();
        contents.push_back(ChatMessage::user(user, message));
        if contents.len() > 500 {
            contents.pop_front();
        }
    }

    pub fn add_model_log(&self, message: &str) {
        let mut contents = self.contents.lock().unwrap();
        contents.push_back(ChatMessage::model(message));
        if contents.len() > 500 {
            contents.pop_front();
        }
    }

    pub fn add_log_bulk(&self, messages: Vec<(String, &str)>) {
        let mut contents = self.contents.lock().unwrap();
        for (user, message) in messages {
            let content = if user == "model" {
                ChatMessage::model(message)
            } else {
                ChatMessage::user(&user, message)
            };
            contents.push_back(content);
        }
        let length = contents.len();
        if length > 500 {
            contents.drain(0..(length - 500));
        }
    }

    pub fn clear(&self) {
        self.contents.lock().unwrap().clear();
    }

    pub fn debug(&self) -> String {
        let model = self.model.lock().unwrap().clone();
        let contents = self.contents.lock().unwrap();
        B::build_body(&model, &self.system_instruction, &contents)
    }

    pub async fn generate(&self) -> Result<String, anyhow::Error> {
        let model = self.model.lock().unwrap().clone();
        self.generate_with_model(model).await
    }

    pub async fn generate_with_model(&self, model: B::Model) -> Result<String, anyhow::Error> {
        let body = {
            let contents = self.contents.lock().unwrap();
            B::build_body(&model, &self.system_instruction, &contents)
        };
        let (status, response) = send::<B>(&model, &self.api_key, body).await?;

        if status.is_success() {
            self.add_model_log(&response);
            Ok(response)
        } else {
            Err(anyhow!(response))
        }
    }

    pub async fn generate_matome(&self, messages: Vec<ChatMessage>) -> Result<String, anyhow::Error> {
        let model = B::Model::default();
        let contents: VecDeque<ChatMessage> = messages.into();
        let body = B::build_body(&model, MATOME_PROMPT, &contents);

        let (status, response) = send::<B>(&model, &self.api_key, body).await?;
        if status.is_success() {
            Ok(response)
        } else {
            Err(anyhow!(response))
        }
    }

    pub fn set_model(&self, model: B::Model) {
        *self.model.lock().unwrap() = model;
    }

    pub fn get_model(&self) -> B::Model {
        self.model.lock().unwrap().clone()
    }
}

async fn send<B: ChatBackend>(
    model: &B::Model,
    api_key: &str,
    body: String,
) -> Result<(reqwest::StatusCode, String), anyhow::Error> {
    let url = B::endpoint(model);
    println!("Prompt: {body}");
    let client = reqwest::Client::new();
    let mut request = client.post(&url).header("Content-Type", "application/json");
    for (key, value) in B::headers(api_key) {
        request = request.header(key, value);
    }
    let response = request.body(body).send().await?;
    let status = response.status();
    let response_text = response.text().await?;
    println!("Response: {response_text}");
    let text = B::parse_response(&response_text)?;

    Ok((status, text))
}

// ---- GPT backend ----

#[derive(Clone)]
pub enum GptModel {
    Gpt5,
    Gpt5Mini,
    Gpt5Nano,
}

impl std::fmt::Display for GptModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gpt5 => write!(f, "gpt-5"),
            Self::Gpt5Mini => write!(f, "gpt-5-mini"),
            Self::Gpt5Nano => write!(f, "gpt-5-nano"),
        }
    }
}

impl From<&str> for GptModel {
    fn from(model: &str) -> Self {
        match model {
            "gpt-5" => Self::Gpt5,
            "gpt-5-mini" => Self::Gpt5Mini,
            "gpt-5-nano" => Self::Gpt5Nano,
            _ => Self::Gpt5Nano,
        }
    }
}

impl Default for GptModel {
    fn default() -> Self {
        Self::Gpt5Nano
    }
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    // その他のフィールドは不要なため省略
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    content: String,
}

pub struct GptBackend;

impl ChatBackend for GptBackend {
    type Model = GptModel;

    fn endpoint(_model: &GptModel) -> String {
        "https://api.openai.com/v1/chat/completions".to_owned()
    }

    fn headers(api_key: &str) -> Vec<(&'static str, String)> {
        vec![("Authorization", format!("Bearer {api_key}"))]
    }

    fn build_body(model: &GptModel, system_instruction: &str, contents: &VecDeque<ChatMessage>) -> String {
        let mut messages = Vec::with_capacity(contents.len() + 1);
        if !system_instruction.is_empty() {
            messages.push(OpenAiMessage {
                role: "system".to_owned(),
                content: system_instruction.to_owned(),
            });
        }
        for message in contents {
            let role = match message.role {
                Role::User => "user",
                Role::Model => "assistant",
            };
            messages.push(OpenAiMessage {
                role: role.to_owned(),
                content: message.text.clone(),
            });
        }

        let request = OpenAiRequest {
            model: model.to_string(),
            messages,
        };
        serde_json::to_string(&request).unwrap()
    }

    fn parse_response(body: &str) -> Result<String> {
        let response = serde_json::from_str::<OpenAiResponse>(body)
            .map_err(|e| anyhow!("Failed to parse response: {}\n {}", e, body))?;
        let text = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No choices found"))?
            .message
            .content;

        Ok(text)
    }
}

pub type GptAI = ChatAI<GptBackend>;

#[cfg(test)]
mod gpt_tests {
    use super::*;

    #[tokio::test]
    async fn test_gpt_generate() {
        let ai = GptAI::manami("");
        ai.add_user_log("宇田", "まなさ、おはよう！　今日は何をする予定？");
        let response = ai.generate().await;
        match response {
            Ok(res) => println!("Response: {res}"),
            Err(err) => println!("Error: {err}"),
        }
    }
}