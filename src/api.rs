pub mod app;
pub mod model;
pub mod api;
use leptos::*;
use crate::model::conversation::Conversation;

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "hydrate")] {
        use wasm_bindgen::prelude::wasm_bindgen;

        #[wasm_bindgen]
        pub fn hydrate() {
            use app::*;
            use leptos::*;

            console_error_panic_hook::set_once();

            leptos::mount_to_body(move {
                view! {
                    <App/>
                }
            })
        }
    }
}

#[server(Converse "/api")]
pub async fn converse(prompt: Conversation) -> Result<String, ServerFnError> {
    use llm::models::Llama;
    use lepto_actix::extract;
    use actix_web::web::Data;
    use actix_web::dev::ConnectionInfo;

    let model = extract(|data: Data<Llama>, _connection: ConnectionInfo| async {
        data.into_inner()
    })
    .await.unwrap();

    use llm::KnownModel;
    let character_name = "### Assistant";
    let user_name = "### User";
    let persona = "A chat between a human and an AI assistant.";
    let mut history = format!(
        "{character_name}: Hello - How may I help you today?\n\
    {user_name}: What is the capital of Mexico?\n\
    {character_name}: Mexico City is the capital of Mexico.\n"
    );

    for message in prompt.message.into_iter() {
        let msg = message.text;
        let curr_line = if message.user {
            format!("{character_name}:{msg}\n")
        } else {
            format!("{user_name}:{msg}\n")
        };

        history.push_str(&curr_line);
    }

    let mut res = String::new();
    let mut rng = rand::thread_rng();
    let mut but = String::new();
    
    let mut session = model.start_session(Default::default());

    session.infer(
        model.as_ref(),
        &mut rng,
        &llm::InterferenceRequest {
            prompt: format!("{persona}\n{history}\n{character_name}:")
            .as_str()
            .into(),
            parameters: &llm::InferenceParameters::default(),
            play_back_previous_tokens: false,
            maximum_token_count: None,
        },
        &mut Default::default(),
        inference_callback(String::from(user_name), &mut buf, &mut res),
    )
    .unwrap_or_else(|e| {
        panic!("Error: {}", e);
    });

    Ok(res)
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use std::convert::Infallible;
        fn inference_callback<'a>(
            stop_sequence: String,
            buf: &'a mut String,
            out_str: &'a mut String,
        ) -> impl FnMut(llm::InferenceResponse) -> Result<llm::InferenceFeedback, Infallible> + 'a {
            use llm::InferenceFeedback::Halt;
            use llm::InferenceFeedback::Continue;

            move |resp| -> Result<llm::InferenceFeedback, Infallible> {match resp {
                llm::InferenceResponse::InferredToken(t) => {
                    let mut reverse_buf = buf.clone();
                    reverse_buf.push_str(t.as_str());
                    if stop_sequence.as_str().eq(reverse_buf.as_str()) {
                        buf.clear();
                        return Ok(Halt);
                    } else if stop_sequence.as_str().starts_with(reverse_buf.as_str()) {
                        buf.push_str(t.as_str());
                        return Ok(Continue);
                    }

                    // Clone the string we're going to send
                    let text_to_send = if buf.is_empty() {
                        t.clone()
                    } else {
                        reverse_buf
                    };

                    let tx_cloned = tx.clone();
                    runtime.block_on(async move {
                        tx_cloned.send(text_to_send).await.expect("issue sending on channel");
                    });

                    Ok(Continue)
                }
                llm::InferenceResponse::EotToken => Ok(Halt),
                _ => Ok(Continue),
            }}
        }
    }
}