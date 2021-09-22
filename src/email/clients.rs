pub struct Message {
    pub to: String,
    pub from: String,
    pub subject: String,
    pub text: String,
}

#[async_trait]
pub trait EmailClient: Send + Sync {
    async fn send(&self, message: &Message) -> Result<(), ()>;
}

pub struct ConsoleMailer();

#[async_trait]
impl EmailClient for ConsoleMailer {
    async fn send(&self, message: &Message) -> Result<(), ()> {
        println!("From: {}", message.from);
        println!("To: {}", message.to);
        println!("Subject: {}", message.subject);
        println!("{}", "-".repeat(80));
        println!("{}\n", message.text);

        Ok(())
    }
}
