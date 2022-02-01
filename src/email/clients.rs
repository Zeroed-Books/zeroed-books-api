use std::error::Error;

use sendgrid::v3::{Content, Email, Personalization, Sender};

pub struct Message {
    pub to: String,
    pub from: String,
    pub subject: String,
    pub text: String,
}

impl From<&Message> for sendgrid::v3::Message {
    fn from(msg: &Message) -> Self {
        let personalization = Personalization::new(Email::new(msg.to.to_owned()));

        Self::new(Email::new(msg.from.clone()))
            .set_subject(&msg.subject)
            .add_content(
                Content::new()
                    .set_content_type("text/plain")
                    .set_value(msg.text.clone()),
            )
            .add_personalization(personalization)
    }
}

#[async_trait]
pub trait EmailClient: Send + Sync {
    async fn send(&self, message: &Message) -> Result<(), Box<dyn Error>>;
}

pub struct ConsoleMailer();

#[async_trait]
impl EmailClient for ConsoleMailer {
    async fn send(&self, message: &Message) -> Result<(), Box<dyn Error>> {
        println!("From: {}", message.from);
        println!("To: {}", message.to);
        println!("Subject: {}", message.subject);
        println!("{}", "-".repeat(80));
        println!("{}\n", message.text);

        Ok(())
    }
}

pub struct SendgridMailer {
    sender: Sender,
}

impl SendgridMailer {
    pub fn new(api_key: String) -> Self {
        Self {
            sender: Sender::new(api_key),
        }
    }
}

#[async_trait]
impl EmailClient for SendgridMailer {
    async fn send(&self, message: &Message) -> Result<(), Box<dyn Error>> {
        match self.sender.send(&message.into()).await {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}
