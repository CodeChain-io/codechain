use sendgrid::v3 as sendgrid;

pub struct EmailAlarmConfig {
    pub to: String,
    pub sendgrid_key: String,
}

impl EmailAlarmConfig {
    pub fn new(to: String, sendgrid_key: String) -> Self {
        Self {
            to,
            sendgrid_key,
        }
    }
}

#[derive(Clone)]
pub struct EmailAlarm {
    pub to: String,
    pub sendgrid_key: String,
}

impl EmailAlarm {
    pub fn new(config: &EmailAlarmConfig) -> Self {
        Self {
            to: config.to.clone(),
            sendgrid_key: config.sendgrid_key.clone(),
        }
    }

    pub fn send(&self, log: &str) {
        let p = sendgrid::Personalization::new().add_to(sendgrid::Email::new().set_email(&self.to));
        let now = time::now_utc();
        let now = now.rfc3339();
        let m = sendgrid::Message::new()
            .set_from(sendgrid::Email::new().set_email("no-reply@codechain.io"))
            // FIXME: fill the network id
            .set_subject(&format!("[error][?c][codechain] Error from CodeChain-{}", now))
            .add_content(sendgrid::Content::new().set_content_type("text/html").set_value(log))
            .add_personalization(p);
        let sender = sendgrid::Sender::new(self.sendgrid_key.clone());
        let send_result = sender.send(&m);
        if let Err(err) = send_result {
            eprintln!("Sent an email, but failed. returned error is {}", err);
        }
    }
}
