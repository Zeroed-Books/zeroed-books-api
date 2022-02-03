use anyhow::Result;
use sendgrid::v3::{Content, Email, Personalization, Sender};
use tracing::info;

pub struct Message {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[async_trait]
pub trait EmailClient: Send + Sync {
    async fn send(&self, message: &Message) -> Result<()>;
}

pub struct ConsoleMailer {
    pub from: String,
}

#[async_trait]
impl EmailClient for ConsoleMailer {
    async fn send(&self, message: &Message) -> Result<()> {
        println!("From: {}", self.from);
        println!("To: {}", message.to);
        println!("Subject: {}", message.subject);
        println!("{}", "-".repeat(80));
        println!("{}\n", message.text);

        Ok(())
    }
}

pub struct SendgridMailer {
    from: Email,
    sender: Sender,
}

impl SendgridMailer {
    pub fn new(api_key: String, from_address: String, from_name: String) -> Self {
        Self {
            from: Email::new(from_address).set_name(from_name),
            sender: Sender::new(api_key),
        }
    }
}

#[async_trait]
impl EmailClient for SendgridMailer {
    async fn send(&self, message: &Message) -> Result<()> {
        let personalization = Personalization::new(Email::new(message.to.to_owned()));

        let sendable_message = sendgrid::v3::Message::new(self.from.clone())
            .set_subject(&message.subject)
            .add_content(
                Content::new()
                    .set_content_type("text/plain")
                    .set_value(message.text.to_owned()),
            )
            .add_personalization(personalization);

        self.sender.send(&sendable_message).await?;
        info!(subject = %message.subject, "Sent email via SendGrid.");

        Ok(())
    }
}
